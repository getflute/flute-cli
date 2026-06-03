# Flute CLI — Design Spec

**Date:** 2026-06-02
**Status:** Approved for planning
**Source spec:** `aurora-payments/luna` → `context/app-specs/FEATURE-FLUTE-CLI.md`
**Reference implementation:** `/Users/chad.lung/aurora-payments/flute-webhooks` (`flute-webhooks-cli`)

## 1. Goal & Scope

Build `flute` — a cross-platform command-line tool for the Flute payments platform,
in Rust (edition 2024). The CLI serves both humans (`--output table`) and machines /
AI agents (`--output json`, plus an `agents.md` contract).

### In scope

- Auth & environments (OAuth2 client_credentials, OS keychain, profiles).
- Command groups: **transactions (card), ACH, customers/vault, POS/terminals/devices,
  settlements, subscriptions, ISV tokens**, plus **utility** (ping, version, update, completion).
- Output contract: `table | json | quiet`, JSON envelope, semantic exit codes, agent error envelope.
- Cross-platform distribution: cargo-dist binaries, GitHub Actions release, Homebrew tap,
  shell installer, `flute update` self-update.
- `agents.md` machine contract for a downstream MCP project.

### Out of scope (explicit)

- **§6 Webhooks** (`flute listen`, `webhooks …`).
- **TUI** (`flute tui`).
- **OpenAPI codegen** — clients are hand-written against the OpenAPI doc.
- API surface present in the OpenAPI doc but not in the source spec's command list:
  Invoices, LineItems, QuickPayments, Categories, PaymentSessions, Configurations,
  TransactionSettings, AffiliateMerchants/Permissions.

### Hard constraints

- Zero `cargo clippy -- -D warnings` warnings.
- No `unsafe` blocks anywhere.
- Sub-200ms cold start; single self-contained binary; no runtime deps.

## 2. Environments & Auth

### Profiles

| Profile | API base URL | OAuth token URL |
|---|---|---|
| `sandbox` (default) | `https://sandbox.api.uat.flute.com` | `https://sandbox.oauth.api.uat.flute.com/oauth2/token` |
| `production` (alias `prod`) | `https://api.arise.risewithaurora.com` | `https://oauth.arise.risewithaurora.com/oauth2/token` |

> **Decision:** the `sandbox` profile points at the **UAT** URLs until the real sandbox
> environment ships. Swapping to real sandbox URLs is a one-line config change with no
> code impact. The base URL is the **host only** (no path prefix); each endpoint carries
> its full documented path (`/pay-api/v1/...`, `/pos-api/v1/...`, `/sub-api/v1/...`,
> `/pay-int-api/...`).

### Auth flow

- OAuth2 **`client_credentials`** grant; POST form `grant_type/client_id/client_secret`
  to `oauth_url`; response `{access_token, expires_in}`.
- Bearer JWT (`Authorization: Bearer …`) on every API call (confirmed: the OpenAPI's
  only security scheme is `Bearer` HTTP/JWT, applied globally).
- Token cached in-process, refreshed 60s before expiry, and re-fetched once on a 401
  (`TokenStore` + `OAuth2Fetcher`, ported from the reference).
- **Secrets** stored in the OS keychain as a single JSON entry per profile
  (`{client_id, client_secret}`) — one auth prompt per launch, never plaintext on disk.
- **Env fallback** for CI/agents: `FLUTE_CLIENT_ID` + `FLUTE_CLIENT_SECRET` (and
  `FLUTE_PROFILE`) checked before the keychain.

### Auth commands

`flute auth login [--profile p]`, `auth status`, `auth switch <p>`, `auth logout`, `auth token`.

### Merchant context

`merchantId` is **not** a global header. In the OpenAPI it appears only as a path/query
param on token-management and a few list endpoints; merchant scoping otherwise rides in
the token (ISV vs merchant-scoped tokens, per profile config). `--merchant-id` is applied
as a query/path param only on the commands whose endpoints declare it.

### Config precedence (highest → lowest)

1. Flags (`--profile`, `--output`, `--merchant-id`)
2. Env vars (`FLUTE_PROFILE`, `FLUTE_CLIENT_ID`, `FLUTE_CLIENT_SECRET`)
3. `~/.flute/profiles/<name>.toml`
4. `~/.flute/config.toml`

## 3. Architecture & Module Layout

Mirrors the reference project's module taxonomy, with the API client split per resource
group (≈50 in-scope endpoints) to keep each file focused.

