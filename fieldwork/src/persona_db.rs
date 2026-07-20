//! Database operations for persona CRUD.
//!
//! Pure CRUD operations against the canonical `personas` table defined in
//! [`crate::db`]. Each function dispatches on [`crate::db::Pool`] to emit the
//! correct SQL dialect.
//!
//! The personas table has 17 columns, exceeding sqlx's 16-element tuple limit,
//! so we use `sqlx::Row::get` with column indices instead of `query_as` tuples.

use crate::db::Pool;
use sqlx::Row;

/// A row from the `personas` table.
#[derive(Debug, Clone)]
pub struct PersonaRow {
    pub id: String,
    pub user_id: String,
    pub username: String,
    pub display_name: String,
    pub bio: String,
    pub bio_html: String,
    pub private_key_pem: String,
    pub public_key_pem: String,
    pub avatar_media_id: Option<i64>,
    pub header_media_id: Option<i64>,
    pub is_locked: bool,
    pub discoverable: bool,
    pub bot: bool,
    pub did_web: Option<String>,
    pub fields_json: String,
    pub created_at: i64,
    pub last_status_at: Option<i64>,
}

pub(crate) const SELECT_COLS_PG: &str = "id::text, user_id::text, username, display_name, bio, bio_html, \
    private_key_pem, public_key_pem, avatar_media_id, header_media_id, \
    is_locked, discoverable, bot, did_web, fields_json::text, created_at, last_status_at";

pub(crate) const SELECT_COLS_SQ: &str = "id, user_id, username, display_name, bio, bio_html, \
    private_key_pem, public_key_pem, avatar_media_id, header_media_id, \
    is_locked, discoverable, bot, did_web, fields_json, created_at, last_status_at";

#[cfg(feature = "db-postgres")]
pub(crate) fn pg_row_to_persona(row: sqlx::postgres::PgRow) -> PersonaRow {
    PersonaRow {
        id: row.get(0),
        user_id: row.get(1),
        username: row.get(2),
        display_name: row.get(3),
        bio: row.get(4),
        bio_html: row.get(5),
        private_key_pem: row.get(6),
        public_key_pem: row.get(7),
        avatar_media_id: row.get(8),
        header_media_id: row.get(9),
        is_locked: row.get(10),
        discoverable: row.get(11),
        bot: row.get(12),
        did_web: row.get(13),
        fields_json: row.get(14),
        created_at: row.get(15),
        last_status_at: row.get(16),
    }
}

#[cfg(feature = "db-sqlite")]
pub(crate) fn sq_row_to_persona(row: sqlx::sqlite::SqliteRow) -> PersonaRow {
    let is_locked: i32 = row.get(10);
    let discoverable: i32 = row.get(11);
    let bot: i32 = row.get(12);
    PersonaRow {
        id: row.get(0),
        user_id: row.get(1),
        username: row.get(2),
        display_name: row.get(3),
        bio: row.get(4),
        bio_html: row.get(5),
        private_key_pem: row.get(6),
        public_key_pem: row.get(7),
        avatar_media_id: row.get(8),
        header_media_id: row.get(9),
        is_locked: is_locked != 0,
        discoverable: discoverable != 0,
        bot: bot != 0,
        did_web: row.get(13),
        fields_json: row.get(14),
        created_at: row.get(15),
        last_status_at: row.get(16),
    }
}

/// Look up a persona by username.
pub async fn get_persona_by_username(
    pool: &Pool,
    username: &str,
) -> Result<Option<PersonaRow>, sqlx::Error> {
    match pool {
        #[cfg(feature = "db-postgres")]
        Pool::Postgres(pg) => {
            let row = sqlx::query(
                &format!("SELECT {SELECT_COLS_PG} FROM personas WHERE username = $1"),
            )
            .bind(username)
            .fetch_optional(pg)
            .await?;
            Ok(row.map(pg_row_to_persona))
        }
        #[cfg(feature = "db-sqlite")]
        Pool::Sqlite(sq) => {
            let row = sqlx::query(
                &format!("SELECT {SELECT_COLS_SQ} FROM personas WHERE username = ?1"),
            )
            .bind(username)
            .fetch_optional(sq)
            .await?;
            Ok(row.map(sq_row_to_persona))
        }
    }
}

/// Look up a persona by ID.
pub async fn get_persona_by_id(
    pool: &Pool,
    id: &str,
) -> Result<Option<PersonaRow>, sqlx::Error> {
    match pool {
        #[cfg(feature = "db-postgres")]
        Pool::Postgres(pg) => {
            let row = sqlx::query(
                &format!("SELECT {SELECT_COLS_PG} FROM personas WHERE id = $1::uuid"),
            )
            .bind(id)
            .fetch_optional(pg)
            .await?;
            Ok(row.map(pg_row_to_persona))
        }
        #[cfg(feature = "db-sqlite")]
        Pool::Sqlite(sq) => {
            let row = sqlx::query(
                &format!("SELECT {SELECT_COLS_SQ} FROM personas WHERE id = ?1"),
            )
            .bind(id)
            .fetch_optional(sq)
            .await?;
            Ok(row.map(sq_row_to_persona))
        }
    }
}

