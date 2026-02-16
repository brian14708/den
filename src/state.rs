use std::collections::HashSet;
use std::sync::Arc;

use sqlx::SqlitePool;
use webauthn_rs::prelude::Webauthn;

#[derive(Clone)]
pub struct AppState {
    pub db: SqlitePool,
    pub webauthn: Arc<Webauthn>,
    pub jwt_secret: Arc<Vec<u8>>,
    pub secure_cookies: bool,
    pub rp_origin: String,
    pub allowed_hosts: Arc<HashSet<String>>,
}
