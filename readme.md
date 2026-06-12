# flute

`flute` — a cross-platform CLI for the Flute payments platform. MIT licensed, built in Rust.

---

## Install

> **Note:** Distribution packages are pending the first public release. The coordinates below are correct but the tap and install script will not resolve until release v0.1.0 is tagged.

**Homebrew (macOS / Linux)**

```sh
brew install flute-payments/tap/flute
```

**Shell script (maps to GitHub Releases)**

```sh
curl -fsSL https://cli.flute.dev/install.sh | sh
```

**From source**

```sh
# Prebuilt release binary
cargo install --path .

# Debug / development build
cargo build --release
# Binary lands at target/release/flute
```

---

## Quick start

**Authenticate** (interactive — prompts for client ID and secret, stored in the OS keychain):

```sh
flute auth login
```

For CI or non-interactive environments use env vars instead of the keychain:

```sh
export FLUTE_CLIENT_ID=<your-client-id>
export FLUTE_CLIENT_SECRET=<your-client-secret>
```

**Health check and version:**

```sh
flute ping
flute version
```

**Charge a card:**

```sh
flute transactions sale \
  --amount 10.00 \
  --card 4111111111111111 \
  --exp 12/27 \
  --cvv 123
```

**Output modes** are controlled by `--output table|json|quiet` (or the `FLUTE_OUTPUT` env var / `output` key in `~/.flute/config.toml`):

- `table` — human-readable (default)
- `json` — structured envelope, suitable for scripts and agents
- `quiet` — resource id only, one per line; ideal for shell capture: `TXN=$(flute … sale -q)`

---

## Profiles / environments

| Profile | Alias | API |
|---|---|---|
| `sandbox` | — | `https://sandbox.api.uat.flute.com` (UAT-backed until the real sandbox ships) |
| `production` | `prod` | `https://api.arise.risewithaurora.com` |

`sandbox` is the default. **Running any command against `production` prints a red warning banner to stderr.**

**Select a profile** (three ways, highest precedence first):

1. Flag: `--profile production`
2. Env var: `FLUTE_PROFILE=production`
3. Config file default: `~/.flute/config.toml` → `default_profile = "production"`

Credentials are kept in the OS keychain, keyed per profile. The config file (`~/.flute/config.toml`) stores only non-secret settings such as `default_profile` and `output`.

**Precedence for all settings:** flag > env var > profile config > global `config.toml` default.

---

## Command overview

| Group | What it does |
|---|---|
| `auth` | `login`, `logout`, `status`, `switch`, `token` — credential and profile management |
| `transactions` | `sale`, `auth`, `capture`, `void`, `refund`, `settle`, `tip-adjust`, `get`, `list`, `inspect` — card payment lifecycle |
| `ach` | `debit`, `credit`, `void`, `refund` — ACH bank-transfer payments |
| `customers` | `create`, `get`, `list`, `update`, `delete`, `add-card`, `add-ach`, `methods`, `remove-method` — customer vault |
| `terminals` | `list`, `status` — POS terminal management |
| `devices` | `list`, `get`, `register`, `ttp-jwt`, `ttp-activate` — mobile payment device management |
| `pos` | `create` (with `--wait` long-poll), `get`, `list`, `cancel` — POS transactions |
| `settlements` | `list`, `get` — settlement batch queries |
| `subscriptions` | `create`, `get`, `list`, `payments`, `terminate` — recurring billing |
| `tokens` | `create`, `list`, `revoke` — ISV API token management |
| `ping` | API health check |
| `version` | Print CLI version and active profile |
| `update` | Self-update to the latest GitHub Release |
| `completion` | Print shell completion script |

Run `flute <group> --help` for flags on any command.

---

## Output and scripting

`--output json` wraps every success response in a consistent envelope:

```json
{ "object": "<type>", "data": <api response>, "meta": { "environment": "sandbox", "correlation_id": "…" } }
```

Errors (non-zero exit) are also written to **stdout** as structured JSON when `--output json` is active:

```json
{ "kind": "api"|"transport"|"auth"|"decode"|"client", "message": "…", "status": 422, "correlation_id": "…" }
```

**Parse one stream — never both.** Data always goes to stdout; tracing, the production banner, and update notices always go to stderr.

### Semantic exit codes

| Code | Meaning |
|---|---|
| `0` | Success |
| `1` | General / unexpected error |
| `2` | Auth failure (401/403 or missing credentials) |
| `3` | Validation error (400/422, bad input) |
| `4` | Not found (404) |

---

## Shell completions

Supported shells: `bash`, `zsh`, `fish`, `powershell`, `elvish`.

Generate and install a completion script:

```sh
# Bash
flute completion bash > /etc/bash_completion.d/flute

# Zsh (add to a directory on $fpath, e.g. /usr/local/share/zsh/site-functions)
flute completion zsh > /usr/local/share/zsh/site-functions/_flute

# Fish
flute completion fish > ~/.config/fish/completions/flute.fish

# PowerShell
flute completion powershell >> $PROFILE

# Elvish
flute completion elvish >> ~/.config/elvish/rc.elv
```

---

## Self-update

```sh
flute update
```

Downloads and installs the latest GitHub Release binary in-place. Prints a no-op message when the CLI was built from source.

---

## For AI agents / MCP

See [`agents.md`](agents.md) for the machine-readable contract: structured output format, error JSON schema, exit codes, idempotency table, and copy-pasteable command recipes for common agent intents.

---

## Development

```sh
# Run all tests
cargo test

# Lint (zero warnings enforced)
cargo clippy --all-targets -- -D warnings

# Format check
cargo fmt --check

# Release build
cargo build --release
```

The codebase enforces `#![forbid(unsafe_code)]` — there is zero `unsafe` Rust. Design documents and the API spec live under `docs/`.
