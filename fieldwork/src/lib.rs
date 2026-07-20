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
//! - **`error`** — [`error::AppError`] with Axum `IntoResponse`.
//! - **`auth`** — Password hashing (argon2id) and token hashing (SHA-256).
//! - **`sanitize`** — HTML sanitization and Markdown rendering.
//! - **`content`** — Hashtag, mention, and URL detection with language detection.
//! - **`theme`** — W3C Design Token to CSS custom property compilation.

pub mod circuit_breaker;
pub mod id;
pub mod retry;

#[cfg(feature = "auth")]
pub mod auth;
#[cfg(feature = "content")]
pub mod content;
#[cfg(feature = "error")]
pub mod error;
#[cfg(any(feature = "db-postgres", feature = "db-sqlite"))]
pub mod db;
#[cfg(feature = "sanitize")]
pub mod sanitize;
#[cfg(feature = "theme")]
pub mod theme;
