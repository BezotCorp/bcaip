use serde::Deserialize;

#[derive(Deserialize, Debug)]
pub(crate) enum WatcherRequest {
    Begin {
        update_id: String,
        product: String,
        expected_size: u64,
        sha256: String,
    },
    Status {
        product: String,
    },
}
