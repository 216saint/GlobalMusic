//! Amizades: pedido por @usuário → aceitar (pedindo de volta) ou recusar.

use axum::extract::{Path, State};
use axum::Json;
use serde::Serialize;

use crate::auth::{now, AuthUser};
use crate::error::{ApiError, ApiResult};
use crate::users::PublicUser;
use crate::AppState;

#[derive(Serialize)]
pub struct FriendList {
    friends: Vec<PublicUser>,
    /// Pedidos que chegaram (esperando você aceitar).
    incoming: Vec<PublicUser>,
    /// Pedidos que você mandou (esperando a outra pessoa).
    outgoing: Vec<PublicUser>,
}

async fn user_id_by_username(state: &AppState, username: &str) -> ApiResult<i64> {
    let row: Option<(i64,)> = sqlx::query_as("SELECT id FROM users WHERE username = ?")
        .bind(username.trim_start_matches('@'))
        .fetch_optional(&state.db)
        .await?;
    row.map(|r| r.0).ok_or_else(|| ApiError::NotFound(format!("@{username} não existe")))
}

pub async fn list(State(state): State<AppState>, AuthUser(me): AuthUser) -> ApiResult<Json<FriendList>> {
    const COLUMNS: &str = "u.username, u.display_name, u.avatar_url";
    let friends = sqlx::query_as::<_, PublicUser>(&format!(
        "SELECT {COLUMNS} FROM friendships f JOIN users u
           ON u.id = CASE WHEN f.requester_id = ? THEN f.addressee_id ELSE f.requester_id END
         WHERE f.status = 'accepted' AND (f.requester_id = ? OR f.addressee_id = ?)
         ORDER BY u.display_name COLLATE NOCASE"
    ))
    .bind(me)
    .bind(me)
    .bind(me)
    .fetch_all(&state.db)
    .await?;
    let incoming = sqlx::query_as::<_, PublicUser>(&format!(
        "SELECT {COLUMNS} FROM friendships f JOIN users u ON u.id = f.requester_id
         WHERE f.status = 'pending' AND f.addressee_id = ? ORDER BY f.created_at DESC"
    ))
    .bind(me)
    .fetch_all(&state.db)
    .await?;
    let outgoing = sqlx::query_as::<_, PublicUser>(&format!(
        "SELECT {COLUMNS} FROM friendships f JOIN users u ON u.id = f.addressee_id
         WHERE f.status = 'pending' AND f.requester_id = ? ORDER BY f.created_at DESC"
    ))
    .bind(me)
    .fetch_all(&state.db)
    .await?;
    Ok(Json(FriendList { friends, incoming, outgoing }))
}

/// Manda pedido. Se a outra pessoa já tinha pedido, isto **aceita**.
pub async fn add(State(state): State<AppState>, AuthUser(me): AuthUser, Path(username): Path<String>) -> ApiResult<Json<FriendList>> {
    let other = user_id_by_username(&state, &username).await?;
    if other == me {
        return Err(ApiError::BadRequest("você não pode se adicionar".into()));
    }
    let accepted = sqlx::query(
        "UPDATE friendships SET status = 'accepted' WHERE requester_id = ? AND addressee_id = ? AND status = 'pending'",
    )
    .bind(other)
    .bind(me)
    .execute(&state.db)
    .await?
    .rows_affected();
    if accepted == 0 {
        // INSERT OR IGNORE: pedir de novo, ou pedir a quem já é amigo, não faz nada.
        sqlx::query(
            "INSERT OR IGNORE INTO friendships (requester_id, addressee_id, status, created_at)
             SELECT ?, ?, 'pending', ? WHERE NOT EXISTS (
               SELECT 1 FROM friendships WHERE requester_id = ? AND addressee_id = ?)",
        )
        .bind(me)
        .bind(other)
        .bind(now())
        .bind(other)
        .bind(me)
        .execute(&state.db)
        .await?;
    }
    list(State(state), AuthUser(me)).await
}

/// Desfaz amizade, recusa um pedido recebido ou cancela um pedido enviado.
pub async fn remove(State(state): State<AppState>, AuthUser(me): AuthUser, Path(username): Path<String>) -> ApiResult<Json<FriendList>> {
    let other = user_id_by_username(&state, &username).await?;
    sqlx::query(
        "DELETE FROM friendships WHERE (requester_id = ? AND addressee_id = ?) OR (requester_id = ? AND addressee_id = ?)",
    )
    .bind(me)
    .bind(other)
    .bind(other)
    .bind(me)
    .execute(&state.db)
    .await?;
    list(State(state), AuthUser(me)).await
}
