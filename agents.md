# agents.md — driving `flute` from an AI agent

Machine-readable contract for autonomous callers (Claude Code, GPT function-calling,
custom MCP servers, CI). Humans should read `readme.md`. This file describes the **v1**
ISV API surface the CLI targets.

## TL;DR

```bash
# Agent-friendly auth: env vars (no keychain, no interactive login)
FLUTE_CLIENT_ID=… FLUTE_CLIENT_SECRET=… FLUTE_PROFILE=sandbox \
  flute --output json transactions list --limit 5
```

`--output json` is honoured by the **API resource commands** — the `transactions`, `ach`,
`customers`, `terminals`, `devices`, `pos`, `settlements`, `subscriptions` and `keys` groups,
plus `ping`, `version` and `auth status`. On success these **normally** print the JSON
envelope on **stdout** — a few mutating verbs succeed with empty stdout instead, documented
below. On failure they print a structured `ErrorJson` on **stdout** and exit non-zero.
**Parse one stream, never both.**

Before writing a parser, read
[Commands with no JSON success output](#commands-with-no-json-success-output): several
commands emit plain text or nothing at all on success even in JSON mode, and two non-zero
exits carry no `ErrorJson`. Do not invoke `auth login` (interactive) from an agent — use the
env vars above.

## Global flags

| Flag | Meaning |
|---|---|
| `--profile <name>` | `sandbox` (default) or `production`/`prod`. Env: `FLUTE_PROFILE`. |
| `--output <fmt>` | `json` (use this), `table` (human, default), `quiet` (resource id only). Falls back to `~/.flute/config.toml` `output`, then `table`. |
| `--debug` | Verbose HTTP request/response to **stderr** (sensitive fields redacted). For agents, prefer `--output json` + the `correlation_id`; only use `--debug` when an operator is investigating. |

There is **no global `--merchant-id`** (it was removed in v0.1.4 — passing it is a parse
error, exit 3). Merchant scoping is per-command: see `keys create|list|revoke` below. Every
other endpoint resolves the merchant server-side from the token principal.

`--output json` also suppresses the "newer version available" notice and never emits it to a
non-TTY/CI stream — JSON stdout stays pure.

## Output contract

### Success (`--output json`)
```json
{ "object": "<type>", "data": <api response>, "meta": { "environment": "sandbox", "correlation_id": "…"|omitted } }
```
- `object` values: `transaction`, `transaction_list`, `customer`, `customer_list`, `payment_method`,
  `payment_methods`, `pos_transaction`, `pos_transaction_list`, `terminal_list`, `terminal_status`,
  `device`, `device_list`, `tap_to_pay_jwt`, `settlement`, `settlement_list`, `subscription`,
  `subscription_list`, `subscription_payments`, `api_token`, `api_token_list`, `auth_status`,
  `ping`, `version`.
- `--output quiet` prints **only the resource id** (one per line for lists) — ideal for
  chaining: `TXN=$(flute --output quiet transactions sale --amount 10.00 …)`. Note there is
  **no `-q` short flag** (the CLI defines no short flags beyond `-h`/`-V`); spell it
  `--output quiet` or set `FLUTE_OUTPUT=quiet`.

### Failure (`--output json`)
```json
{ "kind": "api"|"transport"|"auth"|"decode"|"client", "message": "…", "status": 422, "correlation_id": "…" }
```
`status` and `correlation_id` appear only for `kind:"api"`. Branch on `kind` first, then `status`:

> **A non-zero exit does not always carry an `ErrorJson`.** Both known cases are in
> `pos create --wait`:
> * **timeout** — under `--output json` the CLI prints the last-known **success** envelope
>   (`object:"pos_transaction"`) to stdout, a warning to stderr, and exits **1**. Under
>   `table`/`quiet` it renders the table / bare id instead.
> * **Ctrl-C (SIGINT)** — the last-known status goes to **stderr** only; stdout stays
>   **empty** in every output mode, and the CLI exits **130**.
>
> So don't infer the shape of stdout from the exit code. If stdout is empty, go by the exit
> code alone; if it parses, check for a `kind` field before treating it as an error. See
> [POS](#pos--terminals--devices).

| `kind` | meaning | retry? |
|---|---|---|
| `api` + status ∈ {500,502,503,504} | transient server error | yes, backoff |
| `api` + status ∈ {401,403} | auth broken | no — refresh creds, then retry once |
| `api` + status ∈ {400,409,422} | permanent for this request | no — surface `message` + `correlation_id` |
| `api` + status 404 | not found | no |
| `transport` | connection/TLS/DNS | yes, backoff |
| `auth` | no credentials / OAuth handshake failed | no — operator must configure creds |
| `decode` | CLI bug or server contract change | no — surface for investigation |
| `client` | bad CLI args / input validation | no — fix the invocation |

### Commands with no JSON success output

These commands are local or plain-text: **on success** they emit no JSON in any output mode,
so don't feed their success output to a parser.

**Their failures are still JSON.** Every command routes errors through the same central
handler, so under `--output json` a failure prints an `ErrorJson` on stdout and exits
non-zero — for example, `flute --output json auth switch garbage` →
`{"kind":"client","message":"unknown profile: garbage"}`, exit 3. So the rule is
success-only: parse the failure envelope for every command, but expect plain text on success
for these.

| Command | Stdout **on success**, even with `--output json` |
|---|---|
| `completion <shell>` | raw shell completion script |
| `update` | plain text (for example, `Already on the latest version (<version>).`) |
| `auth switch <profile>` | plain text (`Default profile set to [production].`) |
| `auth logout` | plain text |
| `auth token` | the raw bearer token on one line, no envelope |
| `auth login` | interactive prompts — never call this from an agent |

`auth status` is the one `auth` verb that *does* emit an envelope (`object:"auth_status"`).

And three mutating verbs **succeed** with empty stdout under `--output json` — treat `exit 0`
plus empty stdout as success, not as a truncated response. (Their failures, again, come back
as a normal `ErrorJson`.)

| Command | Success under `--output json` |
|---|---|
| `customers delete <id> --yes` | exit 0, empty stdout |
| `customers remove-method <id> <mid> --yes` | exit 0, empty stdout |
| `keys revoke --client-id … --merchant-id … --yes` | exit 0, empty stdout |

(In `table` mode these print a confirmation line; in `quiet` mode, the affected id.)

### Exit codes
`0` success · `1` general/unexpected (transport, decode, server 5xx) · `2` auth (401/403, missing creds) · `3` validation / bad input (server 400/422, **client-side validation, and CLI usage/parse errors**) · `4` not found (404). Under `--output json`, usage/parse errors are still emitted as a `{kind:"client"}` envelope on stdout.

## Environments

`sandbox` → `https://sandbox.api.flute.com`.
`production` → `https://api.flute.com`. Production commands print a red warning
banner to **stderr** (never stdout).

---

## Commands

All money amounts are plain decimals (`--amount 100.00`), validated to ≤2 decimal places and sent
as exact JSON numbers (no float rounding). `--exp` is `MM/YY` or `MM/YYYY`.

### Transactions (card) — `flute transactions …`
`sale`, `auth`, `capture`, `void`, `refund`, `settle`, `tip-adjust`, `get <id>`, `list`, `inspect <id>`.
- `sale`/`auth` flags: `--amount` (required), `--card`, `--exp`, `--cvv`, `--tip-amount`,
  `--customer-id`, `--payment-method-id`, `--currency-id` (**default 1=USD; required by the API**),
  `--card-data-source` (default 1=Internet/ISV), `--l2-tax-rate`, `--l3-invoice`, `--l3-po`,
  `--l3-product` (repeatable, `Description,SKU,UnitPrice,UnitOfMeasure,Quantity`), `--reference-id`.
- `sale`/`auth` **AVS billing** (ARISE-4706): `--billing-line1`, `--billing-line2`, `--billing-city`,
  `--billing-state`, `--billing-state-id <int>`, `--billing-postal-code`, `--billing-country-id <int>`
  — emitted as `billingAddress` only when at least one is set. **Only a street line and the ZIP
  feed card AVS**: the gateway's pre-sale check (`AvsService`) matches `line1 ?? line2` plus
  `postalCode`, so `--billing-line2` participates when `--billing-line1` is absent — the
  authorization sent on to the processor carries line 1 only. City, state and country are stored
  on the transaction but never matched, so supply a street line + ZIP for AVS coverage.
  No billing field is required — including the ZIP, which is only length-checked when supplied —
  and the rules are **per card-data-source**: under the CLI default `--card-data-source 1`
  (Internet) a supplied ZIP must be 5+ characters and `--billing-state-id` must belong to
  `--billing-country-id` (checked only when both are set); for card-present sources (2–7) a
  supplied ZIP need only be 2+ characters and the state/country check is skipped. The one case
  that makes a field mandatory is `--card-data-source 7` (Manual), which additionally requires a
  ZIP when the merchant has AVS enabled.
  A partial address (city alone, say) is accepted. Omitting the address entirely can still cause
  AVS-sensitive processors to **decline** the transaction. The stricter "city + country id
  required" rule is **ACH-only** (see `flute ach`), not card.
- `capture`/`void`/`refund`/`tip-adjust` take `--transaction-id`; `refund`/`capture` accept optional `--amount`; `tip-adjust` takes `--tip-amount`.
- `settle` takes `--payment-processor-id` (**batch-level** — settles the processor's open batch, NOT a single txn).
- `list` flags: `--limit`(→pageSize), `--page`, `--unsettled`; `--status`/`--from`/`--to` filter the returned page **client-side** (not server params).
- `inspect <id>` is a rich client-composed view; reads the API's `availableOperations` (accepts operation objects with a `type` field **or** bare strings).
- **Two response shapes.** `POST sale`/`auth` return `AuthorizationResponseDto` and
  `capture`/`void`/`refund`/`tip-adjust` return `TransactionResponseIsvDto`: these carry
  `transactionId`, `status`, `processedAmount`, a `details` object (`authCode`, `code`, `message`,
  `hostResponseCode`, `hostResponseMessage`, `maskedPan`) and a nested `transactionReceipt`.
  `GET {id}` (and `inspect`) instead return `GetIsvTransactionResponseDto`, which **is** the receipt
  — there `amount` (object with `totalAmount`), `authCode` and `responseDescription` sit at the top
  level. Only `transactionId` and `status` appear at the top level of both. When reading JSON, take
  the amount from `amount.totalAmount` on a GET and from `processedAmount` (or
  `transactionReceipt.amount.totalAmount`) on a POST; the table renderer resolves both.
- **AVS result (`avsResponse`):** a **structured object** (not a string), present only when AVS is enabled for the merchant/processor — otherwise `null`. Shape: `{ responseCode (raw AVS code, for example "Y"/"A"/"Z"/"N"), action ("Allow"|"Deny"), group ("NoMatch"|"PartialMatch"|"Incompatible"|"Unavailable"|"ValidGroup"), result ("Passed"|"Failed"), codeDescription (human text), plus int enums actionId/groupId/resultId }`. It appears on `sale`/`auth`/`get`/`inspect` responses (top-level; `transactionReceipt.avsResponse` may stay `null`). Read the full object from `--output json`; the `inspect` table summarizes it as `code — group / result / action — description` (or `—` when null). **A transaction can be Approved even when `result` is "Failed"** if the merchant's AVS `action` is "Allow" — treat `avsResponse` as advisory and enforce your own policy on `responseCode`/`group` if needed.
- **Reading current state:** derive a transaction's current state **only** from `status`/`statusId` plus `availableOperations` — **not** from `transactionType` or `operationType`. Both are sticky to the *original* operation: after a `void`, the record still reads `transactionType:"Sale"` and `operationType:"PayNow"`, while `status` becomes `"Voided"` and `availableOperations` becomes `[]`. `void`/`refund` update `status` in place (same `transactionId`); the API exposes **no** `lastOperationType`, `voidedAt`/`refundedAt`, or operations-history field, and `transactionDateTime` stays the original timestamp. Rule of thumb: `availableOperations` lists what you *can* still do, `status` tells you what *happened*, `transactionType` only tells you what it originally *was*.

### ACH — `flute ach …`
`debit`, `credit`, `void <id>`, `refund <id>`.
- `debit`/`credit` **require live**: `--payment-processor-id`, plus a billing address and contact info and account-holder-type (the OpenAPI under-marks these). Supply:
  `--amount`, `--payment-processor-id`, `--routing`, `--account`, `--account-type checking|savings`,
  `--account-holder-type business|personal`, `--billing-line1`, `--billing-city`, `--billing-state`,
  `--billing-state-id <int>` (**numeric state id required**; free-text alone is rejected),
  `--billing-postal-code`, `--billing-country-id 1` (US), `--contact-first-name`, `--contact-last-name`,
  `--contact-email`, `--contact-phone` (**required**), `--contact-company` (optional → `companyName`),
  `--sec-code` (default 1=Web), `--requester-ip` (default 127.0.0.1).
- `void`/`refund` take a positional `<id>` (bodyless).

### Customers / Vault — `flute customers …`
`create`, `get <id>`, `list`, `update <id>`, `delete <id> --yes`, `add-card <id>`, `add-ach <id>`,
`methods <id>`, `remove-method <id> <method-id> --yes`.
- `create`/`update` flags: `--first-name`, `--last-name`, `--email`, `--company`, `--mobile`.
  **`update` is GET-merge-PUT** (the API PUT is full-replace): omitted flags retain existing values.
- `create`/`update` **AVS billing** (ARISE-4706): `--billing-line1`, `--billing-line2`, `--billing-city`,
  `--billing-state`, `--billing-state-id <int>`, `--billing-postal-code`, `--billing-country-id <int>`
  → customer `billingAddress`. On `update`, supplying any `--billing-*` replaces the address wholesale;
  omitting them preserves the customer's current `billingAddress`.
- `list` flags: `--limit`(→pageSize), `--page`, `--search` (real server param).
- `add-card <id>`: `--card`, `--exp`, `--cvv`, `--name`. `add-ach <id>`: `--routing`, `--account`, `--account-type`, `--account-holder-type`, `--tax-id`, `--name`.
- POST responses are minimal (`{id}` / `{clientId,…}`); use `get`/`methods` for full detail.
- `delete`/`remove-method` **require `--yes`**; a repeat delete (404) is treated as idempotent success.

### POS / Terminals / Devices
- `flute terminals list` / `status <id>`. Terminal must be **SemiIntegrated** mode + Online to accept POS transactions (Standalone → 400).
- `flute pos create` flags: `--terminal-id` (required), `--amount`, `--pos-device-id` (**required**),
  `--reference-id` (**required**), `--currency-id` (default 1), `--transaction-type` (default 2=Sale; 1=Auth,3=Capture,4=Void,5=Refund), `--tip-amount`, `--tip-rate`, `--customer-id`, `--payment-processor-id`, `--target-transaction-id`, `--reading-method`, `--wait`, `--wait-timeout` (default 120).
  - `--wait` long-polls `GET /pos-transactions/{id}` until `isCompleted:true` (terminal finished) or timeout. On **timeout** with `--output json`: the CLI prints the **last-known success envelope** (`object:"pos_transaction"`, *not* an `ErrorJson`) to stdout, then a warning to stderr, and exits 1 — in `table`/`quiet` mode it renders the table / bare id instead. On **Ctrl-C**: the last-known status goes to **stderr only**, stdout stays empty in every mode, and it exits 130. Exit 0 on successful completion. **A terminal allows only one in-progress POS transaction** (else 400 "already in progress") — always `cancel` or complete before starting another.
  - **Field-name split:** create/cancel responses use `posTransactionId` + `status`; get/list use `id` + `posTransactionStatus`. (The CLI normalizes both for table/quiet.)
- `flute pos get <id>` / `list` (`--terminal-id`, `--limit`, `--page`) / `cancel <id>`.
- `flute devices list` / `get <id>` / `register <id> [--name]` (`--name` is optional) / `ttp-jwt --device-id <id>` (returns `tap_to_pay_jwt` envelope object) / `ttp-activate <id>`. Device records use `deviceId` (not `id`) and `tapToPayStatus`.

### Settlements — `flute settlements …`
`list` (`--limit`, `--page`, `--from`, `--to`, `--status open|settled`), `get <id>`.
**`get <id>` is a client-side filter over the fetched page** (no single-batch endpoint) — page-bounded.

### Subscriptions — `flute subscriptions …`
`create`, `get <id>`, `list`, `payments <id>`, `terminate <id> --yes`.
- `create` **required**: `--customer-id`, `--payment-method-id` (must be a vaulted+active method), `--amount`, `--number-of-payments`, and **`--payment-processor-id`** (the API rejects a create without it). Plus `--interval day|week|month` (aliases `daily|weekly|monthly`, default month), `--payment-frequency` (default 1 — but **for `--interval day` the API only accepts 7, 15, or 30**), `--currency-id` (default 1), `--transaction-type` (default 2=Sale; 11=AchDebit), `--requester-ip` (default 127.0.0.1), `--start-date`, `--sec-code`, `--faster`.
- `list` flags: `--limit`, `--page`, `--search`, `--customer-id`. `--status` is client-side.
- `terminate` **requires `--yes`**. List items use `subscriptionId`; get uses `id` (CLI normalizes).

### ISV API Keys — `flute keys …`
`create`, `list`, `revoke`. (`flute tokens …` is a deprecated hidden alias that still works.)
- `create --merchant-id <uuid> --name "<name>"` → returns `clientId` + **`clientSecret` shown ONCE** (capture it from the JSON envelope; the API never returns it again).
- `list [--merchant-id <uuid>]`.
- `revoke --client-id <uuid> --merchant-id <uuid> --yes` — **`--merchant-id` is required** (DELETE needs it as a query param) and `--yes` is required. 404 is idempotent success.

### Utility — `flute …`
`ping` (health check), `version`, `update` (self-update; no-op message if built from source), `completion bash|zsh|fish|powershell|elvish`.
- `auth status` → `{profile, api_base_url, authenticated, client_id, merchant_id}` (ARISE-4706). It is a **live** check: it pings the API, so `authenticated` is `true` only when the stored credentials actually round-trip; `client_id` is the server-authoritative value (falls back to the stored id), `merchant_id` is present when the principal is merchant-bound.

---

## Idempotency

| Safe to retry | NOT safe to retry (creates/moves money, or errors on repeat) |
|---|---|
| all `get`/`list`/`status`/`inspect`/`methods`/`payments` (pure reads) | `transactions sale`/`auth`, `ach debit`/`credit`, `transactions capture`/`refund`, `pos create`, `customers create`, `customers add-card`/`add-ach`, `subscriptions create`, `keys create` |
| `delete`/`remove-method`/`keys revoke` — the CLI maps a repeat **404** to success, so re-running is a safe no-op | `void`/`cancel`/`terminate` — **not** idempotent: the CLI does not swallow the repeat, so it surfaces the server's error. E.g. `subscriptions terminate` on an already-terminated subscription returns **400 "already Terminated"** (exit 3), *not* a 404/no-op. Reconcile with `get`/`list` first. |

On an ambiguous timeout for a non-retryable op, **`list`/`get` to reconcile before reissuing**.
Use a unique `--reference-id` on `transactions`/`pos` to leverage server-side duplicate control.

## Common intents → commands

| Intent | Command |
|---|---|
| Health check | `flute --output json ping` |
| Charge a card | `flute --output json transactions sale --amount 10.00 --card <pan> --exp MM/YY --cvv <cvv>` |
| Look up a transaction | `flute --output json transactions get <id>` (or `inspect <id>` for rich detail) |
| Recent transactions | `flute --output json transactions list --limit 25` |
| Refund a transaction | `flute --output json transactions refund --transaction-id <id> [--amount …]` |
| Create + vault a customer | `flute … customers create --first-name … --email …` then `customers add-card <id> --card … --exp … --cvv …` |
| Start a terminal sale | `flute --output json pos create --terminal-id <id> --amount 10.00 --pos-device-id <dev> --reference-id <ref> --wait` |
| Issue a merchant API key | `flute --output json keys create --merchant-id <id> --name "<name>"` (save the one-shot `clientSecret`) |

## Things to avoid

- **Don't combine `--output json` with `auth login`** (interactive) — use env vars.
- **Don't parse stderr** — the structured error is on stdout under `--output json`. Stderr carries tracing, the production banner, and update notices.
- **Don't fire a second `pos create` on a terminal with one in-progress** — cancel/complete first.
- **Don't retry money-moving creates** without reconciling with `list`/`get`.
- **Card/secret redaction is only applied to `--debug` logs**, not to request bodies or JSON output — handle `clientSecret`/PAN responses securely.
- This CLI is **v1**. `aurora-payments/arise-backend#1099` (creditCard*→card* rename + strict unknown-field rejection) is **v2-only** and does not affect it.
