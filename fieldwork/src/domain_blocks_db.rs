//! Database operations for instance-level domain blocks.
//!
//! Pure CRUD against the canonical `domain_blocks` table.

use crate::db::Pool;

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct DomainBlockRow {
    pub domain: String,
    pub severity: String,
    pub reject_media: bool,
    pub reject_reports: bool,
    pub reason: String,
    pub created_at: i64,
}

/// Block a domain. Replaces any existing block for the same domain.
pub async fn block_domain(pool: &Pool, row: &DomainBlockRow) -> Result<(), sqlx::Error> {
    match pool {
        #[cfg(feature = "db-postgres")]
        Pool::Postgres(pg) => {
            sqlx::query(
                "INSERT INTO domain_blocks (domain, severity, reject_media, reject_reports, reason, created_at) \
                 VALUES ($1, $2, $3, $4, $5, $6) \
                 ON CONFLICT (domain) DO UPDATE SET \
                 severity = $2, reject_media = $3, reject_reports = $4, reason = $5, created_at = $6",
            )
            .bind(&row.domain)
            .bind(&row.severity)
            .bind(row.reject_media)
            .bind(row.reject_reports)
            .bind(&row.reason)
            .bind(row.created_at)
            .execute(pg)
            .await?;
        }
        #[cfg(feature = "db-sqlite")]
        Pool::Sqlite(sq) => {
            sqlx::query(
                "INSERT OR REPLACE INTO domain_blocks \
                 (domain, severity, reject_media, reject_reports, reason, created_at) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            )
            .bind(&row.domain)
            .bind(&row.severity)
            .bind(row.reject_media)
            .bind(row.reject_reports)
            .bind(&row.reason)
            .bind(row.created_at)
            .execute(sq)
            .await?;
        }
    }
    Ok(())
}

/// Remove a domain block.
pub async fn unblock_domain(pool: &Pool, domain: &str) -> Result<(), sqlx::Error> {
    match pool {
        #[cfg(feature = "db-postgres")]
        Pool::Postgres(pg) => {
            sqlx::query("DELETE FROM domain_blocks WHERE domain = $1")
                .bind(domain)
                .execute(pg)
                .await?;
        }
        #[cfg(feature = "db-sqlite")]
        Pool::Sqlite(sq) => {
            sqlx::query("DELETE FROM domain_blocks WHERE domain = ?1")
                .bind(domain)
                .execute(sq)
                .await?;
        }
    }
    Ok(())
}

/// Check if a domain is blocked. Returns the block row if found.
pub async fn is_blocked(
    pool: &Pool,
    domain: &str,
) -> Result<Option<DomainBlockRow>, sqlx::Error> {
    match pool {
        #[cfg(feature = "db-postgres")]
        Pool::Postgres(pg) => {
            sqlx::query_as(
                "SELECT domain, severity, reject_media, reject_reports, reason, created_at \
                 FROM domain_blocks WHERE domain = $1",
            )
            .bind(domain)
            .fetch_optional(pg)
            .await
        }
        #[cfg(feature = "db-sqlite")]
        Pool::Sqlite(sq) => {
            sqlx::query_as(
                "SELECT domain, severity, reject_media, reject_reports, reason, created_at \
                 FROM domain_blocks WHERE domain = ?1",
            )
            .bind(domain)
            .fetch_optional(sq)
            .await
        }
    }
}

/// List all domain blocks.
pub async fn list_blocks(pool: &Pool) -> Result<Vec<DomainBlockRow>, sqlx::Error> {
    match pool {
        #[cfg(feature = "db-postgres")]
        Pool::Postgres(pg) => {
            sqlx::query_as(
                "SELECT domain, severity, reject_media, reject_reports, reason, created_at \
                 FROM domain_blocks ORDER BY created_at DESC",
            )
            .fetch_all(pg)
            .await
        }
        #[cfg(feature = "db-sqlite")]
        Pool::Sqlite(sq) => {
            sqlx::query_as(
                "SELECT domain, severity, reject_media, reject_reports, reason, created_at \
                 FROM domain_blocks ORDER BY created_at DESC",
            )
            .fetch_all(sq)
            .await
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
        pool
    }

    fn make_block(domain: &str) -> DomainBlockRow {
        DomainBlockRow {
            domain: domain.to_string(),
            severity: "suspend".to_string(),
            reject_media: true,
            reject_reports: false,
            reason: "spam".to_string(),
            created_at: 1000,
        }
    }

    #[tokio::test]
    async fn block_and_check() {
        let pool = setup().await;
        block_domain(&pool, &make_block("evil.example")).await.unwrap();

        let found = is_blocked(&pool, "evil.example").await.unwrap().unwrap();
        assert_eq!(found.severity, "suspend");
        assert!(found.reject_media);
    }

    #[tokio::test]
    async fn not_blocked_returns_none() {
        let pool = setup().await;
        assert!(is_blocked(&pool, "good.example").await.unwrap().is_none());
    }

    #[tokio::test]
    async fn block_replaces_existing() {
        let pool = setup().await;
        block_domain(&pool, &make_block("evil.example")).await.unwrap();

        let mut updated = make_block("evil.example");
        updated.severity = "silence".to_string();
        updated.reason = "updated reason".to_string();
        block_domain(&pool, &updated).await.unwrap();

        let found = is_blocked(&pool, "evil.example").await.unwrap().unwrap();
        assert_eq!(found.severity, "silence");
        assert_eq!(found.reason, "updated reason");
    }

    #[tokio::test]
    async fn unblock_removes() {
        let pool = setup().await;
        block_domain(&pool, &make_block("evil.example")).await.unwrap();
        unblock_domain(&pool, "evil.example").await.unwrap();
        assert!(is_blocked(&pool, "evil.example").await.unwrap().is_none());
    }

    #[tokio::test]
    async fn list_blocks_returns_all() {
        let pool = setup().await;
        block_domain(&pool, &make_block("a.example")).await.unwrap();

        let mut b = make_block("b.example");
        b.created_at = 2000;
        block_domain(&pool, &b).await.unwrap();

        let blocks = list_blocks(&pool).await.unwrap();
        assert_eq!(blocks.len(), 2);
        // Ordered by created_at DESC
        assert_eq!(blocks[0].domain, "b.example");
    }
}
