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
//! - **`profile`** — Profile metadata types and validation.
//! - **`media`** — Image processing: MIME sniffing, validation, resize, EXIF strip, blurhash.
//! - **`video`** — FFmpeg command builder for HLS transcoding (pure logic, no subprocess).
//! - **`podcast`** — Podcast RSS 2.0 feed generation (iTunes namespace).
//! - **`funkwhale-api`** — Funkwhale-compatible REST API route builders.

pub mod circuit_breaker;
pub mod id;
pub mod retry;
#[cfg(feature = "sanitize")]
pub mod util;

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
#[cfg(feature = "fetch")]
pub mod fetch_remote;
#[cfg(feature = "inbox")]
pub mod inbox;
#[cfg(feature = "inbox")]
pub mod media_integrity;
#[cfg(feature = "media")]
pub mod media_processing;
#[cfg(feature = "nodeinfo")]
pub mod nodeinfo;
#[cfg(feature = "profile")]
pub mod profile;
#[cfg(feature = "sanitize")]
pub mod sanitize;
#[cfg(feature = "theme")]
pub mod theme;
#[cfg(feature = "video")]
pub mod video;
#[cfg(feature = "podcast")]
pub mod podcast_rss;
#[cfg(feature = "funkwhale-api")]
pub mod funkwhale_api;
#[cfg(feature = "bookwyrm-api")]
pub mod bookwyrm_api;
#[cfg(feature = "writefreely-api")]
pub mod writefreely_api;
#[cfg(feature = "webfinger")]
pub mod webfinger;
