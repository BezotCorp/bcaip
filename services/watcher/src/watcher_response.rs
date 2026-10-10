use crate::update_record::UpdateRecord;
use serde::Serialize;

#[derive(Serialize, Debug)]
pub(crate) enum WatcherResponse {
    Accepted {
        update_id: String,
        product: String,
    },
    Success {
        update_id: String,
        product: String,
    },
    Status {
        record: Option<UpdateRecord>,
    },
    Failure {
        message: String,
    },
}
