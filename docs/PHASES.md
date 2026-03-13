# phases.md

This document lists the project phases for CloakFund. Each phase includes clear objectives, tasks the agent should perform, required outputs (deliverables), acceptance criteria, dependencies, risks and mitigations, and exact instructions the agent can act on. No time estimates are given — use the phases as sequential milestones and mark each as complete when its acceptance criteria are met.

---

## Phase 0 — Preparation & Access
**Objective**
Prepare environment, credentials, and project references so implementation proceeds without blockers.

**Tasks**
- Collect and store all API keys and credentials (BitGo dev, Fileverse API key, Base RPC/WSS, ENS access if needed).
- Create a secure secrets store (environment variables or vault) and document required env names.
- Create test accounts and pre-fund test wallets on Base testnet.
- Confirm repo structure and branch policy, create `phases` branch for the agent work.
- Generate a minimal `.env.example` with required variables (no secrets).

**Deliverables**
- `.env.example`
- `SECRETS_SETUP.md` with where keys are stored and access process
- Test wallet addresses and explorer links

**Acceptance criteria**
- All required credentials available to the agent in the agreed secret mechanism.
- Test wallets are funded and links verified.

**Dependencies**
- Access to BitGo dev portal, Fileverse account, Base testnet faucet.

**Risks & mitigations**
- Missing keys: escalate immediately and provide mock endpoints to proceed.
- API rate limits: provision fallback pre-recorded responses for demos.

**Agent instructions**
- Add `SECRETS_SETUP.md` into repo root with exact env variable names.
- Create test wallet address list file `docs/test_wallets.md`.

---

## Phase 1 — Core Rust Backend: Stealth Generator Module
**Objective**
Implement the cryptographic core that generates one-time stealth addresses and ephemeral keys.

**Tasks**
- Design public API for stealth module: functions to generate ephemeral keypair, compute shared secret given recipient public key, derive EOA address from shared secret, return ephemeral public key (or announcement).
- Implement crypto using `k256` or equivalent, HKDF, and keccak.
- Add unit tests that validate deterministic vectors (use fixed seeds).
- Add safe zeroization of ephemeral secrets after use.
- Document the API in `docs/CRYPTOGRAPHY.md` and `docs/RUST_BACKEND_DESIGN.md`.

**Deliverables**
- Rust crate/module `stealth` with documented public interface.
- Unit tests and example CLI usage: `cargo run --bin stealth_demo -- --recipient-pub <hex>` prints pay address and ephem pub.
- `CRYPTO_TEST_VECTORS.md` with test inputs/outputs.

**Acceptance criteria**
- Unit tests pass.
- Demo CLI produces expected address and ephemeral pub for test vectors.
- No secrets are left persisted in logs.

**Dependencies**
- None beyond Rust toolchain and chosen crates.

**Risks & mitigations**
- Incorrect scalar math: validate against reference JS/TS implementation or existing ERC-5564 test vectors.
- Non-deterministic outputs: include fixed-seed unit tests.

**Agent instructions**
- Implement `stealth::generate(ephemeral_seed?, recipient_pub_hex) -> {address, ephem_pub_hex}`.
- Add tests under `rust-backend/tests/stealth_tests.rs`.

---

## Phase 2 — Deposit Watcher / Indexer
**Objective**
Build the watcher that subscribes to Base events and matches incoming payments to generated paylinks.

**Tasks**
- Choose provider: WebSocket/WS via ethers-rs provider to Base WSS.
- Implement event subscription and parsing for `Transfer` events and native ETH receipts.
- Implement DB schema: `paylinks`, `ephemeral_addresses`, `deposits` tables.
- Implement deposit confirmation logic with configurable confirmation count.
- Implement idempotent processing and retries (use transactional DB writes).
- Implement websocket push or server-sent events (SSE) to frontend for deposit notifications.

**Deliverables**
- `watcher` module integrated into backend with logs.
- Convex schema definitions for DB tables.
- Endpoint `GET /api/v1/deposit-status?paylink_id=...`.
- Demonstration script that creates a paylink, sends a test tx, and shows deposit record.

**Acceptance criteria**
- Watcher reliably detects test deposits posted to ephemeral addresses and persists records.
- Deposit records include tx hash, block number, paylink id, and confirmation status.
- Watcher handles restart without duplicating processing.

**Dependencies**
- Base WSS endpoint, Convex DB.

**Risks & mitigations**
- Network disconnections: implement reconnect logic with backoff.
- Missed events: support historical log sync via block range scan.

**Agent instructions**
- Create Convex schema definitions and include connection guidance in `BUILD.md`.
- Provide a `watcher_test.sh` script demonstrating detection.

---

## Phase 3 — Paylink API & Persistence
**Objective**
Expose backend endpoints to generate paylinks and persist mappings between ENS identity and ephemeral addresses.

**Tasks**
- Implement `POST /api/v1/paylink` input: `{ens_name, optional_metadata}` returning `{paylink_id, ephemeral_address, ephem_pub}`.
- Persist mapping in DB.
- Input validation: ensure ENS ownership verification option (off-chain check or simple resolve).
- Implement `GET /api/v1/paylink/:id` to retrieve status and ephemeral addresses.

