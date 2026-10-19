use rustix::event;
use rustix::fd::AsFd;
use rustix::net;
use rustix::pipe;
use rustix::time::Timespec;
use std::env;
use std::io::IoSlice;
use std::mem::MaybeUninit;
use std::os::linux::net::SocketAddrExt;
use std::os::unix::net::{SocketAddr, UnixDatagram};
use std::time::Duration;

const BARRIER_TIMEOUT: Duration = Duration::from_secs(5);

/// Talks to the service manager over `NOTIFY_SOCKET`. Outside systemd the
/// variable is absent and every notification is a no-op.
pub(crate) struct SystemdNotifier;

impl SystemdNotifier {
    pub(crate) fn ready() -> Result<(), String> {
        let socket = Self::connect()?;

        Self::send(socket.as_ref(), "READY=1")
    }

    /// Makes `pid` (a process of the unit's cgroup) the unit's main process,
    /// and returns only once systemd has processed the change (notification
    /// barrier). An error means the change is not confirmed.
    pub(crate) fn main_pid(pid: u32) -> Result<(), String> {
        let Some(socket) = Self::connect()? else {
            return Ok(());
        };

        Self::send(Some(&socket), &format!("MAINPID={pid}"))?;

        Self::barrier(&socket)
    }

    fn connect() -> Result<Option<UnixDatagram>, String> {
        let Some(path) = env::var_os("NOTIFY_SOCKET") else {
            return Ok(None);
        };
        let path = path.to_string_lossy().into_owned();
        let socket = UnixDatagram::unbound()
            .map_err(|error| format!("failed to create notify socket: {error}"))?;

        let connected = match path.strip_prefix('@') {
            Some(name) => SocketAddr::from_abstract_name(name.as_bytes())
                .and_then(|address| socket.connect_addr(&address)),

            None => socket.connect(&path),
        };

        connected.map_err(|error| format!("failed to connect to NOTIFY_SOCKET {path}: {error}"))?;

        Ok(Some(socket))
    }

    fn send(socket: Option<&UnixDatagram>, message: &str) -> Result<(), String> {
        let Some(socket) = socket else {
            return Ok(());
        };

        socket
            .send(message.as_bytes())
            .map(|_| ())
            .map_err(|error| format!("failed to notify systemd ({message}): {error}"))
    }

    /// Equivalent of `sd_notify_barrier`: sends `BARRIER=1` with the write end
    /// of a pipe. Systemd handles notifications in order and closes the fd
    /// once it reaches the barrier, which shows as a hang-up on the read end.
    fn barrier(socket: &UnixDatagram) -> Result<(), String> {
        let (read_end, write_end) = pipe::pipe_with(pipe::PipeFlags::CLOEXEC)
            .map_err(|error| format!("failed to create barrier pipe: {error}"))?;

        let descriptors = [write_end.as_fd()];
        let mut space = [MaybeUninit::uninit(); rustix::cmsg_space!(ScmRights(1))];
        let mut ancillary = net::SendAncillaryBuffer::new(&mut space);

        if !ancillary.push(net::SendAncillaryMessage::ScmRights(&descriptors)) {
            return Err("barrier ancillary buffer is too small".to_string());
        }

        net::sendmsg(
            socket,
            &[IoSlice::new(b"BARRIER=1")],
            &mut ancillary,
            net::SendFlags::NOSIGNAL,
        )
        .map_err(|error| format!("failed to send systemd barrier: {error}"))?;

        // Only systemd's copy of the write end may remain open.
        drop(write_end);

        let mut poll_fds = [event::PollFd::new(&read_end, event::PollFlags::empty())];
        let timeout = Timespec {
            tv_sec: BARRIER_TIMEOUT.as_secs() as i64,
            tv_nsec: 0,
        };

        let ready = event::poll(&mut poll_fds, Some(&timeout))
            .map_err(|error| format!("failed to wait for the systemd barrier: {error}"))?;

        if ready == 0 {
            return Err("systemd did not reach the notification barrier in time".to_string());
        }

        if !poll_fds[0].revents().contains(event::PollFlags::HUP) {
            return Err("unexpected event while waiting for the systemd barrier".to_string());
        }

        Ok(())
    }
}
