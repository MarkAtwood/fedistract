# fedistract

Shared Rust building blocks for the fediverse. Powers three servers — [smallhold](https://github.com/MarkAtwood/smallhold), [gaja](https://github.com/MarkAtwood/gaja), and [broadside](https://github.com/MarkAtwood/broadside) — from one codebase.

36,000 lines of Rust. 782 tests. 9 crates.

None of the servers ship a web frontend. They are pure API servers — users connect with existing fediverse clients (Phanpy, Elk, Ivory, Tusky, Jerboa, etc.). Server-rendered HTML is limited to profile pages and photo galleries for browser visitors.

---

## Workspace

```
fedistract/
├── fieldwork/          Pure AP logic, API types, content processing (6,500 lines, 224 tests)
├── fieldwork-db/       Schema, migrations, CRUD — SQLite + Postgres (25,900 lines, 441 tests)
├── cavage-httpsig/     HTTP Signature signing/verification (612 lines, 17 tests)
├── ssrf-guard/         Private IP blocking, DNS rebinding defense (349 lines, 26 tests)
├── fedi-did/           DID: did:scid, did:key, did:web, did:plc + BIP-39 recovery (913 lines, 24 tests)
├── fedi-integrity/     Linked Data Integrity proofs, FEP-8b32 (410 lines, 13 tests)
├── fedi-e2ee/          Encrypted DMs: X25519 + XChaCha20-Poly1305 (343 lines, 11 tests)
├── fedi-provenance/    Content origin proofs with Ed25519 (337 lines, 10 tests)
├── fedi-ucan/          UCAN capability delegation tokens (506 lines, 16 tests)
└── contexts/           JSON-LD context documents for FEPs
```

---

## fieldwork — pure logic, no database

ActivityPub logic and API type definitions. Feature-gated — only pull in what you need. No sqlx dependency.

### Core modules (always available)

| Module | What it does |
|--------|-------------|
| `id` | Snowflake ID generation (i64, time-sortable) with backpressure |
| `retry` | Exponential backoff schedule for delivery retries |
| `circuit_breaker` | Per-domain circuit breaker with automatic eviction |

### Feature-gated modules

| Feature | Module | What it does |
|---------|--------|-------------|
| `actor` | `actor` | ActivityPub actor document construction (Person, Service, Group) |
| `activities` | `activities` | Activity + Note construction (Create, Follow, Accept, Announce, etc.) |
| `collections` | `collections` | OrderedCollection / OrderedCollectionPage builders with pagination |
| `content` | `content` | Hashtag, mention, URL detection + language detection (whatlang) |
| `sanitize` | `sanitize` | HTML sanitization (ammonia) + Markdown→HTML (pulldown-cmark) |
| `sanitize` | `util` | Shared helpers: `epoch_to_iso` (chrono), `slugify`, `render_markdown`, `render_photo_grid`, `now_secs` |
| `auth` | `auth` | Password hashing (argon2id), token hashing (SHA-256) |
| `inbox` | `inbox` | Inbox POST validation: digest verification (constant-time), size limits, host checks |
| `inbox` | `media_integrity` | SRI hash compute/verify for content-addressed media |
| `fetch` | `fetch_remote` | SSRF-safe remote actor/object fetching with HTTP Signatures |
| `media` | `media_processing` | Image resize, EXIF strip, GIF bomb protection, blurhash generation |
| `video` | `video` | FFmpeg command builder for HLS transcoding (pure logic, no subprocess) |
| `podcast` | `podcast_rss` | RSS 2.0 XML generator with iTunes namespace |
| `theme` | `theme` | W3C Design Token → CSS custom property compilation |
| `nodeinfo` | `nodeinfo` | NodeInfo 2.1 response construction |
| `webfinger` | `webfinger` | RFC 7033 WebFinger JRD response construction |
| `profile` | `profile` | Mastodon Account/CredentialAccount types + builders |

### Client API type modules

Each fediverse client protocol has a types module with request/response structs, route path constants, and serialization tests:

| Feature | Module | Protocol | Types |
|---------|--------|----------|-------|
| `mastodon-api` | `mastodon_api` | Mastodon Client API | Status, MediaAttachment, Mention, Tag, Relationship, Context, StatusBuilder |
| `funkwhale-api` | `funkwhale_api` | Funkwhale REST API | TrackResponse, ChannelResponse, AlbumResponse, PlaylistResponse, PaginatedResponse |
| `bookwyrm-api` | `bookwyrm_api` | Bookwyrm REST API | BookResponse, ShelfSummaryResponse, ReviewResponse, UserReadingResponse |
| `writefreely-api` | `writefreely_api` | WriteFreely REST API | PostResponse, CollectionResponse, WfResponse wrapper, CreatePostRequest |

The `full` feature enables everything:

```toml
fieldwork = { version = "0.1", features = ["full"] }
```

---

## fieldwork-db — database persistence

Schema, migrations, and CRUD operations. Supports SQLite and Postgres via a `Pool` enum that abstracts the backend. Zero SQL in server business logic.

### Schema

Single canonical schema shared by all three servers. Current version: **12**.

Migration system handles:
- Fresh databases (full schema creation)
- Incremental migrations (version 1→12)
- Legacy upgrades (pre-versioned smallhold and broadside databases)
- Server-specific extra migrations

### Database modules (40 modules)

| Category | Modules | What they cover |
|----------|---------|----------------|
| **Core** | `posts_db`, `persona_db`, `tenant_db`, `oauth_db` | Posts, personas, users, OAuth tokens |
| **Social** | `followers_db`, `follows_db`, `follow_requests_db`, `interactions_db`, `relationship_db` | Follow graph, favourites, boosts, blocks, mutes |
| **Content** | `media_db`, `post_tags_db`, `mentions_db`, `cards_db`, `conversations_db` | Media, hashtags, mentions, link previews, threads |
| **Discovery** | `search_db`, `trending_db`, `timeline_db` | Full-text search, trending, timeline construction |
| **Notifications** | `notifications_db`, `push_db` | Notification storage, Web Push subscriptions |
| **Moderation** | `domain_blocks_db`, `filters_db`, `admin_db` | Domain blocks, keyword filters, admin ops |
| **Federation** | `delivery_db`, `actor_cache`, `remote_posts_db`, `relay` | Delivery queue, remote actor cache, relay management |
| **Lists & Tags** | `lists_db`, `followed_tags_db`, `markers_db` | User lists, hashtag following, read position markers |
| **Media modes** | `albums_db`, `videos_db`, `audio_db` | Photo albums, video channels, audio tracks/playlists/podcasts |
| **Reading** | `books_db`, `articles_db` | Book shelves/reviews/ratings, long-form articles/collections |
| **Other** | `communities_db`, `reactions_db`, `scheduled_db`, `idempotency_db`, `invite_codes_db`, `webauthn_db` | Lemmy communities, emoji reactions, scheduled posts, idempotency keys, invites, WebAuthn |

### API type conversions

The `api-convert` feature enables `From<&DbRow> for ApiResponse` impls, so server handlers use `.into()` instead of manual field mapping:

```toml
fieldwork-db = { version = "0.1", features = ["sqlite", "api-convert"] }
```

```rust
// Without api-convert:
let response = TrackResponse { id: row.id, title: row.title.clone(), ... };

// With api-convert:
let response: TrackResponse = (&row).into();
```

---

## Cryptography crates

### cavage-httpsig

HTTP Signature signing and verification per draft-cavage-http-signatures-11. Used for ActivityPub federation authentication.

- Signs requests with RSA-SHA256
- Verifies inbound signatures with public key lookup
- Supports `(request-target)`, `host`, `date`, `digest` signed headers
- 8KB signature header size limit (DoS protection)

### fedi-did

Decentralized Identifier support for portable fediverse identity.

| DID Method | What it does |
|-----------|-------------|
| `did:scid` | Self-certifying IDs with Ed25519. Genesis document, key rotation, chain verification. Primary method. |
| `did:key` | Ephemeral key-based DIDs from Ed25519 public keys |
| `did:web` | Domain-hosted DID documents (`/.well-known/did.json`) |
| `did:plc` | AT Protocol PLC directory integration (opt-in) |

BIP-39 mnemonic recovery: 24-word phrase backs up the Ed25519 signing key. Users can recover their identity on any server.

### fedi-integrity

Linked Data Integrity proofs (FEP-8b32). Sign and verify ActivityPub objects with Ed25519 + JCS canonicalization.

### fedi-e2ee

End-to-end encrypted direct messages. Ed25519 → X25519 key conversion, ECDH key agreement, XChaCha20-Poly1305 encryption with HKDF key derivation.

### fedi-provenance

Content origin proofs. Ed25519 signatures over content hashes prove authorship independent of the server that hosts the content.

### fedi-ucan

UCAN (User Controlled Authorization Network) capability delegation tokens. EdDSA-signed JWTs with attenuation and delegation chains.

### ssrf-guard

Server-Side Request Forgery protection for outbound HTTP:

- Blocks private IPs (RFC 1918, link-local, loopback, IPv6-mapped)
- DNS rebinding defense (resolves DNS before connecting)
- Custom reqwest DNS resolver integration
- No redirect following

---

## FEPs

Draft Fediverse Enhancement Proposals authored from this project:

| FEP | Title | File |
|-----|-------|------|
| FEP-b64f | DID actors (did:scid as primary identifier) | `fep-b64f-did-actors.md` |
| FEP-d4c5 | Compact delivery with IPFS content addressing | `fep-d4c5-compact-delivery.md` |
| FEP-9a14 | Pull-based sync protocol | `fep-9a14-pull-based-sync.md` |
| FEP (draft) | Universal abstract/summary/description for ActivityStreams | `fep-post-abstract.md` |
| FEP (draft) | Explicit contentWarning with categories and severity | `fep-content-warning.md` |

JSON-LD context documents for all FEPs are hosted at `contexts/*.jsonld` and referenced by URL from ActivityPub documents.

---

## The three servers

| Server | What | Users | Database | Personality |
|--------|------|-------|----------|-------------|
| [**smallhold**](https://github.com/MarkAtwood/smallhold) | Solo operator instance | 1 | SQLite | Personal fediverse presence. One admin, multiple personas. |
| [**gaja**](https://github.com/MarkAtwood/gaja) | Multi-tenant platform | Many | Postgres + SQLite | Hosting provider / SaaS. Full user isolation, quotas, admin API. |
| [**broadside**](https://github.com/MarkAtwood/broadside) | Broadcast-only | 0 | SQLite | Org announcements. Publish via CLI/RSS/webhook. No login. |

All three share fieldwork + fieldwork-db. Server-specific code is thin handler glue — auth extraction and route wiring. Each server supports eight client API protocols simultaneously:

Mastodon, Pixelfed, Lemmy, PeerTube, Misskey, Funkwhale, Bookwyrm, WriteFreely.

---

## Building

```bash
cargo build --release
cargo test --workspace
```

Per-crate tests:

```bash
cargo test -p fieldwork --features full          # 224 tests
cargo test -p fieldwork-db --features sqlite     # 441 tests
cargo test -p fedi-did                           # 24 tests
cargo test -p fedi-integrity                     # 13 tests
cargo test -p fedi-e2ee                          # 11 tests
cargo test -p fedi-provenance                    # 10 tests
cargo test -p fedi-ucan                          # 16 tests
cargo test -p cavage-httpsig                     # 17 tests
cargo test -p ssrf-guard                         # 26 tests
```

## License

MIT OR Apache-2.0 (library crates). Server binaries are AGPL-3.0.
