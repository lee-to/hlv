# order.create v1.2.0
owner: commerce

## Sources

- [checkout-feature](../artifacts/tasks/checkout-feature.md) — main task, feature description
- [why-optimistic-locking](../artifacts/decisions/why-optimistic-locking.md) — ADR on concurrent access
- [db-constraints](../artifacts/infra/db-constraints.md) — database constraints

## Intent

Create an order atomically from user cart items.

Called when the user clicks "Place Order" on the checkout page. User is already authenticated. Order is created with status `created` — payment happens in the next step via a separate `payment.initiate` contract.

> **Source**: [checkout-feature](../artifacts/tasks/checkout-feature.md)
> "User clicks 'Place Order', system creates an order from cart items. Payment is a separate step after order creation."

## Input

```yaml
type: object
required: [user_id, items]
properties:
  user_id:
    $ref: "glossary#UserId"
  items:
    type: array
    minItems: 1
    items:
      $ref: "glossary#OrderItem"
```

## Output

```yaml
type: object
required: [order]
properties:
  order:
    type: object
    required: [id, user_id, items, total, status, created_at]
    properties:
      id:
        $ref: "glossary#OrderId"
      user_id:
        $ref: "glossary#UserId"
      items:
        type: array
        items:
          $ref: "glossary#OrderItem"
      total:
        $ref: "glossary#Money"
      status:
        type: string
        enum: [created]
      created_at:
        type: string
        format: date-time
```

## Errors

| Code | HTTP | When | Source |
|------|------|------|--------|
| OUT_OF_STOCK | 409 | Product stock is less than requested quantity. Response includes `product_id`, `available`, `requested`. | [checkout-feature](../artifacts/tasks/checkout-feature.md): "show which one exactly and how much is left" |
| INVALID_QUANTITY | 400 | Item quantity is <= 0 or not an integer. | Schema validation |
| USER_NOT_FOUND | 404 | User with the specified `user_id` does not exist in the system. | — |
| EMPTY_CART | 400 | The `items` array is empty. | Schema validation |

## Invariants

1. **Atomicity**: writing the order, order items, and reserving inventory — a single transaction. Either all three operations succeed or all are rolled back. No partial writes ever.
   > **Source**: [checkout-feature](../artifacts/tasks/checkout-feature.md): "system creates an order from cart items"
   > **Constraint**: [db-constraints](../artifacts/infra/db-constraints.md): "no more than 3 tables in a single transaction" — exactly 3 tables: orders, order_items, inventory.

2. **Non-negative total**: order total is always >= 0. Discounts cannot make the total negative.

3. **Stock depletion idempotency**: after a successful order, `stock(product_id)` decreases by exactly `quantity` from the request. No more, no less.

## Examples

### Happy path

```json
// Request
{
  "user_id": "6f4993d8-7680-4e40-b89f-6f4dcbfd8db8",
  "items": [
    {"product_id": "a1b2c3d4-0000-0000-0000-000000000001", "quantity": 2},
    {"product_id": "a1b2c3d4-0000-0000-0000-000000000002", "quantity": 1}
  ]
}

// Response 200
{
  "order": {
    "id": "0504fd7f-f532-4a8f-ae7c-b896782025f9",
    "user_id": "6f4993d8-7680-4e40-b89f-6f4dcbfd8db8",
    "items": [
      {"product_id": "a1b2c3d4-0000-0000-0000-000000000001", "quantity": 2},
      {"product_id": "a1b2c3d4-0000-0000-0000-000000000002", "quantity": 1}
    ],
    "total": {"amount": 74.97, "currency": "USD"},
    "status": "created",
    "created_at": "2026-03-04T12:00:00Z"
  }
}
```

### Out of stock

```json
// Request — requesting 100 units, only 3 in stock
{
  "user_id": "6f4993d8-7680-4e40-b89f-6f4dcbfd8db8",
  "items": [
    {"product_id": "a1b2c3d4-0000-0000-0000-000000000001", "quantity": 100}
  ]
}

// Response 409
{
  "error": {
    "code": "OUT_OF_STOCK",
    "message": "Insufficient stock for product",
    "details": {
      "product_id": "a1b2c3d4-0000-0000-0000-000000000001",
      "available": 3,
      "requested": 100
    }
  }
}
```

### User not found

```json
// Request — non-existent user_id
{
  "user_id": "00000000-0000-0000-0000-000000000000",
  "items": [{"product_id": "a1b2c3d4-0000-0000-0000-000000000001", "quantity": 1}]
}

// Response 404
{
  "error": {
    "code": "USER_NOT_FOUND",
    "message": "User not found"
  }
}
```

## Edge Cases

1. **Concurrent access**: two users simultaneously buy the last item. Optimistic locking via version field in inventory is used. One gets the order, the other gets OUT_OF_STOCK. This is expected behavior.
   > **Decision**: [why-optimistic-locking](../artifacts/decisions/why-optimistic-locking.md)

2. **Duplicate product_id**: if items contain the same product_id twice, quantities are summed for stock check. `[{pid: X, qty: 2}, {pid: X, qty: 3}]` → check stock(X) >= 5.

3. **Transaction within limit**: the operation touches exactly 3 tables (orders, order_items, inventory), which matches the DBA limit.
   > **Source**: [db-constraints](../artifacts/infra/db-constraints.md): "no more than 3 tables in a single transaction"

## NFR

```yaml
latency_p99_ms: 200       # source: checkout-feature.md — "200ms p99"
availability_slo: 99.9
throughput_rps_min: 300
max_query_time_ms: 100    # source: db-constraints.md — "hard limit from DBA"
```

## Security

- Authentication required (authn_required)
- Only parameterized queries to DB (prepared_statements_only)
- Secrets and tokens are never logged (no_secrets_in_logs)
- PII is masked in logs and traces (pii_masking_enabled)
- Rate limiting on endpoint (request_rate_limit_applied)
