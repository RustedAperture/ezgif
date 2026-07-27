use std::collections::HashSet;

use sqlx::SqlitePool;
use uuid::Uuid;

use super::users::StoredIdentity;

type StoredIdentityRow = (String, String, String, Option<String>, Option<String>);

#[derive(Clone)]
pub struct AdminRepository {
    pool: SqlitePool,
}

#[derive(Clone, Debug)]
pub struct AdminUserRecord {
    pub id: Uuid,
    pub username: Option<String>,
    pub display_name: Option<String>,
    pub role: String,
    pub identities: Vec<StoredIdentity>,
    pub permissions: HashSet<String>,
}

impl AdminRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn search_users(
        &self,
        query: Option<&str>,
        limit: i64,
    ) -> Result<Vec<AdminUserRecord>, sqlx::Error> {
        let limit = limit.clamp(1, 50);
        let query = query.map(str::trim).filter(|query| !query.is_empty());
        let rows: Vec<(String, Option<String>, Option<String>, String)> = match query {
            Some(query) => {
                let pattern = format!("%{query}%");
                sqlx::query_as(
                    "SELECT id, username, display_name, role
                     FROM users
                     WHERE username LIKE ?
                        OR EXISTS (
                            SELECT 1
                            FROM user_identities
                            WHERE user_identities.user_id = users.id
                              AND (provider || ':' || provider_user_id) LIKE ?
                        )
                     ORDER BY username, display_name, id
                     LIMIT ?",
                )
                .bind(&pattern)
                .bind(&pattern)
                .bind(limit)
                .fetch_all(&self.pool)
                .await?
            }
            None => {
                sqlx::query_as(
                    "SELECT id, username, display_name, role
                     FROM users
                     ORDER BY username, display_name, id
                     LIMIT ?",
                )
                .bind(limit)
                .fetch_all(&self.pool)
                .await?
            }
        };

        let mut users = Vec::with_capacity(rows.len());
        for (id, username, display_name, role) in rows {
            let id = Uuid::parse_str(&id).map_err(|error| sqlx::Error::Decode(Box::new(error)))?;
            users.push(AdminUserRecord {
                id,
                username,
                display_name,
                role,
                identities: self.identities_for_user(id).await?,
                permissions: self.permissions_for_user(id).await?,
            });
        }
        Ok(users)
    }

    pub async fn set_role(&self, user_id: Uuid, role: &str) -> Result<bool, sqlx::Error> {
        let result =
            sqlx::query("UPDATE users SET role = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?")
                .bind(role)
                .bind(user_id.to_string())
                .execute(&self.pool)
                .await?;
        Ok(result.rows_affected() > 0)
    }

    pub async fn set_permission(
        &self,
        user_id: Uuid,
        permission: &str,
        enabled: bool,
    ) -> Result<(), sqlx::Error> {
        if enabled {
            sqlx::query(
                "INSERT OR IGNORE INTO user_permissions (user_id, permission) VALUES (?, ?)",
            )
            .bind(user_id.to_string())
            .bind(permission)
            .execute(&self.pool)
            .await?;
        } else {
            sqlx::query("DELETE FROM user_permissions WHERE user_id = ? AND permission = ?")
                .bind(user_id.to_string())
                .bind(permission)
                .execute(&self.pool)
                .await?;
        }
        Ok(())
    }

    async fn identities_for_user(&self, user_id: Uuid) -> Result<Vec<StoredIdentity>, sqlx::Error> {
        let rows: Vec<StoredIdentityRow> = sqlx::query_as(
            "SELECT id, provider, provider_user_id, display_name, avatar_url
             FROM user_identities
             WHERE user_id = ?
             ORDER BY linked_at",
        )
        .bind(user_id.to_string())
        .fetch_all(&self.pool)
        .await?;

        rows.into_iter()
            .map(
                |(id, provider, provider_user_id, display_name, avatar_url)| {
                    Ok(StoredIdentity {
                        id: Uuid::parse_str(&id)
                            .map_err(|error| sqlx::Error::Decode(Box::new(error)))?,
                        provider,
                        provider_user_id,
                        display_name,
                        avatar_url,
                    })
                },
            )
            .collect()
    }

    async fn permissions_for_user(&self, user_id: Uuid) -> Result<HashSet<String>, sqlx::Error> {
        let permissions =
            sqlx::query_scalar("SELECT permission FROM user_permissions WHERE user_id = ?")
                .bind(user_id.to_string())
                .fetch_all(&self.pool)
                .await?;
        Ok(permissions.into_iter().collect())
    }
}
