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

struct StorageProcess {
    child: Child,
    socket: WebSocket<MaybeTlsStream<TcpStream>>,
}

impl StorageProcess {
    fn start(database: &Path) -> Self {
        // En production, le launcher démarre les services locaux.
        // Ici, le test démarre directement le véritable binaire Storage.
        // Aucun hub n'est exécuté et aucune autorisation du hub n'est simulée.

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

    fn request(&mut self, id: u64, operation: StorageOperation) -> StorageOutcome {
        let request = StorageRequest { id, operation };

        let bytes = encode(&request).expect("Encodage binaire de la demande impossible");

        self.socket
            .send(Message::Binary(bytes.into()))
            .expect("Storage doit accepter la demande binaire");

        let message = self
            .socket
            .read()
            .expect("Storage doit renvoyer une réponse WebSocket");

        let bytes = match message {
            Message::Binary(bytes) => bytes,
            other => panic!("Réponse non binaire : {other:?}"),
        };

        let response: StorageResponse = decode(&bytes).expect("Réponse BinCodeNext invalide");

        assert_eq!(
            response.id, id,
            "Storage doit conserver l'identifiant de corrélation"
        );

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

use storage_exchange_format::ExchangeError;

#[test]
fn reading_an_unknown_identifier_returns_not_found() {
    let directory = tempdir().unwrap();
    let mut storage = StorageProcess::start(&directory.path().join("data.sqlite3"));

    assert_eq!(
        storage.request(
            1,
            StorageOperation::Read {
                namespace: "records".into(),
                key: "unknown".into(),
            },
        ),
        StorageOutcome::Error(ExchangeError::NotFound),
    );

    assert_eq!(
        storage.request(2, StorageOperation::Ping),
        StorageOutcome::Pong,
    );
}
