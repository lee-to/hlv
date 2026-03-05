# order.cancel v1.0.0
owner: commerce

## Sources

- [checkout-feature](../artifacts/tasks/checkout-feature.md) — order cancellation rules
- [why-optimistic-locking](../artifacts/decisions/why-optimistic-locking.md) — concurrent access to state transitions

## Intent

Cancel an existing order if it is in a cancellable status.

Called by the user or an administrator. An order can only be cancelled from status `created`. Orders in statuses `paid`, `cancelled`, `failed` are terminal — cancellation is not possible. Upon cancellation, reserved inventory is returned to stock.

> **Source**: [checkout-feature](../artifacts/tasks/checkout-feature.md)
> "Cancellation available only to order owner and admin. Can only cancel from created."

## Input

```yaml
type: object
required: [order_id, requested_by]
properties:
  order_id:
    $ref: "glossary#OrderId"
  requested_by:
    $ref: "glossary#UserId"
  reason:
    type: string
    minLength: 3
    maxLength: 200
```

## Output

```yaml
type: object
required: [order]
properties:
  order:
    type: object
    required: [id, status, cancelled_at]
    properties:
      id:
        $ref: "glossary#OrderId"
      status:
        type: string
        enum: [cancelled]
      cancelled_at:
        type: string
        format: date-time
```

## Errors

| Code | HTTP | When | Source |
|------|------|------|--------|
| ORDER_NOT_FOUND | 404 | Order with the specified `order_id` does not exist in the system. | — |
| ORDER_NOT_CANCELLABLE | 409 | Order is in a terminal status (`paid`, `cancelled`, `failed`). Cancellation is not possible. | [checkout-feature](../artifacts/tasks/checkout-feature.md): "can only cancel from created" |
| FORBIDDEN | 403 | `requested_by` is not the order owner and does not have admin role. | [checkout-feature](../artifacts/tasks/checkout-feature.md): "cancellation available only to order owner and admin" |

## Invariants

1. **Terminal state respected**: if the order is in a terminal status (`paid`, `cancelled`, `failed`), the operation MUST return `ORDER_NOT_CANCELLABLE` error. Never change a terminal status.
   > **Source**: [checkout-feature](../artifacts/tasks/checkout-feature.md): "statuses: created → paid → completed; cancelled is final"

2. **Valid status transition**: the only allowed transition is `created` → `cancelled`. All other transitions are forbidden.

3. **Inventory release**: upon successful cancellation, reserved stock MUST be returned. `stock(product_id)` increases by exactly `quantity` from the order items.

## Examples

### Happy path — cancel created order

```json
// Request
{
  "order_id": "0504fd7f-f532-4a8f-ae7c-b896782025f9",
  "requested_by": "6f4993d8-7680-4e40-b89f-6f4dcbfd8db8",
  "reason": "Changed my mind"
}

// Response 200
{
  "order": {
    "id": "0504fd7f-f532-4a8f-ae7c-b896782025f9",
    "status": "cancelled",
    "cancelled_at": "2026-03-04T12:05:00Z"
  }
}
```

### Error — order not cancellable (paid)

```json
// Request — order already paid
{
  "order_id": "0504fd7f-f532-4a8f-ae7c-b896782025f9",
  "requested_by": "6f4993d8-7680-4e40-b89f-6f4dcbfd8db8",
  "reason": "Changed my mind"
}

// Response 409
{
  "error": {
    "code": "ORDER_NOT_CANCELLABLE",
    "message": "Order cannot be cancelled",
    "details": {
      "order_id": "0504fd7f-f532-4a8f-ae7c-b896782025f9",
      "current_status": "paid"
    }
  }
}
```

### Error — forbidden (not owner)

```json
// Request — different user tries to cancel
{
  "order_id": "0504fd7f-f532-4a8f-ae7c-b896782025f9",
  "requested_by": "00000000-0000-0000-0000-999999999999",
  "reason": "Want to cancel"
}

// Response 403
{
  "error": {
    "code": "FORBIDDEN",
    "message": "Not authorized to cancel this order"
  }
}
```

## Edge Cases

1. **Cancel already cancelled order**: a repeat cancel request for an order in `cancelled` status returns `ORDER_NOT_CANCELLABLE`. Idempotency does not apply — this is a deliberate decision so the client knows the order is already cancelled.

2. **Race condition**: two cancel requests for the same order simultaneously. One successfully cancels, the other gets `ORDER_NOT_CANCELLABLE` (status already `cancelled`). Optimistic locking is used.
   > **Decision**: [why-optimistic-locking](../artifacts/decisions/why-optimistic-locking.md)

3. **Inventory release atomicity**: order cancellation and inventory return is a single transaction. If inventory return fails, the cancellation is rolled back.

## NFR

```yaml
latency_p99_ms: 150       # source: order.cancel — less loaded operation
availability_slo: 99.9
throughput_rps_min: 200
```

## Security

- Authentication required (authn_required)
- Authorization: only order owner or admin (authz_order_scope_check)
- Secrets and tokens are never logged (no_secrets_in_logs)
- PII is masked in logs and traces (pii_masking_enabled)
