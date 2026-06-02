# OpenAPI wire-format reference — Transactions (Phase 1)

Pinned from `https://api.uat.arise.risewithaurora.com/isv-api/swagger/v1/swagger.json`
(title "ISV API BFF", release-3.0.0). Field names/casing are authoritative; model serde
DTOs to match exactly. All amount fields on the wire are JSON `number` (double); the CLI
parses `--amount` as `rust_decimal::Decimal` and emits it as an exact JSON number via the
`to_amount_number` helper (serde_json `arbitrary_precision`).

## Endpoints

| CLI verb | Method + path | Request DTO | Response |
|---|---|---|---|
| sale | POST `/pay-api/v1/transactions/sale` | SaleRequestDto | GetIsvTransactionResponseDto (assume; permissive) |
| auth | POST `/pay-api/v1/transactions/auth` | AuthorizationRequestDto | same |
| capture | POST `/pay-api/v1/transactions/capture` | CaptureRequestDto | same |
| void | POST `/pay-api/v1/transactions/void` | VoidRequestDto | same |
| refund | POST `/pay-api/v1/transactions/return` | ReturnRequestDto | same |
| settle | POST `/pay-api/v1/transactions/settle` | SettleRequestDto | same |
| tip-adjust | POST `/pay-api/v1/transactions/tip-adjustment` | TipAdjustmentRequestDto | same |
| get | GET `/pay-api/v1/transactions/{id}` | — | GetIsvTransactionResponseDto |
| list | GET `/pay-api/v1/transactions` | query params | PageOfGetIsvTransactionsResponseDto |
| inspect | GET `/pay-api/v1/transactions/{id}` (client view) | — | GetIsvTransactionResponseDto |

## Request DTOs (send only the fields the CLI sets; skip_serializing_if None)

### SaleRequestDto / AuthorizationRequestDto (same shape)
- **required**: `cardDataSource` (int enum, see below)
- `amount`: number (double) — from `--amount`
- `tipAmount`: number nullable — from `--tip-amount`
- `tipRate`, `percentageOffRate`, `surchargeRate`: number nullable
- `currencyId`: integer (int32) — **NOT in required**; OMIT unless `--currency-id` is passed
  (server defaults). Do not send 0.
- `customerId`, `paymentMethodId`, `paymentProcessorId`: uuid nullable — `--customer-id`, `--payment-method-id`
- `accountNumber`: string nullable — from `--card`
- `securityCode`: string nullable — from `--cvv`
- `expirationMonth`, `expirationYear`: int nullable — from `--exp MM/YY` (year → 4-digit, 26→2026)
- `referenceId`: string nullable
- `customerInitiatedTransaction`: boolean (required-ish; default false)
- `l2` → L2DataDto `{ salesTaxRate }` — from `--l2-tax-rate`
- `l3` → L3DataDto `{ dutyCharges, invoiceNumber, products, purchaseOrder, shippingCharges }`
  — from `--l3-invoice`, `--l3-po`, `--l3-product` (product is an array of items)
- `billingAddress`/`shippingAddress` → AddressIsvDto; `contactInfo` → ContactInfoIsvDto
  `{ companyName, email, firstName, lastName, mobileNumber, smsNotification }`
- track1/track2/emvTags/pin/etc.: card-present fields — NOT exposed by the CLI (YAGNI)

### CaptureRequestDto
- **required**: `transactionId` (uuid) — from `--transaction-id`
- `amount`: number nullable — from optional `--amount` (partial capture)

### VoidRequestDto
- **required**: `transactionId` (uuid)

### ReturnRequestDto  (CLI verb: `refund`)
- **required**: `transactionId` (uuid), `cardDataSource` (int enum)
- `amount`: number nullable (partial refund)
- track1/track2/emvTags/pin/pinKsn/emvPaymentAppVersion: card-present, not exposed
- NOTE: `cardDataSource` is required here too — default to `1` (Internet) like sale.

### SettleRequestDto  ⚠ DIVERGENCE
- **required**: `paymentProcessorId` (uuid) — **NOT** a transactionId.
- The API "settle" closes/settles the batch for a payment processor, not a single
  transaction. The source spec's `settle --transaction-id` does not map. **Decision:**
  expose `flute transactions settle --payment-processor-id <uuid>` and document that it
  settles the processor's open batch. Do NOT silently reuse `--transaction-id`.

### TipAdjustmentRequestDto
- `transactionId` (uuid) — from `--transaction-id`
- `tipAmount`: number — from `--tip-amount`

## cardDataSource enum (CardDataSourceDto, integer)
1 = Internet (Virtual Terminal, **ISV API**) ← **CLI default for keyed `--card`**
2 = Swipe (Track1/Track2) · 3 = NFC · 4 = EMV · 5 = EMVContactless · 6 = FallbackSwipe ·
7 = Manual (card-present keyed).
Expose `--card-data-source <int>` to override; default `1`.

## Response DTOs (model permissively; keep unknown fields)

### GetIsvTransactionResponseDto (get / inspect)
Key fields: `transactionId` (uuid), `transactionDateTime` (date-time), `amount` (AmountIsvDto
object — see below), `currencyId`/`currency`, `status`/`statusId`, `authCode`, `responseCode`/
`responseDescription`, `cardDataSource`/`cardDataSourceId`, `customerPan`, `avsResponse`,
`emvTags`, **`availableOperations`: array** (USE THIS for `inspect` — the API already tells
you which operations are valid; do NOT re-derive client-side), `orderNumber`, merchant* fields.

### PageOfGetIsvTransactionsResponseDto (list)
- `items`: array of GetIsvTransactionsResponseDto · `total`: int

### GetIsvTransactionsResponseDto (list item)
`id` (uuid), `date`, `baseAmount`/`totalAmount`/`surchargeAmount`, `currencyCode`/`currencyId`,
`merchant`/`merchantId`, `paymentMethodType`, `customerName`, `customerPan`, `status`/`statusId`,
`type`/`typeId`, `batchId`, `availableOperations`, `amount` (IsvAmountDto).

### AmountIsvDto / IsvAmountDto (amounts are OBJECTS, not scalars)
`{ baseAmount, totalAmount, surchargeAmount, surchargeRate, tipAmount, tipRate,
percentageOffAmount, percentageOffRate, cashDiscountAmount, cashDiscountRate,
totalAmount[, taxAmount, taxRate (IsvAmountDto only)] }` — all number (double).
For table output use `totalAmount`; for quiet/get use `transactionId`/`id`.

## list query params
`page`, `pageSize` (← `--limit`), `asc`, `orderBy`, `createMethodId`, `createdById`,
`batchId`, `noBatch` (← `--unsettled`). The source spec's `--status`/`--from`/`--to` are
NOT server params → apply client-side over the returned page, and document the limitation.

## Modeling guidance
- Request structs: only the CLI-set fields, all `Option<T>` with `skip_serializing_if`,
  `#[serde(rename_all = "camelCase")]`. amount fields use the `to_amount_number` Decimal path.
- Response structs: `#[serde(rename_all = "camelCase")]`, fields `Option<T>`, and a
  `#[serde(flatten)] extra: serde_json::Map<String, serde_json::Value>` to round-trip unknowns
  into the JSON envelope so `--output json` is lossless. Table/quiet read the named fields.
- Amount sub-objects can be modeled as `serde_json::Value` (read `totalAmount` when present)
  to avoid over-modeling — YAGNI.
