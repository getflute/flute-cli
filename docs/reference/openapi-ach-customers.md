# OpenAPI wire-format reference — ACH + Customers (Phase 2)

Pinned from the UAT OpenAPI ("ISV API BFF"). Field names/casing authoritative.
Amounts: parse `--amount` as `Decimal`, emit via `to_amount_number` (exact JSON number).
Model request bodies as `serde_json::Value` builders; responses as `serde_json::Value`
(lossless), reusing the transactions pattern (`render_transaction` etc.).

## ACH — `/pay-api/v1/transactions/ach/*`

| CLI verb | Method + path | Body DTO |
|---|---|---|
| `ach debit` | POST `/pay-api/v1/transactions/ach/payment` | CreateIsvAchPaymentRequestDto |
| `ach credit` | POST `/pay-api/v1/transactions/ach/payment/credit` | CreateIsvAchRefundRequestDto (same shape) |
| `ach void <id>` | POST `/pay-api/v1/transactions/ach/{id}/void` | **bodyless** (id in path) |
| `ach refund <id>` | POST `/pay-api/v1/transactions/ach/{id}/refund` | **bodyless** (id in path) |

### CreateIsvAchPaymentRequestDto / CreateIsvAchRefundRequestDto (debit/credit share this shape)
**required: `amount`, `paymentProcessorId`, `requesterIpAddress`, `secCode`** — note three of
these are NOT in the spec's CLI flags (same gap as `currencyId` for card). Expose and default them:
- `amount`: number — `--amount`
- `paymentProcessorId`: uuid — `--payment-processor-id` (**required**; no default)
- `requesterIpAddress`: string — `--requester-ip`, **default `"127.0.0.1"`** (the end-customer IP; CLI default is fine for dev)
- `secCode`: int enum AchSECCodeDto — `--sec-code`, **default `1`** (Web). Values: 1=Web, 2=PPD, 3=CCD, 4=Telephone
- `routingNumber`: string nullable — `--routing`
- `accountNumber`: string nullable — `--account`
- `accountType`: int enum AccountTypeDto — `--account-type` accepts `checking`/`savings` → **1=Checking, 2=Savings** (default checking)
- `accountHolderType`: int enum AccountHolderTypeDto — `--account-holder-type` accepts `business`/`personal` → **1=Business, 2=Personal** (optional, omit if not given)
- `taxId`: string nullable — `--tax-id` (optional)
- `customerId`/`paymentMethodId`: uuid nullable — `--customer-id`/`--payment-method-id` (vaulted ACH)
- `isFasterProcessing`: bool — `--faster` (default false)
- `billingAddress` (AddressIsvDto): `--billing-line1`→line1, `--billing-line2`→line2, `--billing-city`→city, `--billing-state`→stateName, `--billing-postal-code`→postalCode, `--billing-country-id`→countryId (i32). Object included only when ≥1 field present.
- `contactInfo` (ContactInfoIsvDto): `--contact-first-name`→firstName, `--contact-last-name`→lastName, `--contact-email`→email, `--contact-phone`→mobileNumber, `--contact-company`→companyName. Object included only when ≥1 field present.
- **LIVE NOTE**: The live sandbox rejects ACH debit/credit without `accountHolderType`, `billingAddress`, and `contactInfo` even though these are not listed as required in the OpenAPI spec. Always supply `--account-holder-type`, at least one `--billing-*` field, and at least one `--contact-*` field for live calls.
- `percentageOffRate`, shipping: not exposed (YAGNI)

`ach void`/`ach refund`: bodyless POST by path id → `self.send(POST, ".../{id}/void", None)`.
Response: AchPaymentResponseIsvDto (handle as Value; render with render_transaction or a small ACH renderer).

## Customers / Vault — `/pay-api/v1/customers/*`

