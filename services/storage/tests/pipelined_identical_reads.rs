use std::collections::BTreeMap;
use std::net::{TcpListener, TcpStream};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use storage_exchange_format::{
    ExchangeError, StorageOperation, StorageOutcome, StorageRequest, StorageResponse, decode,
    encode,
};
use tempfile::tempdir;
use tungstenite::stream::MaybeTlsStream;
use tungstenite::{Message, WebSocket, connect};

struct StorageProcess {
    child: Child,
    socket: WebSocket<MaybeTlsStream<TcpStream>>,
}

impl StorageProcess {
    fn start(database: &Path) -> Self {
        let listener =
            TcpListener::bind("127.0.0.1:0").expect("Impossible de réserver un port local");

        let address = listener.local_addr().expect("Adresse locale indisponible");
        drop(listener);

        let mut child = Command::new(env!("CARGO_BIN_EXE_storage"))
            .env("BCAIP_STORAGE_PATH", database)
            .env("BCAIP_STORAGE_LISTEN_ADDR", address.to_string())
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("Impossible de démarrer Storage");

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

                    return Self { child, socket };
                }
                Err(error) => {
                    if let Some(status) = child.try_wait().expect("Impossible de vérifier Storage")
                    {
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

    fn send(&mut self, id: u64, operation: StorageOperation) {
        let request = StorageRequest { id, operation };
        let bytes = encode(&request).expect("Encodage binaire impossible");

        self.socket
            .send(Message::Binary(bytes.into()))
            .expect("Storage doit accepter la demande");
    }

    fn receive(&mut self) -> StorageResponse {
        let message = self
            .socket
            .read()
            .expect("Storage doit renvoyer une réponse");

        let bytes = match message {
            Message::Binary(bytes) => bytes,
            other => panic!("Réponse non binaire : {other:?}"),
        };

        decode(&bytes).expect("Réponse BinCodeNext invalide")
    }

    fn receive_many(&mut self, count: usize) -> BTreeMap<u64, StorageOutcome> {
        let mut responses = BTreeMap::new();

        for _ in 0..count {
            let response = self.receive();

            assert!(
                responses.insert(response.id, response.outcome).is_none(),
                "Storage a répondu plusieurs fois avec le même identifiant",
            );
        }

        responses
    }

    fn request(&mut self, id: u64, operation: StorageOperation) -> StorageOutcome {
        self.send(id, operation);

        let response = self.receive();

        assert_eq!(response.id, id);

        response.outcome
    }
}

impl Drop for StorageProcess {
    fn drop(&mut self) {
        let _ = self.socket.close(None);
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

#[test]
fn accepts_ten_in_flight_reads_of_the_same_record() {
    let directory = tempdir().unwrap();
    let mut storage = StorageProcess::start(&directory.path().join("data.sqlite3"));

    assert_eq!(
        storage.request(
            1,
            StorageOperation::Create {
                namespace: "records".into(),
                key: "shared".into(),
                content: vec![4, 5, 6],
            },
        ),
        StorageOutcome::Created { revision: 1 },
    );

    for id in 10..20 {
        storage.send(
            id,
            StorageOperation::Read {
                namespace: "records".into(),
                key: "shared".into(),
            },
        );
    }

    let responses = storage.receive_many(10);

    for id in 10..20 {
        assert_eq!(
            responses.get(&id),
            Some(&StorageOutcome::Data {
                content: vec![4, 5, 6],
                revision: 1,
            }),
        );
    }
}
