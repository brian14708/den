use axum::extract::FromRequestParts;
use axum::http::StatusCode;
use axum::http::request::Parts;
use axum_extra::extract::cookie::{Cookie, CookieJar, SameSite};
use jsonwebtoken::{DecodingKey, EncodingKey, Header, Validation, decode, encode};
use serde::{Deserialize, Serialize};
use time::Duration;

use crate::state::AppState;

#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String,
    pub iat: i64,
    pub exp: i64,
}

#[derive(Clone)]
pub struct AuthUser {
    pub user_id: String,
}

pub struct MaybeAuthUser(pub Option<AuthUser>);

pub fn create_token(secret: &[u8], user_id: &str) -> Result<String, jsonwebtoken::errors::Error> {
    let now = time::OffsetDateTime::now_utc();
    let claims = Claims {
        sub: user_id.to_string(),
        iat: now.unix_timestamp(),
        exp: (now + Duration::days(7)).unix_timestamp(),
    };
    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret),
    )
}

pub fn user_id_from_token(
    secret: &[u8],
    token: &str,
) -> Result<String, jsonwebtoken::errors::Error> {
    decode::<Claims>(
        token,
        &DecodingKey::from_secret(secret),
        &Validation::default(),
    )
    .map(|d| d.claims.sub)
}

pub fn session_cookie(token: String, secure: bool) -> Cookie<'static> {
    Cookie::build(("den_session", token))
        .path("/")
        .http_only(true)
        .same_site(SameSite::Strict)
        .max_age(Duration::days(7))
        .secure(secure)
        .build()
}

impl FromRequestParts<AppState> for AuthUser {
    type Rejection = StatusCode;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let jar = CookieJar::from_request_parts(parts, state).await.unwrap();
        let cookie = jar.get("den_session").ok_or(StatusCode::UNAUTHORIZED)?;
        let user_id = user_id_from_token(&state.jwt_secret, cookie.value())
            .map_err(|_| StatusCode::UNAUTHORIZED)?;
        Ok(AuthUser { user_id })
    }
}

impl FromRequestParts<AppState> for MaybeAuthUser {
    type Rejection = std::convert::Infallible;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        Ok(MaybeAuthUser(
            AuthUser::from_request_parts(parts, state).await.ok(),
        ))
    }
}
