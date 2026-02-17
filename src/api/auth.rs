use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::Redirect;
use axum::routing::{delete, get, patch, post};
use axum::{Json, Router};
use axum_extra::extract::cookie::{Cookie, CookieJar};
use jsonwebtoken::{DecodingKey, EncodingKey, Header, Validation, decode, encode};
use serde::{Deserialize, Serialize};
use time::{Duration, OffsetDateTime};
use uuid::Uuid;
use webauthn_rs::prelude::*;

use crate::auth::{self, AuthUser, MaybeAuthUser};
use crate::origin::{normalize_origin, origin_host, request_fallback_scheme, request_origin};
use crate::state::AppState;

// --- Types ---

#[derive(Deserialize)]
struct RegisterBeginRequest {
    user_name: Option<String>,
    passkey_name: String,
}

#[derive(Serialize)]
struct BeginResponse<T: Serialize> {
    challenge_id: String,
    options: T,
}

#[derive(Deserialize)]
struct RegisterCompleteRequest {
    challenge_id: String,
    credential: RegisterPublicKeyCredential,
}

#[derive(Deserialize)]
struct LoginCompleteRequest {
    challenge_id: String,
    credential: PublicKeyCredential,
}

#[derive(Deserialize)]
struct LoginBeginRequest {
    redirect_origin: Option<String>,
    redirect_path: Option<String>,
}

#[derive(Serialize, Deserialize)]
struct RegistrationContext {
    webauthn_state: PasskeyRegistration,
    user_id: String,
    user_name: String,
    passkey_name: String,
    is_new_user: bool,
}

#[derive(Serialize, Deserialize)]
struct AuthenticationContext {
    webauthn_state: PasskeyAuthentication,
    user_id: String,
    redirect_origin: Option<String>,
    redirect_path: Option<String>,
}

#[derive(Serialize)]
struct PasskeyInfo {
    id: i64,
    name: String,
    created: String,
    last_used: Option<String>,
}

#[derive(Deserialize)]
struct RenameRequest {
    name: String,
}

#[derive(Serialize, Deserialize)]
struct LoginRedirectClaims {
    iss: String,
    aud: String,
    sub: String,
    path: String,
    iat: i64,
    exp: i64,
}

#[derive(Deserialize)]
struct RedirectCompleteQuery {
    token: String,
}

#[derive(Deserialize)]
struct RedirectStartRequest {
    redirect_origin: String,
    redirect_path: Option<String>,
}

// --- Router ---

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/auth/register/begin", post(register_begin))
        .route("/auth/register/complete", post(register_complete))
        .route("/auth/login/begin", post(login_begin))
        .route("/auth/login/complete", post(login_complete))
        .route("/auth/redirect/start", post(redirect_start))
        .route("/auth/redirect/complete", get(redirect_complete))
        .route("/auth/logout", post(logout))
        .route("/auth/passkeys", get(list_passkeys))
        .route("/auth/passkeys/{id}/name", patch(rename_passkey))
        .route("/auth/passkeys/{id}", delete(delete_passkey))
}

// --- Handlers ---

fn request_secure_cookie(headers: &HeaderMap, fallback: bool) -> bool {
    let scheme = if fallback { "https" } else { "http" };
    request_origin(headers, scheme).map_or(fallback, |o| o.starts_with("https://"))
}

fn normalize_redirect_origin(
    state: &AppState,
    origin: Option<&str>,
) -> Result<Option<String>, StatusCode> {
    let Some(origin) = origin else {
        return Ok(None);
    };
    let normalized = normalize_origin(origin).ok_or(StatusCode::BAD_REQUEST)?;
    if normalized.eq_ignore_ascii_case(&state.rp_origin) {
        return Ok(None);
    }
    let host = origin_host(&normalized).ok_or(StatusCode::BAD_REQUEST)?;
    if !state.allowed_hosts.contains(&host) {
        return Err(StatusCode::BAD_REQUEST);
    }
    Ok(Some(normalized))
}