**Deliverables**
- Paylink API endpoints with OpenAPI-style documentation (swagger or `docs/API.md`).
- Integration tests showing end-to-end: generate paylink → return ephemeral address.

**Acceptance criteria**
- API returns deterministic ephemeral address and stores record.
- API is secured (basic auth or token) for paylink creation if required.

**Dependencies**
- Stealth module and DB.

**Risks & mitigations**
- Abuse of paylink endpoint: add rate limits and validation.

**Agent instructions**
- Add `docs/API.md` with example requests/responses.
- Implement proper logging and input sanitation.

---

## Phase 4 — BitGo Consolidation Flow (Sweeper)
**Objective**
Implement consolidation process to move funds from ephemeral addresses to BitGo MPC vault.

**Tasks**
- Design consolidation jobs: manual trigger (button) and auto-sweep rules (configurable).
- Implement `consolidator` module that constructs consolidation transactions and submits signing requests to BitGo REST.
- Implement job state machine: `pending -> signed -> broadcasted -> confirmed -> failed`.
- Implement co-sign notification handling (polling or webhook).
- Implement safe idempotency and reconciliation: check UTXO-like finality and prevent double-spend.

**Deliverables**
- Consolidation job API: `POST /api/v1/consolidate?paylink_id=...` and job status endpoints.
- Logs and reconciliation report showing consolidation txs (on explorer).

**Acceptance criteria**
- Consolidation job goes from pending to broadcasted on testnet with BitGo response (or simulated if BitGo dev sandbox has restrictions).
- Backend records signed tx hash and updates DB.

**Dependencies**
- BitGo dev credentials and understanding of their REST signing flow.

**Risks & mitigations**
- BitGo rate limits or sandbox differences: prepare simulated signed payloads for demo fallback.

**Agent instructions**
- Implement robust job retry and error handling. Log only job ids; do not log keys.
- Add `docs/BITGO_FLOW.md` describing API calls and required headers.

---

## Phase 5 — Frontend Integration (Next.js TSX)
**Objective**
Connect frontend to backend: generate paylinks in UI, show deposit status, and display aggregated balance and receipt links.

**Tasks**
- Implement wallet connection (wagmi/ethers.js or viem) and ENS input UI.
- Call `POST /api/v1/paylink` and display ephemeral address / QR / copyable link to payer.
- Subscribe to deposit events via SSE/WebSocket to update UI in real time.
- Display aggregated balance computed by querying `GET /api/v1/deposit-status` or aggregated endpoint.

**Deliverables**
- Frontend pages: `PaylinkGenerator`, `Dashboard`, `DepositHistory`, `ReceiptViewer`.
- Integration tests or manual test instructions to validate flow end-to-end.

**Acceptance criteria**
- UI can generate a paylink and display ephemeral address.
- UI updates when deposit is detected and shows aggregated balance (sum of confirmed deposits).

**Dependencies**
- Paylink API and watcher.

**Risks & mitigations**
- UI desync: ensure events include idempotent state and finality checks.

**Agent instructions**
- Provide exact API contract and sample responses in `docs/API.md` for frontend implementers.
- Add `docs/FRONTEND_RUN.md` with local dev steps and env vars.

---

## Phase 6 — Fileverse Receipts Integration
**Objective**
Encrypt and store receipts in Fileverse, return pointers to frontend which decrypts client-side.

**Tasks**
- Define receipt schema: paylink id, amount, timestamp, memo (optional), ephem_pub, tx_hash.
- Implement server-side symmetric encryption with per-paylink or per-user keys derived securely. Decide whether keys are user-provided or derived (document tradeoffs).
- Implement `POST /api/v1/receipts` to encrypt payload and upload to Fileverse via REST; store pointer in DB.
- Implement `GET /api/v1/receipts?paylink_id=...` to return pointers.
- Update frontend to fetch pointer and decrypt using user key or prompt for decryption key.

**Deliverables**
- `receipt` module in Rust that encrypts and uploads to Fileverse.
- Sample encrypted receipt and decryption demo in frontend.

**Acceptance criteria**
- Receipt stored in Fileverse and frontend successfully decrypts and renders receipt content.
- Backend never stores plaintext receipts.

**Dependencies**
- Fileverse API key, receipt encryption design.

**Risks & mitigations**
- Key management complexity: prefer user-controlled keys or ephemeral viewing keys; document implications and fallback to user-passphrase.

**Agent instructions**
- Add `docs/RECEIPT_KEY_MGMT.md` describing chosen approach and security implications.
- Provide sample JS decrypt utility for frontend.

---

## Phase 7 — ENS & Smart Contract Helpers
**Objective**
Implement optional minimal on-chain helpers (PaymentResolver) and ENS-related workflow.

