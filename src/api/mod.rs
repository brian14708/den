mod health;

use crate::state::AppState;
use axum::Router;

pub fn router() -> Router<AppState> {
    Router::new().route("/health", axum::routing::get(health::check))
}