/// Insert a new persona.
pub async fn create_persona(pool: &Pool, row: &PersonaRow) -> Result<(), sqlx::Error> {
    match pool {
        #[cfg(feature = "db-postgres")]
        Pool::Postgres(pg) => {
            sqlx::query(
                "INSERT INTO personas \
                 (id, user_id, username, display_name, bio, bio_html, \
                  private_key_pem, public_key_pem, avatar_media_id, header_media_id, \
                  is_locked, discoverable, bot, did_web, fields_json, created_at, last_status_at) \
                 VALUES ($1::uuid, $2::uuid, $3, $4, $5, $6, $7, $8, $9, $10, \
                         $11, $12, $13, $14, $15::jsonb, $16, $17)",
            )
            .bind(&row.id)
            .bind(&row.user_id)
            .bind(&row.username)
            .bind(&row.display_name)
            .bind(&row.bio)
            .bind(&row.bio_html)
            .bind(&row.private_key_pem)
            .bind(&row.public_key_pem)
            .bind(row.avatar_media_id)
            .bind(row.header_media_id)
            .bind(row.is_locked)
            .bind(row.discoverable)
            .bind(row.bot)
            .bind(&row.did_web)
            .bind(&row.fields_json)
            .bind(row.created_at)
            .bind(row.last_status_at)
            .execute(pg)
            .await?;
        }
        #[cfg(feature = "db-sqlite")]
        Pool::Sqlite(sq) => {
            let is_locked: i32 = if row.is_locked { 1 } else { 0 };
            let discoverable: i32 = if row.discoverable { 1 } else { 0 };
            let bot: i32 = if row.bot { 1 } else { 0 };
            sqlx::query(
                "INSERT INTO personas \
                 (id, user_id, username, display_name, bio, bio_html, \
                  private_key_pem, public_key_pem, avatar_media_id, header_media_id, \
                  is_locked, discoverable, bot, did_web, fields_json, created_at, last_status_at) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, \
                         ?11, ?12, ?13, ?14, ?15, ?16, ?17)",
            )
            .bind(&row.id)
            .bind(&row.user_id)
            .bind(&row.username)
            .bind(&row.display_name)
            .bind(&row.bio)
            .bind(&row.bio_html)
            .bind(&row.private_key_pem)
            .bind(&row.public_key_pem)
            .bind(row.avatar_media_id)
            .bind(row.header_media_id)
            .bind(is_locked)
            .bind(discoverable)
            .bind(bot)
            .bind(&row.did_web)
            .bind(&row.fields_json)
            .bind(row.created_at)
            .bind(row.last_status_at)
            .execute(sq)
            .await?;
        }
    }
    Ok(())
}

/// Update profile fields on a persona. Only non-`None` fields are updated.
pub async fn update_persona_profile(
    pool: &Pool,
    id: &str,
    display_name: Option<&str>,
    bio: Option<&str>,
    bio_html: Option<&str>,
) -> Result<(), sqlx::Error> {
    // ponytail: individual UPDATEs per field; upgrade to a query builder if field count grows
    match pool {
        #[cfg(feature = "db-postgres")]
        Pool::Postgres(pg) => {
            if let Some(dn) = display_name {
                sqlx::query("UPDATE personas SET display_name = $1 WHERE id = $2::uuid")
                    .bind(dn).bind(id).execute(pg).await?;
            }
            if let Some(b) = bio {
                sqlx::query("UPDATE personas SET bio = $1 WHERE id = $2::uuid")
                    .bind(b).bind(id).execute(pg).await?;
            }
            if let Some(bh) = bio_html {
                sqlx::query("UPDATE personas SET bio_html = $1 WHERE id = $2::uuid")
                    .bind(bh).bind(id).execute(pg).await?;
            }
        }
        #[cfg(feature = "db-sqlite")]
        Pool::Sqlite(sq) => {
            if let Some(dn) = display_name {
                sqlx::query("UPDATE personas SET display_name = ?1 WHERE id = ?2")
                    .bind(dn).bind(id).execute(sq).await?;
            }
            if let Some(b) = bio {
                sqlx::query("UPDATE personas SET bio = ?1 WHERE id = ?2")
                    .bind(b).bind(id).execute(sq).await?;
            }
            if let Some(bh) = bio_html {
                sqlx::query("UPDATE personas SET bio_html = ?1 WHERE id = ?2")
                    .bind(bh).bind(id).execute(sq).await?;
            }
        }
    }
    Ok(())
}

/// List all personas.
pub async fn list_personas(pool: &Pool) -> Result<Vec<PersonaRow>, sqlx::Error> {
    match pool {
        #[cfg(feature = "db-postgres")]
        Pool::Postgres(pg) => {
            let rows = sqlx::query(
                &format!("SELECT {SELECT_COLS_PG} FROM personas ORDER BY username"),
            )
            .fetch_all(pg)
            .await?;
            Ok(rows.into_iter().map(pg_row_to_persona).collect())
        }
        #[cfg(feature = "db-sqlite")]
        Pool::Sqlite(sq) => {
            let rows = sqlx::query(
                &format!("SELECT {SELECT_COLS_SQ} FROM personas ORDER BY username"),
            )
            .fetch_all(sq)
            .await?;
            Ok(rows.into_iter().map(sq_row_to_persona).collect())
        }
    }
}

