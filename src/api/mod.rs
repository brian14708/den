mod auth;
mod health;

use crate::state::AppState;
use axum::Router;

pub fn router(state: AppState) -> Router<AppState> {
    Router::new()
        .route("/health", axum::routing::get(health::check))
        .merge(auth::router(state))
}
