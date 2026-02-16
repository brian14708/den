use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use serde::Serialize;

use crate::state::AppState;

#[derive(Serialize)]
pub struct Health {
    pub status: &'static str,
}

pub async fn check(State(state): State<AppState>) -> Result<Json<Health>, StatusCode> {
    sqlx::query_scalar::<_, i64>("SELECT 1")
        .fetch_one(&state.db)
        .await
        .map_err(|error| {
            tracing::warn!(error = %error, "health check database ping failed");
            StatusCode::SERVICE_UNAVAILABLE
        })?;

    Ok(Json(Health { status: "ok" }))
}
