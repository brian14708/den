use axum::Json;
use axum::extract::State;
use serde::Serialize;

use crate::state::AppState;

#[derive(Serialize)]
pub struct Health {
    pub status: &'static str,
}

pub async fn check(State(_state): State<AppState>) -> Json<Health> {
    Json(Health { status: "ok" })
}
