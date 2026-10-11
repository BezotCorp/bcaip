pub(crate) enum BeginUpdateResult {
    Started,
    AlreadyPending,
    AlreadyCommitted,
}
