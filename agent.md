# FrostDAO Agent Guide

## Project Overview

FrostDAO is a Rust implementation of FROST threshold signatures with hierarchical TSS for Bitcoin Taproot. The repository contains a CLI, terminal UI, WebAssembly exports, a small static frontend, and integration tests for DKG, signing, resharing, and NIP-44 encryption.

## Working Rules

- Treat protocol and crypto changes as high risk. Prefer small, reviewable edits with focused tests.
- Do not change public protocol formats, wallet storage formats, or generated address behavior unless the task explicitly requires it.
- Keep secrets, shares, private keys, mnemonics, and nonces out of logs, panics, and test output.
- Preserve user data under `~/.frostdao/`; tests and scripts should avoid relying on or mutating real wallets.
- Prefer existing module boundaries:
  - `src/protocol/` for DKG, signing, reshare, and recovery flows.
  - `src/crypto/` for Birkhoff, HD derivation, helpers, and encryption primitives.
  - `src/btc/` for Bitcoin address, script, Schnorr, and transaction logic.
- `src/tui/` for terminal UI state, screens, and components.
- `frontend/` for browser-facing static assets and wasm consumers.
- `src/nostr/` for relay transport and versioned message envelopes; keep cryptography outside this layer.

## Quality Commands

Run these before handing off code changes when practical:

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
```

The same checks are available as a script:

```bash
./scripts/quality.sh
```

Use the full script before releases or demos:

```bash
./scripts/quality.sh --full
```

Use targeted tests while iterating:

```bash
./scripts/test.sh signing
./scripts/test.sh dkg
./scripts/test.sh reshare
```

Build commands:

```bash
cargo build
./scripts/build.sh
./scripts/wasm-build.sh
```

## Style

- Keep error handling explicit with `Result` and contextual messages where failures cross user-facing boundaries.
- Avoid `unwrap` and `expect` outside tests unless the invariant is local and obvious.
- Prefer typed structs and serde over ad hoc string parsing for protocol payloads.
- Keep frontend JavaScript small and direct; do not introduce a framework without a clear need.
- Add tests for behavioral changes, especially in protocol, Bitcoin, storage, and serialization code.

## Review Focus

When reviewing or improving code quality, prioritize:

1. Security-sensitive correctness issues.
2. Serialization and compatibility regressions.
3. Error handling that can hide failed protocol state.
4. Panics in CLI, TUI, wasm, or network-facing paths.
5. Duplicated logic that can diverge across protocol flows.
