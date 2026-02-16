use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::Redirect;
use axum::routing::{delete, get, patch, post};
use axum::{Json, Router};
use axum_extra::extract::cookie::{Cookie, CookieJar};
use jsonwebtoken::{DecodingKey, EncodingKey, Header, Validation, decode, encode};
use serde::{Deserialize, Serialize};
use time::{Duration, OffsetDateTime};
use url::form_urlencoded;
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
    let fallback_scheme = if fallback { "https" } else { "http" };
    request_origin(headers, fallback_scheme)
        .map(|origin| origin.starts_with("https://"))
        .unwrap_or(fallback)
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
    if !state.allowed_hosts.contains(host.as_str()) {
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
    if !state.allowed_hosts.contains(host.as_str()) {
        return Err(StatusCode::BAD_REQUEST);
    }
    Ok(normalized)
}

fn normalize_redirect_path(path: Option<&str>) -> String {
    let Some(path) = path.map(str::trim) else {
        return "/".to_string();
    };
    if path.starts_with('/') && !path.starts_with("//") && !path.contains('\\') {
        path.to_string()
    } else {
        "/".to_string()
    }
}

fn redirect_complete_url(target_origin: &str, token: &str) -> String {
    let mut query = form_urlencoded::Serializer::new(String::new());
    query.append_pair("token", token);
    format!(
        "{target_origin}/api/auth/redirect/complete?{}",
        query.finish()
    )
}

fn issue_login_redirect_token(
    state: &AppState,
    user_id: &str,
    target_origin: &str,
    target_path: &str,
) -> Result<String, StatusCode> {
    let now = OffsetDateTime::now_utc();
    let expires_at = now + Duration::seconds(60);

    let claims = LoginRedirectClaims {
        iss: state.rp_origin.clone(),
        aud: target_origin.to_string(),
        sub: user_id.to_string(),
        path: target_path.to_string(),
        iat: now.unix_timestamp(),
        exp: expires_at.unix_timestamp(),
    };

    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(&state.jwt_secret),
    )
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

async fn register_begin(
    State(state): State<AppState>,
    auth: MaybeAuthUser,
    Json(req): Json<RegisterBeginRequest>,
) -> Result<Json<BeginResponse<CreationChallengeResponse>>, StatusCode> {
    // Clean up expired challenges
    sqlx::query("DELETE FROM auth_challenge WHERE expires_at < datetime('now')")
        .execute(&state.db)
        .await
        .ok();

    // Check if user exists
    let existing: Option<(String, String)> = sqlx::query_as("SELECT id, name FROM user LIMIT 1")
        .fetch_optional(&state.db)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // If user exists, require auth (adding additional passkey)
    if existing.is_some() && auth.0.is_none() {
        return Err(StatusCode::UNAUTHORIZED);
    }

    let (user_id, user_name, is_new_user) = match existing {
        Some((id, name)) => (
            Uuid::parse_str(&id).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?,
            name,
            false,
        ),
        None => {
            let user_name = req
                .user_name
                .as_deref()
                .map(str::trim)
                .filter(|name| !name.is_empty())
                .ok_or(StatusCode::BAD_REQUEST)?
                .to_string();
            (Uuid::new_v4(), user_name, true)
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

    let mut jar = jar;

    // Set session cookie on first setup
    if context.is_new_user {
        let secure_cookie = request_secure_cookie(&headers, state.secure_cookies);
        let token = auth::create_token(&state.jwt_secret, &context.user_id)
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        let cookie = auth::session_cookie(token, secure_cookie);
        jar = jar.add(cookie);
    }

    Ok((
        jar,
        Json(serde_json::json!({
            "success": true,
        })),
    ))
}

async fn login_begin(
    State(state): State<AppState>,
    Json(req): Json<LoginBeginRequest>,
) -> Result<Json<BeginResponse<RequestChallengeResponse>>, StatusCode> {
    let redirect_origin = normalize_redirect_origin(&state, req.redirect_origin.as_deref())?;
    let redirect_path = redirect_origin
        .as_ref()
        .map(|_| normalize_redirect_path(req.redirect_path.as_deref()));

    // Clean up expired challenges
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

    // Get user name
    let user_name: Option<(String,)> = sqlx::query_as("SELECT name FROM user WHERE id = ?")
        .bind(&context.user_id)
        .fetch_optional(&state.db)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let redirect_url = if let Some(target_origin) = context.redirect_origin.as_deref() {
        let target_path = context
            .redirect_path
            .as_deref()
            .map(str::to_string)
            .unwrap_or_else(|| "/".to_string());
        let token =
            issue_login_redirect_token(&state, &context.user_id, target_origin, &target_path)?;
        Some(redirect_complete_url(target_origin, &token))
    } else {
        None
    };

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
    if !state.allowed_hosts.contains(aud_host.as_str()) {
        return Err(StatusCode::UNAUTHORIZED);
    }

    let secure_cookie = origin.starts_with("https://");
    let token = auth::create_token(&state.jwt_secret, &claims.sub)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let cookie = auth::session_cookie(token, secure_cookie);
    let redirect_path = normalize_redirect_path(Some(&claims.path));

    Ok((jar.add(cookie), Redirect::to(&redirect_path)))
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

    let passkeys = rows
        .into_iter()
        .map(|(id, name, created, last_used)| PasskeyInfo {
            id,
            name,
            created,
            last_used,
        })
        .collect();

    Ok(Json(passkeys))
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

    if result.rows_affected() == 0 {
        // Distinguish "not found" from "last passkey" for the client
        let exists: bool =
            sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM passkey WHERE id = ? AND user_id = ?)")
                .bind(id)
                .bind(&auth.user_id)
                .fetch_one(&state.db)
                .await
                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

        return Err(if exists {
            StatusCode::BAD_REQUEST
        } else {
            StatusCode::NOT_FOUND
        });
    }

    Ok(StatusCode::NO_CONTENT)
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
