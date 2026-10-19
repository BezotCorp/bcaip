use std::net::{TcpListener, TcpStream};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use storage_exchange_format::{
    StorageOperation, StorageOutcome, StorageRequest, StorageResponse, decode, encode,
};
use tempfile::tempdir;
use tungstenite::stream::MaybeTlsStream;
use tungstenite::{Message, WebSocket, connect};

fn connect_storage(
    address: std::net::SocketAddr,
    child: &mut Child,
) -> WebSocket<MaybeTlsStream<TcpStream>> {
    let url = format!("ws://{address}");
    let deadline = Instant::now() + Duration::from_secs(5);

    loop {
        match connect(url.as_str()) {
            Ok((mut socket, _)) => {
                if let MaybeTlsStream::Plain(stream) = socket.get_mut() {
                    stream
                        .set_read_timeout(Some(Duration::from_secs(3)))
                        .expect("Timeout de lecture impossible");

                    stream
                        .set_write_timeout(Some(Duration::from_secs(3)))
                        .expect("Timeout d'écriture impossible");
                }

                return socket;
            }
            Err(error) => {
                if let Some(status) = child.try_wait().expect("Impossible de vérifier Storage") {
                    panic!("Storage s'est arrêté : {status}");
                }

                if Instant::now() >= deadline {
                    panic!("Connexion WebSocket impossible : {error}");
                }

                thread::sleep(Duration::from_millis(50));
            }
        }
    }
}

fn request(
    socket: &mut WebSocket<MaybeTlsStream<TcpStream>>,
    id: u64,
    operation: StorageOperation,
) -> StorageOutcome {
    let request = StorageRequest { id, operation };
    let bytes = encode(&request).expect("Encodage binaire impossible");

    socket
        .send(Message::Binary(bytes.into()))
        .expect("Storage doit accepter la demande");

    let message = socket.read().expect("Storage doit renvoyer une réponse");

    let bytes = match message {
        Message::Binary(bytes) => bytes,
        other => panic!("Réponse non binaire : {other:?}"),
    };

    let response: StorageResponse = decode(&bytes).expect("Réponse BinCodeNext invalide");

    assert_eq!(response.id, id);

    response.outcome
}

#[test]
fn storage_accepts_a_new_connection_after_client_disconnect() {
    let directory = tempdir().unwrap();
    let database = directory.path().join("data.sqlite3");

    let listener = TcpListener::bind("127.0.0.1:0").expect("Impossible de réserver un port");

    let address = listener.local_addr().expect("Adresse indisponible");
    drop(listener);

    let mut child = Command::new(env!("CARGO_BIN_EXE_storage"))
        .env("BCAIP_STORAGE_PATH", &database)
        .env("BCAIP_STORAGE_LISTEN_ADDR", address.to_string())
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("Impossible de démarrer Storage");

    {
        let mut first = connect_storage(address, &mut child);

        assert_eq!(
            request(&mut first, 1, StorageOperation::Ping),
            StorageOutcome::Pong,
        );

        first
            .close(None)
            .expect("La première connexion doit pouvoir être fermée");
    }

    let mut second = connect_storage(address, &mut child);

    assert_eq!(
        request(&mut second, 2, StorageOperation::Ping),
        StorageOutcome::Pong,
    );

    let _ = second.close(None);
    let _ = child.kill();
    let _ = child.wait();
}