fn normalize_redirect_target_origin(state: &AppState, origin: &str) -> Result<String, StatusCode> {
    let normalized = normalize_origin(origin).ok_or(StatusCode::BAD_REQUEST)?;
    if normalized.eq_ignore_ascii_case(&state.rp_origin) {
        return Ok(state.rp_origin.clone());
    }
    let host = origin_host(&normalized).ok_or(StatusCode::BAD_REQUEST)?;
    if !state.allowed_hosts.contains(&host) {
        return Err(StatusCode::BAD_REQUEST);
    }
    Ok(normalized)
}

fn normalize_redirect_path(path: Option<&str>) -> String {
    let path = path
        .map(str::trim)
        .filter(|p| p.starts_with('/') && !p.starts_with("//") && !p.contains('\\'));
    path.map_or_else(|| "/".into(), Into::into)
}

fn redirect_complete_url(origin: &str, token: &str) -> String {
    format!("{origin}/api/auth/redirect/complete?token={token}")
}

fn issue_login_redirect_token(
    state: &AppState,
    user_id: &str,
    origin: &str,
    path: &str,
) -> Result<String, StatusCode> {
    let now = OffsetDateTime::now_utc();
    encode(
        &Header::default(),
        &LoginRedirectClaims {
            iss: state.rp_origin.clone(),
            aud: origin.to_string(),
            sub: user_id.to_string(),
            path: path.to_string(),
            iat: now.unix_timestamp(),
            exp: (now + Duration::seconds(60)).unix_timestamp(),
        },
        &EncodingKey::from_secret(&state.jwt_secret),
    )
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

async fn register_begin(
    State(state): State<AppState>,
    auth: MaybeAuthUser,
    Json(req): Json<RegisterBeginRequest>,
) -> Result<Json<BeginResponse<CreationChallengeResponse>>, StatusCode> {
    sqlx::query("DELETE FROM auth_challenge WHERE expires_at < datetime('now')")
        .execute(&state.db)
        .await
        .ok();

    let existing: Option<(String, String)> = sqlx::query_as("SELECT id, name FROM user LIMIT 1")
        .fetch_optional(&state.db)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if existing.is_some() && auth.0.is_none() {
        return Err(StatusCode::UNAUTHORIZED);
    }

    let (user_id, user_name, is_new_user) = match existing {
        Some((id, name)) => (
            id.parse().map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?,
            name,
            false,
        ),
        None => {
            let name = req
                .user_name
                .as_deref()
                .map(str::trim)
                .filter(|n| !n.is_empty())
                .ok_or(StatusCode::BAD_REQUEST)?;
            (Uuid::new_v4(), name.to_string(), true)
        }
    };

    // Get existing passkeys to exclude
    let existing_passkeys: Vec<Passkey> = if !is_new_user {
        let rows: Vec<(String,)> = sqlx::query_as("SELECT data FROM passkey WHERE user_id = ?")
            .bind(user_id.to_string())
            .fetch_all(&state.db)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        rows.into_iter()
            .filter_map(|(data,)| serde_json::from_str(&data).ok())
            .collect()
    } else {
        vec![]
    };

    let exclude: Option<Vec<CredentialID>> = if existing_passkeys.is_empty() {
        None
    } else {
        Some(
            existing_passkeys
                .iter()
                .map(|p| p.cred_id().clone())
                .collect(),
        )
    };

    let (ccr, reg_state) = state
        .webauthn
        .start_passkey_registration(user_id, &user_name, &user_name, exclude)
        .map_err(|e| {
            tracing::error!(error = %e, "registration start failed");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    let challenge_id = Uuid::new_v4().to_string();
    let context = RegistrationContext {
        webauthn_state: reg_state,
        user_id: user_id.to_string(),
        user_name,
        passkey_name: req.passkey_name,
        is_new_user,
    };
    let state_json =
        serde_json::to_string(&context).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    sqlx::query("INSERT INTO auth_challenge (id, state, kind, expires_at) VALUES (?, ?, 'registration', datetime('now', '+5 minutes'))")
        .bind(&challenge_id)
        .bind(&state_json)
        .execute(&state.db)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(BeginResponse {
        challenge_id,
        options: ccr,
    }))
}

async fn register_complete(
    State(state): State<AppState>,
    auth: MaybeAuthUser,
    jar: CookieJar,
    headers: HeaderMap,
    Json(req): Json<RegisterCompleteRequest>,
) -> Result<(CookieJar, Json<serde_json::Value>), StatusCode> {
    // Fetch and delete challenge (single-use)
    let row: Option<(String,)> = sqlx::query_as(
        "DELETE FROM auth_challenge WHERE id = ? AND kind = 'registration' AND expires_at > datetime('now') RETURNING state",
    )
    .bind(&req.challenge_id)
    .fetch_optional(&state.db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let (state_json,) = row.ok_or(StatusCode::BAD_REQUEST)?;
    let context: RegistrationContext =
        serde_json::from_str(&state_json).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // If not new user, require auth
    if !context.is_new_user && auth.0.is_none() {
        return Err(StatusCode::UNAUTHORIZED);
    }

    let passkey = state
        .webauthn
        .finish_passkey_registration(&req.credential, &context.webauthn_state)
        .map_err(|e| {
            tracing::error!(error = %e, "registration finish failed");
            StatusCode::BAD_REQUEST
        })?;

    // Create user if new — atomic guard ensures only one user can ever be created
    if context.is_new_user {
        let result = sqlx::query(
            "INSERT INTO user (id, name) SELECT ?, ? WHERE NOT EXISTS (SELECT 1 FROM user)",
        )
        .bind(&context.user_id)
        .bind(&context.user_name)
        .execute(&state.db)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        if result.rows_affected() == 0 {
            return Err(StatusCode::CONFLICT);
        }
    }

    // Store passkey
    let passkey_data =
        serde_json::to_string(&passkey).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    sqlx::query("INSERT INTO passkey (user_id, name, data) VALUES (?, ?, ?)")
        .bind(&context.user_id)
        .bind(&context.passkey_name)
        .bind(&passkey_data)
        .execute(&state.db)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if context.is_new_user {
        let token = auth::create_token(&state.jwt_secret, &context.user_id)
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        let cookie =
            auth::session_cookie(token, request_secure_cookie(&headers, state.secure_cookies));
        return Ok((
            jar.add(cookie),
            Json(serde_json::json!({ "success": true })),
        ));
    }
    Ok((jar, Json(serde_json::json!({ "success": true }))))
}

async fn login_begin(
    State(state): State<AppState>,
    Json(req): Json<LoginBeginRequest>,
) -> Result<Json<BeginResponse<RequestChallengeResponse>>, StatusCode> {
    let redirect_origin = normalize_redirect_origin(&state, req.redirect_origin.as_deref())?;
    let redirect_path = redirect_origin
        .as_ref()
        .map(|_| normalize_redirect_path(req.redirect_path.as_deref()));

    sqlx::query("DELETE FROM auth_challenge WHERE expires_at < datetime('now')")
        .execute(&state.db)
        .await
        .ok();

    // Get all passkeys
    let rows: Vec<(String, String)> = sqlx::query_as("SELECT user_id, data FROM passkey")
        .fetch_all(&state.db)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if rows.is_empty() {
        return Err(StatusCode::BAD_REQUEST);
    }

    let user_id = rows[0].0.clone();
    let passkeys: Vec<Passkey> = rows
        .into_iter()
        .filter_map(|(_, data)| serde_json::from_str(&data).ok())
        .collect();

    if passkeys.is_empty() {
        return Err(StatusCode::INTERNAL_SERVER_ERROR);
    }

    let (rcr, auth_state) = state
        .webauthn
        .start_passkey_authentication(&passkeys)
        .map_err(|e| {
            tracing::error!(error = %e, "authentication start failed");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    let challenge_id = Uuid::new_v4().to_string();
    let context = AuthenticationContext {
        webauthn_state: auth_state,
        user_id,
        redirect_origin,
        redirect_path,
    };
    let state_json =
        serde_json::to_string(&context).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    sqlx::query("INSERT INTO auth_challenge (id, state, kind, expires_at) VALUES (?, ?, 'authentication', datetime('now', '+5 minutes'))")
        .bind(&challenge_id)
        .bind(&state_json)
        .execute(&state.db)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(BeginResponse {
        challenge_id,
        options: rcr,
    }))
}

async fn login_complete(
    State(state): State<AppState>,
    jar: CookieJar,
    headers: HeaderMap,
    Json(req): Json<LoginCompleteRequest>,
) -> Result<(CookieJar, Json<serde_json::Value>), StatusCode> {
    // Fetch and delete challenge (single-use)
    let row: Option<(String,)> = sqlx::query_as(
        "DELETE FROM auth_challenge WHERE id = ? AND kind = 'authentication' AND expires_at > datetime('now') RETURNING state",
    )
    .bind(&req.challenge_id)
    .fetch_optional(&state.db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let (state_json,) = row.ok_or(StatusCode::BAD_REQUEST)?;
    let context: AuthenticationContext =
        serde_json::from_str(&state_json).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let auth_result = state
        .webauthn
        .finish_passkey_authentication(&req.credential, &context.webauthn_state)
        .map_err(|e| {
            tracing::error!(error = %e, "authentication finish failed");
            StatusCode::UNAUTHORIZED
        })?;

    // Update the authenticated passkey: persist credential state (counter, backup flags) and last_used
    let rows: Vec<(i64, String)> = sqlx::query_as("SELECT id, data FROM passkey WHERE user_id = ?")
        .bind(&context.user_id)
        .fetch_all(&state.db)
        .await
        .unwrap_or_default();
    for (pk_id, data) in rows {
        if let Ok(mut pk) = serde_json::from_str::<Passkey>(&data)
            && let Some(changed) = pk.update_credential(&auth_result)
        {
            let query = if changed {
                let updated_data =
                    serde_json::to_string(&pk).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
                sqlx::query("UPDATE passkey SET data = ?, last_used = datetime('now') WHERE id = ?")
                    .bind(updated_data)
                    .bind(pk_id)
            } else {
                sqlx::query("UPDATE passkey SET last_used = datetime('now') WHERE id = ?")
                    .bind(pk_id)
            };
            query.execute(&state.db).await.ok();
            break;
        }
    }

    // Issue JWT
    let secure_cookie = request_secure_cookie(&headers, state.secure_cookies);
    let token = auth::create_token(&state.jwt_secret, &context.user_id)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let cookie = auth::session_cookie(token, secure_cookie);

    let user_name: Option<(String,)> = sqlx::query_as("SELECT name FROM user WHERE id = ?")
        .bind(&context.user_id)
        .fetch_optional(&state.db)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let redirect_url = context.redirect_origin.as_deref().and_then(|origin| {
        let path = context.redirect_path.as_deref().unwrap_or("/");
        issue_login_redirect_token(&state, &context.user_id, origin, path)
            .ok()
            .map(|t| redirect_complete_url(origin, &t))
    });

    Ok((
        jar.add(cookie),
        Json(serde_json::json!({
            "success": true,
            "user_name": user_name.map(|u| u.0),
            "redirect_url": redirect_url,
        })),
    ))
}

async fn redirect_start(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(req): Json<RedirectStartRequest>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let target_origin = normalize_redirect_target_origin(&state, &req.redirect_origin)?;
    let target_path = normalize_redirect_path(req.redirect_path.as_deref());
    let token = issue_login_redirect_token(&state, &auth.user_id, &target_origin, &target_path)?;

    Ok(Json(serde_json::json!({
        "redirect_url": redirect_complete_url(&target_origin, &token),
    })))
}

async fn redirect_complete(
    State(state): State<AppState>,
    jar: CookieJar,
    Query(query): Query<RedirectCompleteQuery>,
    headers: HeaderMap,
) -> Result<(CookieJar, Redirect), StatusCode> {
    let mut validation = Validation::default();
    validation.validate_aud = false;

    let claims = decode::<LoginRedirectClaims>(
        &query.token,
        &DecodingKey::from_secret(&state.jwt_secret),
        &validation,
    )
    .map_err(|_| StatusCode::UNAUTHORIZED)?
    .claims;

    if !claims.iss.eq_ignore_ascii_case(&state.rp_origin) {
        return Err(StatusCode::UNAUTHORIZED);
    }

    let fallback_scheme = request_fallback_scheme(&headers, &state.rp_origin);
    let origin = request_origin(&headers, fallback_scheme).ok_or(StatusCode::BAD_REQUEST)?;
    if !claims.aud.eq_ignore_ascii_case(&origin) {
        return Err(StatusCode::UNAUTHORIZED);
    }
    let aud_host = origin_host(&claims.aud).ok_or(StatusCode::UNAUTHORIZED)?;
    if !state.allowed_hosts.contains(&aud_host) {
        return Err(StatusCode::UNAUTHORIZED);
    }

    let token = auth::create_token(&state.jwt_secret, &claims.sub)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let cookie = auth::session_cookie(token, origin.starts_with("https://"));

    Ok((
        jar.add(cookie),
        Redirect::to(&normalize_redirect_path(Some(&claims.path))),
    ))
}

async fn logout(jar: CookieJar) -> CookieJar {
    jar.remove(
        Cookie::build(("den_session", ""))
            .path("/")
            .max_age(time::Duration::ZERO)
            .build(),
    )
}

async fn list_passkeys(
    State(state): State<AppState>,
    auth: AuthUser,
) -> Result<Json<Vec<PasskeyInfo>>, StatusCode> {
    let rows: Vec<(i64, String, String, Option<String>)> =
        sqlx::query_as("SELECT id, name, created, last_used FROM passkey WHERE user_id = ?")
            .bind(&auth.user_id)
            .fetch_all(&state.db)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(
        rows.into_iter()
            .map(|(id, name, created, last_used)| PasskeyInfo {
                id,
                name,
                created,
                last_used,
            })
            .collect(),
    ))
}

async fn rename_passkey(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<i64>,
    Json(req): Json<RenameRequest>,
) -> Result<StatusCode, StatusCode> {
    let result = sqlx::query("UPDATE passkey SET name = ? WHERE id = ? AND user_id = ?")
        .bind(&req.name)
        .bind(id)
        .bind(&auth.user_id)
        .execute(&state.db)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if result.rows_affected() == 0 {
        return Err(StatusCode::NOT_FOUND);
    }

    Ok(StatusCode::NO_CONTENT)
}

async fn delete_passkey(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<i64>,
) -> Result<StatusCode, StatusCode> {
    let result = sqlx::query(
        "DELETE FROM passkey WHERE id = ? AND user_id = ? \
         AND (SELECT COUNT(*) FROM passkey WHERE user_id = ?) > 1",
    )
    .bind(id)
    .bind(&auth.user_id)
    .bind(&auth.user_id)
    .execute(&state.db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if result.rows_affected() > 0 {
        return Ok(StatusCode::NO_CONTENT);
    }
    // Distinguish "not found" from "last passkey"
    let exists: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM passkey WHERE id = ? AND user_id = ?)")
            .bind(id)
            .bind(&auth.user_id)
            .fetch_one(&state.db)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Err(if exists {
        StatusCode::BAD_REQUEST
    } else {
        StatusCode::NOT_FOUND
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_redirect_path_rejects_protocol_relative_and_backslashes() {
        assert_eq!(normalize_redirect_path(Some("//evil.com")), "/");
        assert_eq!(normalize_redirect_path(Some("/\\evil.com")), "/");
    }

    #[test]
    fn normalize_redirect_path_accepts_regular_relative_path() {
        assert_eq!(normalize_redirect_path(Some("/dashboard")), "/dashboard");
    }
}