```
src/
├── main.rs                  # thin: flute_cli::run()
├── lib.rs                   # run(): clap parse → tracing → tokio runtime → dispatch
│                            #        → JSON error-envelope on failure under --output json
├── config.rs                # Config (TOML) + Profile (sandbox→UAT, production) + precedence
├── auth/
│   ├── mod.rs
│   ├── keychain.rs          # single JSON entry per profile + env fallback
│   └── token.rs             # TokenStore + OAuth2Fetcher (cache, 60s margin, 401 refresh)
├── api/
│   ├── mod.rs
│   ├── error.rs             # ApiError + from_aspnet (title/details/exceptionType/correlation_id)
│   ├── client/
│   │   ├── mod.rs           # ApiClient + shared send/issue/401-retry core
│   │   ├── transactions.rs  ach.rs  customers.rs  pos.rs
│   │   └── settlements.rs   subscriptions.rs  tokens.rs
│   └── models/              # serde DTOs per group, modeled from the OpenAPI doc
├── cli/
│   ├── mod.rs               # clap tree + global flags
│   ├── output.rs            # envelope + table + quiet + ErrorJson + exit codes
│   └── transactions.rs ach.rs customers.rs pos.rs settlements.rs
│       subscriptions.rs tokens.rs auth.rs util.rs
├── update.rs                # axoupdater self-update
└── update_check.rs          # optional post-command "newer version" notice (stderr only)
```

### Design-for-isolation notes

- The API client's request/auth/401-retry plumbing lives once in `client/mod.rs`; each
  per-group file only declares typed endpoint methods. Same `send`/`issue` core as the
  reference.
- `output.rs` owns the single source of truth for the JSON envelope, the `ErrorJson`
  agent envelope, and exit-code mapping, so the machine contract stays uniform across
  all command groups.

## 4. Output, Errors & Exit Codes

- `--output table` (default) | `json` | `quiet`/`-q` on **every** command.
- **JSON envelope** (spec §4): `{ "object": <type>, "data": <api response>,
  "meta": { "environment": <profile>, "correlation_id": <id|null> } }`.
- **quiet**: prints only the resource id (for `TXN_ID=$(flute … -q)` scripting).
- **Streams:** data → stdout, human errors → stderr (table mode). Under `--output json`,
  failures print the structured `ErrorJson` to **stdout** and exit non-zero, so an agent
  parses a single stream.
- **`ErrorJson`** (ported from reference): `{ kind, message, status?, correlation_id? }`
  where `kind ∈ {api, transport, auth, decode, client}`.
- **Exit codes:** `0` success, `1` general, `2` auth (401/403 / keychain / OAuth),
  `3` validation (400/422 / client-side input validation), `4` not found (404).
- **`--debug`:** full HTTP request/response to stderr (table) / log; sensitive fields
  (bearer token, card/PAN, CVV, account/routing, client_secret) redacted.

## 5. Command → Endpoint Mapping

| Group | Commands | Endpoint(s) |
|---|---|---|
| **transactions** | `auth`, `sale`, `capture`, `void`, `refund`→`return`, `settle`, `tip-adjust`→`tip-adjustment`, `get <id>`, `list`, `inspect <id>` | `/pay-api/v1/transactions/*` |
| **ach** | `debit`→`ach/payment`, `credit`→`ach/payment/credit`, `void <id>`, `refund <id>` | `/pay-api/v1/transactions/ach/*` |
| **customers** | `create/get/list/update/delete`, `add-card`, `add-ach`, `methods`, `remove-method` | `/pay-api/v1/customers/*` |
| **terminals** | `list`, `status <id>` | `/pos-api/v1/terminals*` |
| **pos** | `create [--wait]`, `get <id>`, `list`, `cancel <id>` | `/pos-api/v1/pos-transactions/*` |
| **devices** | `list`, `get <id>`, `register <id>`, `ttp-jwt`, `ttp-activate <id>` | `/pay-api/v1/devices/*` |
| **settlements** | `list`, `get <id>` (client-side filter over batches) | `/pay-api/v1/settlements/batches` |
| **subscriptions** | `create`, `get <id>`, `list`, `payments <id>`, `terminate <id>` | `/sub-api/v1/subscriptions/*` |
| **tokens** | `create --merchant-id`, `list [--merchant-id]`, `revoke --client-id` | `/pay-api/v1/merchants/tokens` |
| **utility** | `ping`, `version`, `update`, `completion bash|zsh|fish` | `/pay-int-api/ping` |

### Documented divergences from source spec (approved)

1. **`settlements get <id>`** — the API has no single-settlement endpoint, only the
   `/settlements/batches` list. Implemented as a client-side filter over the batch list;
   the limitation is documented in `--help` and `agents.md`.
2. **`transactions inspect <id>`** — a client-composed rich view (GET the transaction,
   derive available operations / amounts / L2-L3 / AVS-EMV from the response), not a
   distinct API call.
3. **`refund` → `/transactions/return`**, **`tip-adjust` → `/transactions/tip-adjustment`**,
   **devices live under `/pay-api/v1/devices`** (not `/pos-api`). CLI verbs follow the
   source spec; endpoints follow the OpenAPI.
4. The OpenAPI exposes extra ACH ops (`hold`/`unhold`); these are **not** in scope.
5. **Friendly card flags map to API fields:** `--card` → `accountNumber`, `--cvv` →
   `securityCode`, `--exp MM/YY` → `expirationMonth` + `expirationYear` (4-digit). The
   sale/auth body also requires `cardDataSource` (defaulted to manual key-entry) and
   `currencyId` (defaulted to USD); both overridable by flag.
