# Agent Instructions — fedistract

Shared crate extraction workspace. Code extracted here becomes public crates
(`cavage-httpsig`, `ssrf-guard`, `fieldwork`) consumed by smallhold, broadside, and gaja.

## Read First

Before doing any work:
1. Run `bd ready` — check for open issues
2. Read `EXTRACTION-PLAN.md` for the full design context
3. Read the source module in the donor project before extracting

## Source Projects

| Project | Path | Purpose |
|---------|------|---------|
| smallhold | `~/PROJECT/smallhold` | Single-user AP server, full Mastodon Client API |
| broadside | `~/PROJECT/broadside` | One-way AP publisher for organizations |
| gaja | `~/PROJECT/gaja` | Multi-tenant AP server (evolved from smallhold) |

## Defensive Programming

These crates face the open internet via their consumers. Every function that processes
federated input, HTTP headers, URLs, or media is an attack surface.

**Defensive defaults:**
- Validate and reject at the boundary. Do not pass unsanitized data deeper.
- Enforce size limits on every input: request bodies, JSON fields, string lengths, collection sizes, media dimensions. Define constants, not magic numbers.
- Never trust `Content-Type` headers from remote servers. Sniff and verify.
- HTML-sanitize all federated content (ammonia) before storing. Sanitize on ingest, not on render.
- Fail closed. If signature verification is ambiguous, reject. If a field is missing, reject.
- No `.unwrap()` in any code path reachable from a network request.
- Log enough to detect attacks but never log secrets, tokens, or full request bodies at INFO level.

## Test Coverage

Target 100% test coverage on all non-trivial code.

**What to test:**
- Every public function. Every error path. Every boundary condition.
- Every input validation rule — valid input accepted AND invalid input rejected with correct error.
- Test hostile inputs: strings at/past length limits, null bytes, unicode edge cases (ZWJ, RTL overrides, homoglyphs), deeply nested objects, oversized arrays.

**Test quality rules:**
- Tests must have an independent oracle. Never use the code under test to generate expected values.
- For crypto (signatures, HMAC): use test vectors from OpenSSL, pyca/cryptography, or RFCs.
- Regression test before fix: test must fail before the fix and pass after.

## Cargo Quality Gates

Run these before closing any code-touching issue:

```bash
cargo fmt --all                          # commit result if anything changes
typos src/                               # cargo install typos-cli
cargo clippy --all-features -- -D warnings
cargo test --all-targets
```

For pre-publish:
```bash
cargo hack check --feature-powerset --no-dev-deps   # cargo install cargo-hack
RUSTDOCFLAGS="--cfg docsrs -D warnings" cargo +nightly doc --no-deps --all-features
cargo publish --dry-run
```

## Code Review Gate

**MANDATORY**: After every logical chunk of code is generated or modified, immediately run `/codereview` against it **3 times** before moving on. A "logical chunk" is a function, module, endpoint, or coherent set of changes — not individual lines. All findings from `/codereview` must be filed as beads issues (tagged with `codereview`). Do not proceed to the next chunk until all 3 review passes are complete and findings are addressed or tracked.

## Non-Interactive Shell Commands

**ALWAYS use non-interactive flags** with file operations to avoid hanging on confirmation prompts.

```bash
cp -f source dest           # NOT: cp source dest
mv -f source dest           # NOT: mv source dest
rm -f file                  # NOT: rm file
rm -rf directory            # NOT: rm -r directory
```

## Subagent Guidance

- Spawn subagents for parallel work on different crates/modules.
- Never spawn two subagents that edit the same file — serialize those.
- Each subagent reads only the files it needs. Do not load the full codebase.
- If a subagent hits the same error three times without progress, stop and escalate.

## Beads Issue Tracker

This project uses **bd (beads)** for issue tracking. Run `bd prime` for full workflow context.

```bash
bd ready              # Find available work
bd show <id>          # View issue details
bd update <id> --claim  # Claim work atomically
bd close <id>         # Complete work
bd dolt push          # Push beads data to remote
```

**Beads is the only task and planning tool.** Do NOT use:
- TodoWrite / markdown TODO lists
- Scratchpad or audit files
- MEMORY.md or any other markdown file as a knowledge store

Use `bd remember` for persistent knowledge and `bd create` for all task tracking.

## Session Completion

**Mandatory sequence** when ending a session:

```bash
# If code changed:
cargo fmt --all && cargo clippy --all-features -- -D warnings && cargo test --all-targets
# Always:
bd close <completed-ids>
git pull --rebase
bd dolt push
git push
git status  # MUST show "up to date with origin"
```
