use bincode_next::{BorrowDecode, Decode, Encode};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;
use uuid::{Uuid, Version};

#[derive(Encode, Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) struct Id([u8; 16]);

impl Id {
    pub(crate) fn from_bytes(bytes: [u8; 16]) -> Result<Self, String> {
        let uuid = Uuid::from_bytes(bytes);

        if uuid.get_version() != Some(Version::SortRand) {
            return Err(format!("expected UUIDv7, got {uuid}"));
        }

        Ok(Self(bytes))
    }

    pub(crate) fn validate(&self) -> Result<(), String> {
        Self::from_bytes(self.0).map(|_| ())
    }
}

impl<Context> Decode<Context> for Id {
    fn decode<D: bincode_next::de::Decoder<Context = Context>>(
        decoder: &mut D,
    ) -> Result<Self, bincode_next::error::DecodeError> {
        let bytes = <[u8; 16] as Decode<Context>>::decode(decoder)?;

        Self::from_bytes(bytes)
            .map_err(|_| bincode_next::error::DecodeError::Other("Invalid UUIDv7"))
    }
}


impl<'de, Context> BorrowDecode<'de, Context> for Id {
    fn borrow_decode<D>(
        decoder: &mut D,
    ) -> Result<Self, bincode_next::error::DecodeError>
    where
        D: bincode_next::de::BorrowDecoder<'de, Context = Context>,
    {
        let bytes =
            <[u8; 16] as BorrowDecode<'de, Context>>::borrow_decode(decoder)?;

        Self::from_bytes(bytes)
            .map_err(|_| bincode_next::error::DecodeError::Other("invalid UUIDv7"))
    }
}

impl<'de> Deserialize<'de> for Id {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;

        let uuid = Uuid::parse_str(&value).map_err(serde::de::Error::custom)?;

        Self::from_bytes(*uuid.as_bytes()).map_err(serde::de::Error::custom)
    }
}

impl Serialize for Id {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

impl fmt::Display for Id {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        Uuid::from_bytes(self.0).fmt(formatter)
    }
}
