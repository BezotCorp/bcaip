use serde::Serialize;

#[derive(Serialize)]
pub(crate) enum SupervisorHealthRequest {
    Health,
}
