#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Extension {
    Ron,
    Bin,
}

impl Extension {
    pub(crate) fn as_str(&self) -> &'static str {
        match self {
            Self::Ron => "ron",
            Self::Bin => "bin",
        }
    }
}
