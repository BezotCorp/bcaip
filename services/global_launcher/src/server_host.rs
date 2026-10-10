use crate::dns_name::DnsName;
use bincode_next::{Decode, Encode};
use serde::Deserialize;
use std::net::{Ipv4Addr, Ipv6Addr};

#[derive(Encode, Decode, Deserialize, Clone, Debug, PartialEq, Eq)]
pub(crate) enum ServerHost {
    Ipv4(Ipv4Addr),
    Ipv6(Ipv6Addr),
    Dns(DnsName),
}

impl ServerHost {
    pub(crate) fn validate(&self) -> Result<(), String> {
        match self {
            Self::Ipv4(_) | Self::Ipv6(_) => Ok(()),
            Self::Dns(name) => name.validate(),
        }
    }
}
