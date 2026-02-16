use axum::Json;
use serde::Serialize;

#[derive(Serialize)]
pub struct Health {
    pub status: &'static str,
}

pub async fn check() -> Json<Health> {
    Json(Health { status: "ok" })
}
