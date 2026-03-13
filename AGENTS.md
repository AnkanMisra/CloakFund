# CloakFund Agent Guide

## Tech Stack
- **Frontend/Data**: Convex (TypeScript)
- **Backend/Blockchain**: Rust (`rust-backend/`)

## Build/Lint/Test Commands
- **Convex Dev**: `npm run convex:dev`
- **Convex Push**: `npx convex push`
- **Rust Build**: `cd rust-backend && cargo build`
- **Rust Tests**: `cd rust-backend && cargo test`
- **Single Rust Test**: `cd rust-backend && cargo test <test_name>`
- **Rust Linting**: `cd rust-backend && cargo clippy -- -D warnings`
- **Rust Formatting**: `cd rust-backend && cargo fmt`

## Code Guidelines

### Rust (`rust-backend/`)
- **Error Handling**: Use `anyhow` for application-level errors and `thiserror` for library-level errors. Avoid `.unwrap()` or `.expect()` unless in tests.
- **Async**: Use `tokio` for all I/O and heavy blockchain operations.
- **Logging**: Use the `tracing` crate for structured logging instead of `println!`.
- **Formatting & Linting**: Strictly adhere to `rustfmt` and ensure no `clippy` warnings.
- **Architecture**: Organize code logically (`models`, `watcher`, `sweeper`, `api`). 
- **Blockchain**: Handle stealth addressing and cryptography securely using `ethers` and `k256`.

### Convex (TypeScript)
- **Typing**: Use strong typing with Convex `v` validators in schemas and function arguments/returns.
- **Schemas**: Keep `schema.ts` as the single source of truth for the data model. Avoid `v.any()` where possible.
- **Integration**: Ensure schema parity with Rust models (`rust-backend/src/models.rs`).

## Security
- **Never commit secrets**. Use `.env` files (refer to `.env.example`).
- Ensure sensitive blockchain operations (like key derivations) are handled securely without logging sensitive key material.