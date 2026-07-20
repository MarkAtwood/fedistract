//! Opinionated building blocks for fediverse servers in Rust.
//!
//! # Core modules (always available)
//!
//! - [`retry`] — Exponential backoff schedule for delivery retries.
//! - [`circuit_breaker`] — Per-domain circuit breaker with eviction.
//! - [`id`] — Snowflake ID generation with backpressure.
//!
//! # Feature-gated modules
//!
//! - **`actor`** — ActivityPub actor document construction and serialization.
//! - **`error`** — [`error::AppError`] with Axum `IntoResponse`.
//! - **`auth`** — Password hashing (argon2id) and token hashing (SHA-256).
//! - **`sanitize`** — HTML sanitization and Markdown rendering.
//! - **`content`** — Hashtag, mention, and URL detection with language detection.
//! - **`theme`** — W3C Design Token to CSS custom property compilation.
//! - **`activities`** — ActivityPub activity and Note construction.
//! - **`collections`** — ActivityPub `OrderedCollection` / `OrderedCollectionPage` builders.
//! - **`fetch`** — SSRF-safe remote actor and object fetching with HTTP Signatures.
//! - **`inbox`** — Inbox POST validation: size limits, digest verification, host checks.
//! - **`nodeinfo`** — NodeInfo 2.1 data types and discovery document construction.
//! - **`webfinger`** — RFC 7033 WebFinger JRD types and response construction.

pub mod circuit_breaker;
pub mod id;
pub mod retry;

#[cfg(feature = "activities")]
pub mod activities;
#[cfg(feature = "actor")]
pub mod actor;
#[cfg(feature = "auth")]
pub mod auth;
#[cfg(feature = "collections")]
pub mod collections;
#[cfg(feature = "content")]
pub mod content;
#[cfg(feature = "error")]
pub mod error;
#[cfg(any(feature = "db-postgres", feature = "db-sqlite"))]
pub mod actor_cache;
#[cfg(any(feature = "db-postgres", feature = "db-sqlite"))]
pub mod admin_db;
#[cfg(any(feature = "db-postgres", feature = "db-sqlite"))]
pub mod cards_db;
#[cfg(any(feature = "db-postgres", feature = "db-sqlite"))]
pub mod conversations_db;
#[cfg(any(feature = "db-postgres", feature = "db-sqlite"))]
pub mod db;
#[cfg(any(feature = "db-postgres", feature = "db-sqlite"))]
pub mod delivery_db;
#[cfg(any(feature = "db-postgres", feature = "db-sqlite"))]
pub mod domain_blocks_db;
#[cfg(any(feature = "db-postgres", feature = "db-sqlite"))]
pub mod filters_db;
#[cfg(any(feature = "db-postgres", feature = "db-sqlite"))]
pub mod follow_requests_db;
#[cfg(any(feature = "db-postgres", feature = "db-sqlite"))]
pub mod followed_tags_db;
#[cfg(any(feature = "db-postgres", feature = "db-sqlite"))]
pub mod followers_db;
#[cfg(any(feature = "db-postgres", feature = "db-sqlite"))]
pub mod follows_db;
#[cfg(any(feature = "db-postgres", feature = "db-sqlite"))]
pub mod idempotency_db;
#[cfg(any(feature = "db-postgres", feature = "db-sqlite"))]
pub mod interactions_db;
#[cfg(any(feature = "db-postgres", feature = "db-sqlite"))]
pub mod invite_codes_db;
#[cfg(any(feature = "db-postgres", feature = "db-sqlite"))]
pub mod lists_db;
#[cfg(any(feature = "db-postgres", feature = "db-sqlite"))]
pub mod markers_db;
#[cfg(any(feature = "db-postgres", feature = "db-sqlite"))]
pub mod media_db;
#[cfg(feature = "inbox")]  // reuses sha2 + base64 deps from inbox feature
pub mod media_integrity;
#[cfg(any(feature = "db-postgres", feature = "db-sqlite"))]
pub mod mentions_db;
#[cfg(any(feature = "db-postgres", feature = "db-sqlite"))]
pub mod notifications_db;
#[cfg(any(feature = "db-postgres", feature = "db-sqlite"))]
pub mod oauth_db;
#[cfg(any(feature = "db-postgres", feature = "db-sqlite"))]
pub mod persona_db;
#[cfg(any(feature = "db-postgres", feature = "db-sqlite"))]
pub mod post_tags_db;
#[cfg(any(feature = "db-postgres", feature = "db-sqlite"))]
pub mod posts_db;
#[cfg(any(feature = "db-postgres", feature = "db-sqlite"))]
pub mod push_db;
#[cfg(any(feature = "db-postgres", feature = "db-sqlite"))]
pub mod relay;
#[cfg(any(feature = "db-postgres", feature = "db-sqlite"))]
pub mod scheduled_db;
#[cfg(any(feature = "db-postgres", feature = "db-sqlite"))]
pub mod search_db;
#[cfg(any(feature = "db-postgres", feature = "db-sqlite"))]
pub mod tenant_db;
#[cfg(any(feature = "db-postgres", feature = "db-sqlite"))]
pub mod timeline_db;
#[cfg(any(feature = "db-postgres", feature = "db-sqlite"))]
pub mod relationship_db;
#[cfg(any(feature = "db-postgres", feature = "db-sqlite"))]
pub mod remote_posts_db;
#[cfg(feature = "fetch")]
pub mod fetch_remote;
#[cfg(feature = "inbox")]
pub mod inbox;
#[cfg(feature = "nodeinfo")]
pub mod nodeinfo;
#[cfg(feature = "sanitize")]
pub mod sanitize;
#[cfg(feature = "theme")]
pub mod theme;
#[cfg(any(feature = "db-postgres", feature = "db-sqlite"))]
pub mod webauthn_db;
#[cfg(feature = "webfinger")]
pub mod webfinger;