**Tasks**
- Implement `PaymentResolver.sol` if required to anchor paylink pointer to ENS (contenthash or metadata pointer).
- Provide deployment script or ethers-rs deploy function.
- Implement backend ENS helper functions (resolve ownership, optionally write contenthash if user opts in).
- Update frontend to offer optional "anchor to ENS" option.

**Deliverables**
- Solidity contract source (if used) and ABI.
- Deployment instructions or script.
- `docs/ENS_USAGE.md` explaining pros/cons.

**Acceptance criteria**
- Contract deployed on Base testnet and backend can query it.
- ENS anchor update is optional and documented.

**Dependencies**
- ENS registration (test or main), deployer key.

**Risks & mitigations**
- Gas costs for anchors: warn user and make it opt-in.

**Agent instructions**
- Keep contracts minimal. Provide tests for resolver read/write.

---

## Phase 8 — Monitoring, Testing, & Hardening
**Objective**
System testing, security checks, and operational readiness for demo.

**Tasks**
- Unit tests for Rust modules (stealth, watcher, consolidator).
- Integration tests that simulate full flow with test transactions.
- Security review: confirm no private keys logged, ephemeral keys zeroized.
- Add health endpoints and Prometheus metrics.
- Create backup demo assets (recorded flows, pre-funded txs) as fallbacks.

**Deliverables**
- Test suite results and instructions to run them.
- `HEALTH_CHECKS.md` with endpoints and expected outputs.
- Demo fallback assets in `docs/demo_fallbacks/`.

**Acceptance criteria**
- All unit tests pass.
- Integration test runs show deposit detection and receipt storage.
- Health endpoints return OK.

**Dependencies**
- DB and test wallets.

**Risks & mitigations**
- Test failures: isolate modules and run component-level tests.

**Agent instructions**
- Provide `make test` or `cargo test` commands and CI configuration sample (`.github/workflows/ci.yml`).

---

## Phase 9 — Demo Preparation & Presentation Materials
**Objective**
Prepare the live demo script, slides, and backup materials for judges.

**Tasks**
- Finalize 3-minute demo script and checklist of steps (exact UI clicks and data to show).
- Create 2-slide summary: (1) Problem + Solution + Prize mapping, (2) Architecture + What to open in demo.
- Prepare fallback recorded video or pre-run transactions and receipts.
- Prepare short README summary for judges and reviewers.

**Deliverables**
- `docs/DEMO.md` final version.
- `slides/demo_slides.pdf`.
- Fallback video `docs/demo_fallbacks/demo.mp4`.

**Acceptance criteria**
- Demo script rehearsed and all necessary endpoints accessible.
- Fallback artifacts tested on a separate machine.

**Agent instructions**
- Produce final `docs/DEMO.md` in the repo root and attach `slides/demo_slides.pdf`.

---

## Phase 10 — Post-demo & Handoff
**Objective**
Tidy codebase, prepare handoff notes, and document next steps.

**Tasks**
- Merge development branches and tag release for hackathon deliverable.
- Create `docs/HANDOFF.md` listing remaining work and how to continue.
- Archive secrets used in demo and rotate keys if necessary.
- Prepare a list of suggested next features and estimated complexity.

**Deliverables**
- `Handoff.md`
- Release tag and changelog
- Rotated/archived keys note in `SECRETS_SETUP.md`

**Acceptance criteria**
- Repo clean, clear handoff notes exist, and CI/CD (if any) is configured.

**Agent instructions**
- Produce `docs/HANDOFF.md` with remaining TODOs and who to contact.

---

## QA & Acceptance Checklist (global)
- [ ] All required env variables documented in `.env.example`.
- [ ] `stealth` module unit tests pass.
- [ ] Watcher correctly detects deposits for ephemeral addresses.
- [ ] Paylink API returns ephemeral address and persists entry.
- [ ] Frontend displays paylink and updates on deposit.
- [ ] Receipt encryption/upload to Fileverse works and frontend decrypts.
- [ ] Consolidation job transitions recorded and tx hash available.
- [ ] Health endpoints return OK.
- [ ] Demo script validated; fallback assets prepared.

---

## Communication & Reporting for Agents
- Commit messages: `feat/<area>: short description` or `fix/<area>: short description`.
- Create PR for each phase completion with summary in description and link to demo artifacts.
- When blocked, add a short issue with logs and steps tried, and move to next phase tasks that can proceed.
- Use `docs/PHASE_STATUS.md` to mark phase state: `not_started`, `in_progress`, `blocked`, `done` and short notes.

---

## Final instructions to the agent (actionable)
1. Clone repo, create branch `feature/phases-implementation`.
2. Complete Phase 0 tasks and push `SECRETS_SETUP.md` and `docs/test_wallets.md`.
3. Implement Phase 1 stealth module with unit tests and push as PR titled: `feat(stealth): implement stealth address generator`.
4. Run watcher skeleton and confirm detection with a test tx; update PRs for each phase.
5. For any external API that is unavailable, create a mock endpoint in `rust-backend/mocks/` and document how to switch to real API in `docs/SECRETS_SETUP.md`.
6. After each phase PR, attach demo evidence (screenshots or terminal output).

---

End of phases.md