#[cfg(test)]
#[cfg(feature = "db-sqlite")]
mod tests {
    use super::*;
    use crate::db;

    async fn setup() -> Pool {
        use sqlx::sqlite::SqlitePoolOptions;
        let sq = SqlitePoolOptions::new()
            .connect("sqlite::memory:")
            .await
            .unwrap();
        let pool = Pool::Sqlite(sq.clone());
        db::migrate(&pool).await.unwrap();
        sqlx::raw_sql(
            "INSERT INTO users (id, email, display_name, role, created_at) \
             VALUES ('u1', 'test@example.com', 'Test', 'user', 1000);",
        )
        .execute(&sq)
        .await
        .unwrap();
        pool
    }

    fn sample_persona(id: &str, username: &str) -> PersonaRow {
        PersonaRow {
            id: id.into(),
            user_id: "u1".into(),
            username: username.into(),
            display_name: format!("Display {username}"),
            bio: "A test persona".into(),
            bio_html: "<p>A test persona</p>".into(),
            private_key_pem: "-----BEGIN PRIVATE KEY-----\ntest\n-----END PRIVATE KEY-----".into(),
            public_key_pem: "-----BEGIN PUBLIC KEY-----\ntest\n-----END PUBLIC KEY-----".into(),
            avatar_media_id: None,
            header_media_id: None,
            is_locked: false,
            discoverable: true,
            bot: false,
            did_web: None,
            fields_json: "[]".into(),
            created_at: 1000,
            last_status_at: None,
        }
    }

    #[tokio::test]
    async fn create_and_get_by_id() {
        let pool = setup().await;
        let persona = sample_persona("p1", "alice");
        create_persona(&pool, &persona).await.unwrap();

        let fetched = get_persona_by_id(&pool, "p1").await.unwrap().expect("should exist");
        assert_eq!(fetched.id, "p1");
        assert_eq!(fetched.username, "alice");
        assert_eq!(fetched.display_name, "Display alice");
        assert!(!fetched.is_locked);
        assert!(fetched.discoverable);
        assert!(!fetched.bot);
    }

    #[tokio::test]
    async fn get_by_username() {
        let pool = setup().await;
        create_persona(&pool, &sample_persona("p1", "alice")).await.unwrap();

        let fetched = get_persona_by_username(&pool, "alice").await.unwrap().expect("should exist");
        assert_eq!(fetched.id, "p1");
    }

    #[tokio::test]
    async fn get_nonexistent_returns_none() {
        let pool = setup().await;
        assert!(get_persona_by_id(&pool, "nope").await.unwrap().is_none());
        assert!(get_persona_by_username(&pool, "nope").await.unwrap().is_none());
    }

    #[tokio::test]
    async fn update_profile() {
        let pool = setup().await;
        create_persona(&pool, &sample_persona("p1", "alice")).await.unwrap();

        update_persona_profile(
            &pool, "p1",
            Some("New Name"),
            Some("New bio"),
            Some("<p>New bio</p>"),
        )
        .await
        .unwrap();

        let fetched = get_persona_by_id(&pool, "p1").await.unwrap().unwrap();
        assert_eq!(fetched.display_name, "New Name");
        assert_eq!(fetched.bio, "New bio");
        assert_eq!(fetched.bio_html, "<p>New bio</p>");
    }

    #[tokio::test]
    async fn update_partial_fields() {
        let pool = setup().await;
        create_persona(&pool, &sample_persona("p1", "alice")).await.unwrap();

        update_persona_profile(&pool, "p1", Some("Updated"), None, None).await.unwrap();

        let fetched = get_persona_by_id(&pool, "p1").await.unwrap().unwrap();
        assert_eq!(fetched.display_name, "Updated");
        assert_eq!(fetched.bio, "A test persona"); // unchanged
    }

    #[tokio::test]
    async fn list_personas_ordered() {
        let pool = setup().await;
        create_persona(&pool, &sample_persona("p2", "bob")).await.unwrap();
        create_persona(&pool, &sample_persona("p1", "alice")).await.unwrap();

        let all = list_personas(&pool).await.unwrap();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].username, "alice");
        assert_eq!(all[1].username, "bob");
    }

    #[tokio::test]
    async fn list_personas_empty() {
        let pool = setup().await;
        let all = list_personas(&pool).await.unwrap();
        assert!(all.is_empty());
    }

    #[tokio::test]
    async fn boolean_fields_roundtrip() {
        let pool = setup().await;
        let mut persona = sample_persona("p1", "alice");
        persona.is_locked = true;
        persona.discoverable = false;
        persona.bot = true;
        create_persona(&pool, &persona).await.unwrap();

        let fetched = get_persona_by_id(&pool, "p1").await.unwrap().unwrap();
        assert!(fetched.is_locked);
        assert!(!fetched.discoverable);
        assert!(fetched.bot);
    }
}