| CLI verb | Method + path | Body |
|---|---|---|
| `customers create` | POST `/customers` | CreateCustomerRequestIsvDto |
| `customers get <id>` | GET `/customers/{customerId}` | — |
| `customers list` | GET `/customers` | query params (below) |
| `customers update <id>` | PUT `/customers/{customerId}` | UpdateCustomerRequestIsvDto |
| `customers delete <id> --yes` | DELETE `/customers/{customerId}` | bodyless |
| `customers add-card <id>` | POST `/customers/{customerId}/payment-methods/cards` | CreatePaymentMethodRequestIsvDto |
| `customers add-ach <id>` | POST `/customers/{customerId}/payment-methods/ach` | CreateAchAccountPaymentMethodRequestIsvDto |
| `customers methods <id>` | GET `/customers/{customerId}/payment-methods` | — |
| `customers remove-method <id> <mid>` | DELETE `/customers/{customerId}/payment-methods/{paymentMethodId}` | bodyless |

### CreateCustomerRequestIsvDto (no required fields)
`firstName`, `lastName`, `companyName`, `email`, `mobilePhoneNumber` (all string nullable),
`isMobileNumberSmsNotificationsEnabled` (bool), `useBillingAsShippingAddress` (bool),
optional nested `billingAddress`/`shippingAddress`. CLI: `--first-name`, `--last-name`, `--email`,
`--company`, `--mobile`. Send only provided fields.

### UpdateCustomerRequestIsvDto
Same basic fields as create. `customers update <id> --email … --first-name …`.
**LIVE NOTE**: The PUT endpoint is a **full replacement** — sending only `--email` wipes `firstName`/`lastName`/etc.
The CLI now does **GET-merge-PUT**: it fetches the current customer record first, overlays only the user-supplied flags, then PUTs the complete merged body. Omitted flags retain their existing server values.

### list query params (server-side!): `page`, `pageSize` (←`--limit`), `orderBy`, `asc`, `search` (←`--search`), `customerIds`, `dateFrom`, `dateTo`
Unlike transactions, `search` IS a real server param — wire `--search`→`search`, `--limit`→`pageSize`.
Response: GetCustomerPageResponseIsvDto `{items?, total?}` (confirm field names live). Item: GetCustomerResponseIsvDto
(`id, firstName, lastName, companyName, email, mobilePhoneNumber, cards, achAccounts, createdOn,
transactionsCount, transactionsVolume, lastTransactionDate, …`).

### CreatePaymentMethodRequestIsvDto (add-card)
`name` (nullable), `pan`, `expirationMonth` (int), `expirationYear` (int), `securityCode` (nullable).
CLI: `add-card <customer-id> --card`(→pan) `--exp MM/YY`(→month/year via parse_exp) `--cvv`(→securityCode) `--name`.

### CreateAchAccountPaymentMethodRequestIsvDto (add-ach)
`name`, `accountNumber`, `routingNumber`, `accountType` (enum), `accountHolderType` (enum), `taxId`.
CLI: `add-ach <customer-id> --routing --account [--account-type checking|savings] [--account-holder-type …] [--name] [--tax-id]`.

### GetCustomerPaymentMethodsResponseIsvDto (methods list item)
`id, name, typeId, typeName, panMask, cardTokenType, expirationMonth, expirationYear,
accountNumber, routingNumber, accountType, accountHolderType, taxId, isDefault, createdOn`.

## Shared enums
- AchSECCodeDto: 1=Web (default), 2=PPD, 3=CCD, 4=Telephone
- AccountTypeDto: 1=Checking, 2=Savings
- AccountHolderTypeDto: 1=Business, 2=Personal

## Output / rendering
- Reuse `render_transaction` for ACH debit/credit/void/refund responses (they're transaction-shaped),
  or add a small `render_customer` / `render_customer_list` / `render_payment_methods` per the
  established pure-helper + golden-test pattern. quiet = resource id; delete/remove-method require `--yes`.
- `delete`/`remove-method` are bodyless DELETE; treat 404 on re-delete as idempotent success.
