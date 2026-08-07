# Security

ViewIt is an **offline** file viewer: files are opened locally and (by default)
not transmitted anywhere. This document covers the plugin catalog trust model,
dependency policy, and how to report issues.

## Plugin catalog trust model

`plugins/catalog.signed.json` is the signed manifest of downloadable plugins.
Trust is the foundation of the plugin distribution path:

- The catalog is signed with an Ed25519 key; the public half is
  `scripts/.catalog-signing-key.pub` (git-ignored on disk but expected on the
  distribution host/build).
- The **private** key `scripts/.catalog-signing-key.priv` is never committed
  and never sent to CI. CI only **verifies** signatures; it never signs.
- Clients must reject a catalog whose signature the pinned public key does not
  verify. See [`plugins/DISTRIBUTION_DESIGN.md`](plugins/DISTRIBUTION_DESIGN.md).

If a private signing key is exposed, rotate it immediately (see below).

## Reporting a vulnerability

**Do not open a public issue for security bugs.** Reach out privately to the
maintainers via the repository owner (GitHub issue → private / email as linked
from the repository homepage). Include:

- Affected version(s) / commit.
- A minimal reproducer (file format or input that triggers it) — input files
  are preferred to code, since ViewIt's parsers are the attack surface.
- Impact (e.g. memory unsafety via a format parser, plugin identity confusion).

Expected response: acknowledgement within 5 working days, and a fix/coordination
milestone in the following release. Standalone parsers are isolated per
`crates/fmt-*`, so a parse bug in one format should never compromise others.

## Dependency policy

- The workspace pins Rust via `rust-toolchain.toml` and `.cargo/config.toml`.
- `cargo deny` (config: `deny.toml`) enforces the license allowlist and blocks
  known-vulnerable versions; it runs in CI.
- Semantic-version-sensitive crates such as `zip` are pinned exactly in
  `[workspace.dependencies]`; dependabot (`dependabot.yml`) proposes updates for
  both npm and cargo.
- Runtime parsing of untrusted formats should prefer parsers that are
  memory-safe by construction and tested against malformed inputs.

## Key rotation (maintainers)

1. Generate a new keypair (`crypto/keygen` — see `sign-catalog.py`).
2. Publish the new `.pub` to every client + distribution host.
3. Re-sign `catalog.json` and ship the new `catalog.signed.json`.
4. Revoke and destroy the old `.priv`.
