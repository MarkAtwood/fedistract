//! Database persistence layer for fieldwork fediverse building blocks.
//!
//! This crate owns all sqlx-dependent modules extracted from `fieldwork`.
//! Pure ActivityPub logic lives in the `fieldwork` crate; this crate provides
//! the database schema, pool abstraction, and CRUD operations.
//!
//! # Features
//!
//! - **`postgres`** — PostgreSQL support via sqlx.
//! - **`sqlite`** — SQLite support via sqlx.
//! - **`serde`** — Serde support for relay types.

#[cfg(any(feature = "postgres", feature = "sqlite"))]
pub mod actor_cache;
#[cfg(any(feature = "postgres", feature = "sqlite"))]
pub mod admin_db;
#[cfg(any(feature = "postgres", feature = "sqlite"))]
pub mod cards_db;
#[cfg(any(feature = "postgres", feature = "sqlite"))]
pub mod conversations_db;
#[cfg(any(feature = "postgres", feature = "sqlite"))]
pub mod db;
#[cfg(any(feature = "postgres", feature = "sqlite"))]
pub mod delivery_db;
#[cfg(any(feature = "postgres", feature = "sqlite"))]
pub mod domain_blocks_db;
#[cfg(any(feature = "postgres", feature = "sqlite"))]
pub mod filters_db;
#[cfg(any(feature = "postgres", feature = "sqlite"))]
pub mod follow_requests_db;
#[cfg(any(feature = "postgres", feature = "sqlite"))]
pub mod followed_tags_db;
#[cfg(any(feature = "postgres", feature = "sqlite"))]
pub mod followers_db;
#[cfg(any(feature = "postgres", feature = "sqlite"))]
pub mod follows_db;
#[cfg(any(feature = "postgres", feature = "sqlite"))]
pub mod idempotency_db;
#[cfg(any(feature = "postgres", feature = "sqlite"))]
pub mod interactions_db;
#[cfg(any(feature = "postgres", feature = "sqlite"))]
pub mod invite_codes_db;
#[cfg(any(feature = "postgres", feature = "sqlite"))]
pub mod lists_db;
#[cfg(any(feature = "postgres", feature = "sqlite"))]
pub mod markers_db;
#[cfg(any(feature = "postgres", feature = "sqlite"))]
pub mod media_db;
#[cfg(any(feature = "postgres", feature = "sqlite"))]
pub mod mentions_db;
#[cfg(any(feature = "postgres", feature = "sqlite"))]
pub mod notifications_db;
#[cfg(any(feature = "postgres", feature = "sqlite"))]
pub mod oauth_db;
#[cfg(any(feature = "postgres", feature = "sqlite"))]
pub mod persona_db;
#[cfg(any(feature = "postgres", feature = "sqlite"))]
pub mod post_tags_db;
#[cfg(any(feature = "postgres", feature = "sqlite"))]
pub mod posts_db;
#[cfg(any(feature = "postgres", feature = "sqlite"))]
pub mod push_db;
#[cfg(any(feature = "postgres", feature = "sqlite"))]
pub mod relay;
#[cfg(any(feature = "postgres", feature = "sqlite"))]
pub mod relationship_db;
#[cfg(any(feature = "postgres", feature = "sqlite"))]
pub mod remote_posts_db;
#[cfg(any(feature = "postgres", feature = "sqlite"))]
pub mod scheduled_db;
#[cfg(any(feature = "postgres", feature = "sqlite"))]
pub mod search_db;
#[cfg(any(feature = "postgres", feature = "sqlite"))]
pub mod tenant_db;
#[cfg(any(feature = "postgres", feature = "sqlite"))]
pub mod timeline_db;
#[cfg(any(feature = "postgres", feature = "sqlite"))]
pub mod webauthn_db;
