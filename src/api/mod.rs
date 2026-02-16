mod health;

use axum::Router;

pub fn router() -> Router {
    Router::new().route("/health", axum::routing::get(health::check))
}
