mod exchange_error;
mod storage_operation;
mod storage_outcome;
mod storage_request;
mod storage_response;

pub use exchange_error::ExchangeError;
pub use storage_operation::StorageOperation;
pub use storage_outcome::StorageOutcome;
pub use storage_request::StorageRequest;
pub use storage_response::StorageResponse;

pub fn encode<T: bincode_next::Encode>(
    value: &T,
) -> Result<Vec<u8>, bincode_next::error::EncodeError> {
    bincode_next::encode_to_vec(value, bincode_next::config::standard())
}

pub fn decode<T: bincode_next::Decode<()>>(
    bytes: &[u8],
) -> Result<T, bincode_next::error::DecodeError> {
    let (value, consumed) =
        bincode_next::decode_from_slice(bytes, bincode_next::config::standard())?;

    if consumed != bytes.len() {
        return Err(bincode_next::error::DecodeError::Other(
            "Trailing bytes in Storage message",
        ));
    }

    Ok(value)
}
