# OpenAPI wire-format reference — Settlements + Subscriptions + ISV Tokens (Phase 4)

> **API version: v1.** Targets v1 surface. Reuse the established pattern (Value body-builders,
> Value responses, pure render helpers + golden tests, `to_amount_number`, `send`/`send_no_body`,
> `url::form_urlencoded` for list queries, `--yes` guard + 404-idempotent for deletes).
> Live testing of prior phases proved the OpenAPI "required" list undersells real requirements —
> expect to add fields when testing live.

## Settlements — `/pay-api/v1/settlements/*`

| CLI verb | Method + path | Response |
|---|---|---|
| `settlements list` | GET `/pay-api/v1/settlements/batches` (query) | PageOfGetSettlementBatchesResponseDto `{items,total}` |
| `settlements get <id>` | **client-side filter** over the batch list (no single-batch endpoint) | one batch |

- list query params: `page`, `pageSize`(←`--limit`), `dateFrom`(←`--from`), `dateTo`(←`--to`),
  `statusId`(←`--status`: open→1, settled→2), `batchIds`, `paymentProcessorIds`, `orderBy`, `asc`.
- item GetSettlementBatchesResponseDto: `id, paymentProcessorName, externalBatchId, batchDateTime,
  transactionCount, salesAmount, refundsAmount, netAmount, statusId, statusName`.
- `get <id>`: fetch a page and filter items by `id`; document it's page-bounded (no dedicated endpoint).
- SettlementBatchStatus: 1=Open, 2=Settled.

## Subscriptions — `/sub-api/v1/subscriptions/*`

| CLI verb | Method + path | Body/Response |
|---|---|---|
| `subscriptions create` | POST `/sub-api/v1/subscriptions` | CreateSubscriptionRequestDto |
| `subscriptions get <id>` | GET `/sub-api/v1/subscriptions/{id}` | GetSubscriptionResponseDto |
| `subscriptions list` | GET `/sub-api/v1/subscriptions` (query) | PageOfGetSubscriptionsResponseDto |
| `subscriptions payments <id>` | GET `/sub-api/v1/subscriptions/{id}/payments` | list of GetSubscriptionPaymentsResponseDto |
| `subscriptions terminate <id>` | PUT `/sub-api/v1/subscriptions/{id}/terminate` | **bodyless** |

### CreateSubscriptionRequestDto (only `requesterIpAddress` is schema-required, but the others are effectively required)
- `customerId`: uuid — `--customer-id` (**required** in practice)
- `paymentMethodId`: uuid — `--payment-method-id` (**required**; must be a vaulted+active method)
- `amount`: number — `--amount` (via to_amount_number)
- `currencyId`: int — `--currency-id`, **default 1** (USD)
- `paymentFrequencyUnit`: enum PaymentFrequencyUnitDto — `--interval` accepts `day|week|month`
  (also `daily|weekly|monthly` aliases) → **1=Day, 2=Week, 3=Month**
- `paymentFrequency`: int — `--payment-frequency`, default `1` (every 1 unit)
- `numberOfPayments`: int — `--number-of-payments` (**required**; total scheduled payments)
- `transactionType`: enum SubscriptionTransactionTypeDto — `--transaction-type`, **default 2=Sale**
  (1=Authorization, 2=Sale, 11=AchDebit)
- `requesterIpAddress`: string — `--requester-ip`, **default `"127.0.0.1"`**
- `paymentProcessorId`: uuid — `--payment-processor-id` (optional)
- `paymentStartDateTime`: date-time nullable — `--start-date` (optional; null = immediate/PayNow)
- `secCode` (AchSECCodeDto), `isFasterProcessing` (bool) — only for ACH subscriptions (`--sec-code`/`--faster`)
- `percentageOffRate`, `surchargeRate`, `useCardPrice` — not exposed (YAGNI)

### list query params (server-side): `page`, `pageSize`(←`--limit`), `search`(←`--search`), `customerIds`(←`--customer-id`), `orderBy`, `asc`. `--status` is client-side filter (not a server param).
- list item GetSubscriptionsResponseDto: `subscriptionId, customerName, amountPerPayment,
  paymentFrequencyUnit, paymentFrequency, status, nextPaymentDate, lastPaymentDate,
  successfulPaymentsCount, totalPaymentsCount, numberOfPayments`.
- get GetSubscriptionResponseDto: `id, status, customerId, paymentMethodId, paymentAmount,
  alreadyPaidAmount, allPaymentsAmount, currencyId, paymentFrequencyUnit, paymentFrequency,
  subscriptionStartDate, nextPaymentDate, successfulPaymentsCount, numberOfPayments`.
- payments: array of GetSubscriptionPaymentsResponseDto `{id, status, amount, paymentOrder,
  initialExecutionDateTime, attempts}` (model the response defensively — array or `{items}`).
- NOTE on list id field: list items use `subscriptionId`; get uses `id`. Render with a
  `sub_id(v) = v["id"] or v["subscriptionId"]` helper (mirrors the pos_id pattern).

## ISV Tokens — `/pay-api/v1/merchants/tokens`

| CLI verb | Method + path | Body/Response |
|---|---|---|
| `tokens create` | POST `/pay-api/v1/merchants/tokens` | CreateMerchantApiTokenRequestDto → CreateMerchantApiTokenResponseDto |
| `tokens list` | GET `/pay-api/v1/merchants/tokens?merchantId=` | GetMerchantApiTokensResponseDto `{tokens:[…]}` |
| `tokens revoke` | DELETE `/pay-api/v1/merchants/tokens/{clientId}` | **bodyless** |

- create body CreateMerchantApiTokenRequestDto: **required `merchantId`, `tokenName`** —
  `tokens create --merchant-id <uuid> --name "<display name>"`.
- create response CreateMerchantApiTokenResponseDto: `clientId`, **`clientSecret` (shown ONLY once!)**.
  Render the full response (json lossless); quiet → `clientId`. Document loudly that the secret is
  one-shot (like the webhook signing secret).
- list: `tokens list [--merchant-id <uuid>]` → query `merchantId`; response `{tokens:[{clientId,
  tokenName, merchantId, creationDate}]}`. Table cols: CLIENT ID, NAME, MERCHANT, CREATED. quiet → clientId.
- revoke: `tokens revoke --client-id <uuid>` (require `--yes`; bodyless DELETE; 404-idempotent).

## Rendering
- Add pure render helpers per the established pattern: settlement list/get (table cols: ID, PROCESSOR,
  BATCH DATE, TXNS, SALES, REFUNDS, NET, STATUS), subscription get/list/payments, token create/list.
  json = Envelope (objects "settlement"/"subscription"/"subscription_payments"/"api_token"/"…list");
  quiet = id (settlement id / subscription id / clientId). "—" fallbacks, no unwrap.
- terminate/revoke: bodyless; terminate returns a body (render), revoke is delete-style (require --yes).
