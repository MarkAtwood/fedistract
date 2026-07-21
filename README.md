# fedistract

Shared building blocks for three fediverse servers: [smallhold](https://github.com/MarkAtwood/smallhold), [gaja](https://github.com/MarkAtwood/gaja), and [broadside](https://github.com/MarkAtwood/broadside).

One codebase, three server personalities, eight client API faces.

## Crates

| Crate | Purpose |
|-------|---------|
| **fieldwork** | Pure ActivityPub logic — actor documents, activities, collections, content processing, media handling, API type definitions. No database dependency. |
| **fieldwork-db** | Database persistence — schema, migrations, CRUD operations. SQLite and Postgres via sqlx. Pool enum abstracts the backend. |
| **cavage-httpsig** | HTTP Signature signing and verification (draft-cavage-11). |
| **ssrf-guard** | SSRF protection — private IP blocking, DNS rebinding defense. |
| **fedi-did** | Decentralized Identifiers — did:scid, did:key, did:web, did:plc. BIP-39 mnemonic recovery. |
| **fedi-integrity** | Linked Data Integrity proofs (FEP-8b32). |
| **fedi-e2ee** | Encrypted DMs — X25519 ECDH + XChaCha20-Poly1305. |
| **fedi-provenance** | Content origin proofs with Ed25519 signatures. |
| **fedi-ucan** | UCAN capability delegation tokens. |

## Eight API faces

Every server speaks multiple client APIs through shared fieldwork modules:

| API | What it does | Types module | DB module |
|-----|-------------|--------------|-----------|
| **Mastodon** | Microblogging — statuses, timelines, notifications | `mastodon_api` + `profile` | `posts_db`, `interactions_db`, etc. |
| **Pixelfed** | Photo sharing — albums, trending, discover | `profile` | `albums_db`, `media_db` |
| **Lemmy** | Communities — threaded posts, comments, voting | (inline JSON) | `communities_db` |
| **PeerTube** | Video — channels, uploads, HLS | (inline JSON) | `videos_db` |
| **Misskey** | Alt microblogging — emoji reactions, POST-only | (inline JSON) | `reactions_db` |
| **Funkwhale** | Audio — tracks, albums, channels, playlists | `funkwhale_api` | `audio_db` |
| **Bookwyrm** | Books — shelves, reviews, ratings | `bookwyrm_api` | `books_db` |
| **WriteFreely** | Long-form — articles, collections, markdown | `writefreely_api` | `articles_db` |

## Schema

Single canonical schema across all servers. 40+ tables covering posts, media, followers, communities, videos, audio, books, articles, and more. Migration system with legacy upgrade support for existing smallhold and broadside databases.

Current schema version: **12**.

## FEPs

Draft Fediverse Enhancement Proposals authored from this project:

- **FEP-b64f** — DID actors (did:scid as primary identifier)
- **FEP-d4c5** — Compact delivery with IPFS content addressing
- **FEP-9a14** — Pull-based sync protocol
- **FEP (post abstract)** — Universal abstract/summary/description for all ActivityStreams objects
- **FEP (content warning)** — Explicit contentWarning field with categories and severity

JSON-LD contexts hosted at `contexts/*.jsonld`.

## The three servers

| Server | Personality | Users | Database | Target |
|--------|------------|-------|----------|--------|
| **smallhold** | Solo operator | 1 (admin) | SQLite | Personal fediverse presence |
| **gaja** | Multi-tenant | Many | Postgres + SQLite | Hosting provider / SaaS |
| **broadside** | Broadcast-only | 0 (no client API) | SQLite | Org announcements |

All three share the same fieldwork/fieldwork-db crates. Server-specific code is thin handler glue — auth extraction and route wiring.

None of the servers ship a web frontend. They are pure API servers — users connect with existing fediverse clients (Phanpy, Elk, Ivory, Tusky, Jerboa, etc.). Server-rendered HTML is limited to profile pages and photo galleries for browser visitors.

## Building

```bash
cargo build --release
cargo test
```

Individual crate tests:

```bash
cargo test -p fieldwork --features full
cargo test -p fieldwork-db --features sqlite
cargo test -p fedi-did
cargo test -p fedi-integrity
```

## License

MIT OR Apache-2.0 (library crates). AGPL-3.0 (server binaries).
