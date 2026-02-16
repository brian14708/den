use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::routing::{delete, get, patch, post};
use axum::{Json, Router};
use axum_extra::extract::cookie::{Cookie, CookieJar};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use webauthn_rs::prelude::*;

use crate::auth::{self, AuthUser, MaybeAuthUser};
use crate::state::AppState;

// --- Types ---

#[derive(Serialize)]
struct AuthStatus {
    setup_complete: bool,
    authenticated: bool,
    user_name: Option<String>,
}

#[derive(Deserialize)]
struct RegisterBeginRequest {
    user_name: String,
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

// --- Router ---

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/auth/status", get(status))
        .route("/auth/register/begin", post(register_begin))
        .route("/auth/register/complete", post(register_complete))
        .route("/auth/login/begin", post(login_begin))
        .route("/auth/login/complete", post(login_complete))
        .route("/auth/logout", post(logout))
        .route("/auth/passkeys", get(list_passkeys))
        .route("/auth/passkeys/{id}/name", patch(rename_passkey))
        .route("/auth/passkeys/{id}", delete(delete_passkey))
}

// --- Handlers ---

async fn status(
    State(state): State<AppState>,
    auth: MaybeAuthUser,
) -> Result<Json<AuthStatus>, StatusCode> {
    let user: Option<(String,)> = sqlx::query_as("SELECT name FROM user LIMIT 1")
        .fetch_optional(&state.db)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let setup_complete = user.is_some();
    let authenticated = auth.0.is_some();

    Ok(Json(AuthStatus {
        setup_complete,
        authenticated,
        user_name: if authenticated {
            user.map(|u| u.0)
        } else {
            None
        },
    }))
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
    let existing: Option<(String,)> = sqlx::query_as("SELECT id FROM user LIMIT 1")
        .fetch_optional(&state.db)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // If user exists, require auth (adding additional passkey)
    if existing.is_some() && auth.0.is_none() {
        return Err(StatusCode::UNAUTHORIZED);
    }

    let (user_id, is_new_user) = match &existing {
        Some((id,)) => (
            Uuid::parse_str(id).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?,
            false,
        ),
        None => (Uuid::new_v4(), true),
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
        .start_passkey_registration(user_id, &req.user_name, &req.user_name, exclude)
        .map_err(|e| {
            tracing::error!(error = %e, "registration start failed");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    let challenge_id = Uuid::new_v4().to_string();
    let context = RegistrationContext {
        webauthn_state: reg_state,
        user_id: user_id.to_string(),
        user_name: req.user_name,
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

    // Set session cookie on first setup
    if context.is_new_user {
        let token = auth::create_token(&state.jwt_secret, &context.user_id)
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        let cookie = auth::session_cookie(token, state.secure_cookies);
        return Ok((jar.add(cookie), Json(serde_json::json!({"success": true}))));
    }

    Ok((jar, Json(serde_json::json!({"success": true}))))
}

async fn login_begin(
    State(state): State<AppState>,
) -> Result<Json<BeginResponse<RequestChallengeResponse>>, StatusCode> {
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
    let token = auth::create_token(&state.jwt_secret, &context.user_id)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let cookie = auth::session_cookie(token, state.secure_cookies);

    // Get user name
    let user_name: Option<(String,)> = sqlx::query_as("SELECT name FROM user WHERE id = ?")
        .bind(&context.user_id)
        .fetch_optional(&state.db)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok((
        jar.add(cookie),
        Json(serde_json::json!({
            "success": true,
            "user_name": user_name.map(|u| u.0),
        })),
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