6. **`transactions list` filters:** the API supports `page`/`pageSize`/`asc`/`orderBy`/
   `batchId`/`noBatch` — not the source spec's `--status`/`--from`/`--to`. The CLI exposes
   `--limit` (→`pageSize`), `--page`, and `--unsettled` (→`noBatch`); `--status`/date
   filters are applied **client-side** over the returned page and documented as such.

### Money handling

Amounts are parsed from the `--amount`/`--tip-amount` strings into
`rust_decimal::Decimal` for validation (well-formed, non-negative, scale ≤ 2) — **never
`f64` arithmetic**. The CLI performs no money math; it forwards the exact decimal the user
typed. The wire field is a JSON number, so the request-body builder emits the amount via a
serde path that preserves the exact decimal value (no float rounding).

### Idempotency (drives `agents.md` retry table)

- **Safe to retry:** all `get`/`list`/`status`, `void`/`cancel` (404 on second call =
  idempotent success), `terminate`, `delete`/`remove-method`/`revoke`.
- **NOT safe to retry:** `transactions sale`/`auth`, `ach debit`/`credit`, `capture`,
  `refund`/`return`, `pos create`, `customers create`, `tokens create` — each may create
  a duplicate financial/resource record. On ambiguous timeout, `list`/`get` to reconcile
  before reissuing.

## 6. Phasing (walking skeleton first)

Each phase ends green: `cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test`,
zero `unsafe`.

- **Phase 0 — Foundation.** Cargo deps; `config`; `auth` (keychain + token); `api` core
  (`send`/`issue`/401-retry/`from_aspnet`); `output` (envelope/table/quiet/`ErrorJson`/
  exit codes); clap skeleton with global flags; tracing.
  **Smoke:** `flute ping` against UAT; `flute auth login/status/token`.
- **Phase 1 — Transactions vertical slice.** sale/auth/capture/void/refund/settle/
  tip-adjust/get/list/inspect. Proves request-DTO → client → envelope → table/json/quiet
  end-to-end with golden tests.
- **Phase 2 — ACH + Customers/Vault.**
- **Phase 3 — POS / Terminals / Devices** (incl. `pos create --wait` long-poll).
- **Phase 4 — Settlements + Subscriptions + ISV Tokens.**
- **Phase 5 — Distribution & polish.** `completion`, `version`, `update`; cargo-dist;
  GitHub Actions release; Homebrew tap; `install.sh`; **`agents.md`**.

## 7. Distribution (Phase 5)

- **Targets:** macOS amd64/arm64, Linux amd64/arm64, Windows amd64; SHA256 checksums.
- **cargo-dist** (`dist-workspace.toml`, `[profile.dist]` thin-LTO) for cross builds.
- **GitHub Actions:** `PR → fmt → clippy -D warnings → test → build (all platforms)
  → release on tag`.
- **`flute update`** via `axoupdater` (GitHub Releases); source builds get an info message.
- **Homebrew** tap (`flute-payments/tap/flute`) + `curl … install.sh | sh`.

## 8. Testing

- `wiremock` (mock API + OAuth host), `tempfile` (isolated `~/.flute`),
  `pretty_assertions`, golden-file tests for table & JSON output, `assert_cmd` for CLI
  invocation, `tokio` `test-util`.
- Unit: flag parsing, input validation, envelope formatting, exit-code mapping,
  redaction, error parsing (`from_aspnet`), token cache/refresh.
- Integration: against UAT-as-sandbox (full lifecycles), gated behind env credentials.
- TDD throughout (superpowers).

## 9. `agents.md` (machine contract)

Modeled on the reference `AGENTS.md`:

- TL;DR: env-var auth path (`FLUTE_CLIENT_ID/SECRET`, `--output json`).
- Per-command success stdout shapes (the JSON envelope's `data` type).
- Failure envelope + `kind`/`status` → retry/backoff decision table.
- Idempotency table (§5) — emphasize money-moving `create`/`sale` are NOT retryable.
- Exit-code table; global flags; `--merchant-id` semantics.
- Intent → command mapping. "Things to avoid" (no TUI, don't mix `--output json` with
  interactive `auth login`, redaction caveats).

## 10. Tech Stack

`tokio` (multi-thread rt, macros, signal), `reqwest` (rustls-tls, json),
`clap` (derive, env), `serde`/`serde_json`, `rust_decimal` (money parse/validate),
`keyring` (apple/windows/linux-native), `toml`, `dirs`, `thiserror`, `anyhow`,
`async-trait`, `rpassword`, `tracing`/`tracing-subscriber`, `url`, `axoupdater`.
Dev: `wiremock`, `tempfile`, `pretty_assertions`, `assert_cmd`. Edition 2024.

## 11. Open Items (resolve during implementation)

- Confirm the live UAT base-path routing with the Phase 0 `ping` smoke test (the
  reference project hit a gateway-vs-swagger path subtlety; `/pay-int-api/ping` verifies it).
- Pin exact request/response DTO field names & casing from the OpenAPI doc per phase
  (casing is known to be non-uniform across surfaces in this API family).
- Confirm `pos create --wait` completion signal (terminal status field to poll on).
