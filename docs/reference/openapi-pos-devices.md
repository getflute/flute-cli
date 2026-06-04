# OpenAPI wire-format reference — POS / Terminals / Devices (Phase 3)

Pinned from the UAT OpenAPI. Reuse the established pattern (Value body-builders, Value
responses, pure render helpers + golden tests, `to_amount_number` for amounts,
`send`/`send_no_body` client core). Note: live testing of earlier phases proved the
OpenAPI "required" list undersells real requirements — expect to add fields when testing live.

## Terminals — `/pos-api/v1/terminals`

| CLI verb | Method + path | Response |
|---|---|---|
| `terminals list` | GET `/pos-api/v1/terminals` (query: page, pageSize←`--limit`, asc, orderBy) | PageOfGetIsvTerminalsResponseDto `{items,total}` |
| `terminals status <id>` | GET `/pos-api/v1/terminals/{terminalId}/status` | RetrieveIsvTerminalStatusResponseDto |

- list item GetIsvTerminalsResponseDto: `id, serialNumber, terminalManufacturer, terminalModel,
  terminalModeName, connectionStatus, deliveryStatusName, lastSeenTimestamp`.
- status RetrieveIsvTerminalStatusResponseDto: `terminalId, terminalPosStatus (Active/Busy),
  connectionStatus, connectionType, wifiConnectionStrength, batteryLevel, availabilityStatus,
  ariseTerminalVersion, printerStatus, lastSeenTimestamp`.

## POS transactions — `/pos-api/v1/pos-transactions`

| CLI verb | Method + path | Body/Response |
|---|---|---|
| `pos create [--wait]` | POST `/pos-api/v1/pos-transactions` | CreateIsvPosTransactionRequestDto → GetIsvPosTransactionResponseDto |
| `pos get <id>` | GET `/pos-api/v1/pos-transactions/{id}` | GetIsvPosTransactionResponseDto |
| `pos list` | GET `/pos-api/v1/pos-transactions` (query: page, pageSize←`--limit`, terminalId←`--terminal-id`) | page `{items,total}` |
| `pos cancel <id>` | POST `/pos-api/v1/pos-transactions/{id}/cancel` | **bodyless** |

(`/print` exists but is OUT of scope — spec CLI only has create/get/list/cancel.)

### CreateIsvPosTransactionRequestDto
- `terminalId`: uuid — `--terminal-id` (**required**)
- `transactionTypeId`: int enum TransactionTypeDto — `--transaction-type`; **1=Authorization, 2=Sale
  (default), 3=Capture, 4=Void, 5=Refund**
- `amount`: number nullable — `--amount` (required for Auth/Sale; via `to_amount_number`)
- `currencyId`: int nullable — `--currency-id` (conditional; default 1=USD like card, may be required live)
- `tipAmount`/`tipRate`: number nullable — `--tip-amount`/`--tip-rate`
- `posDeviceId`: string nullable — `--pos-device-id` (desc says "Mandatory"; expose it)
- `referenceId`: string nullable — `--reference-id`
- `paymentProcessorId`: uuid nullable — `--payment-processor-id`
- `customerId`: uuid nullable — `--customer-id`
- `targetTransactionId`: uuid nullable — `--target-transaction-id` (for Void/Capture/Refund types)
- `readingMethodId`: int enum — `--reading-method` (1=Reading default, 2=KeyedIn); default omit
- `waitForAcceptanceByTerminal`: bool — set **true when `--wait`**, else false (server-side long poll for terminal acceptance)
- `requestPaymentMethodStorageConsent`: bool — default false (omit)

### GetIsvPosTransactionResponseDto (poll target for `--wait`)
- `id`, `terminalId`, `posDeviceId`, `transactionType`, `amount`, `currencyId`
- `posTransactionStatusId`/`posTransactionStatus`: enum PosTransactionStatusDto
  **1=TerminalConnecting, 2=TransactionProcessing, 3=DeclinedByProcessor, 4=CancelByPos, … (final states ≥3)**
- **`isCompleted`: bool — THE poll-exit signal** (final status reached, success or failure)
- `transactionId`: uuid nullable (the resulting card transaction id, once processed)
- `transaction`, `transactionReceipt`: nested details

### `pos create --wait` long-poll (the new mechanic)
1. Build create body with `waitForAcceptanceByTerminal=true`; POST → get the pos-transaction `id`.
2. If `--wait`: poll `GET /pos-transactions/{id}` every N seconds (fixed, e.g. 2s) until
   `isCompleted == true` OR a timeout (`--wait-timeout` seconds, default 120). Honor Ctrl-C via
   `tokio::signal` — on interrupt, print the last-known status and exit non-zero/zero gracefully.
   Render the final pos-transaction. Without `--wait`: render the create response immediately.
3. Implement the poll loop with `tokio::time::sleep`; make the "is this a terminal/exit state"
   decision a PURE testable fn `pos_is_final(v: &Value) -> bool` (reads `isCompleted`), unit-tested.
   Test the loop with `tokio::time::pause()` + a wiremock status sequence (Connecting→Processing→isCompleted:true).

## Devices — `/pay-api/v1/devices`

| CLI verb | Method + path | Body/Response |
|---|---|---|
| `devices list` | GET `/pay-api/v1/devices` | GetIsvDevicesResponseDto `{devices:[DeviceResponseDto]}` |
| `devices get <id>` | GET `/pay-api/v1/devices/{deviceId}` | device |
| `devices register <id>` | POST `/pay-api/v1/devices` | CreateOrUpdateIsvDeviceRequestDto `{deviceId, deviceName}` |
| `devices ttp-jwt` | POST `/pay-api/v1/devices/tap-to-pay/jwt` | GenerateIsvTapToPayJwtRequestDto `{deviceId}` (`--device-id`) |
| `devices ttp-activate <id>` | POST `/pay-api/v1/devices/{deviceId}/tap-to-pay/activate` | **bodyless** |

- `register <id>` → body `{deviceId: <id>, deviceName: --name}`. (POST /devices is create-or-update.)
- `ttp-jwt --device-id <id>` → body `{deviceId: <id>}`; response carries the Tap-to-Pay JWT.

## Rendering
- pos/terminals/devices responses: add pure render helpers (render_pos_transaction, terminal table,
  terminal_status table, devices table, device) per the established pattern; json=Envelope (objects
  "pos_transaction"/"terminal"/"terminal_status"/"device"/"...list"); quiet=id; table=key fields.
- cancel / ttp-activate are bodyless POST returning a body (render it) — use `send` with None body.
