# Test Spec: order.cancel

derived_from: [human/milestones/001/contracts/order.cancel.md](../../human/milestones/001/contracts/order.cancel.md)
contract_version: 1.0.0
generated: 2026-03-05

## Contract Tests

Each test verifies a specific scenario from the contract.

### CT-ORDER-CANCEL-001: Happy path — cancel created order

- **Input**: existing order in status=created, requested_by=owner, valid reason
- **Expected**: 200, order with status=cancelled, cancelled_at present
- **Assertions**:
  - response.order.id == request.order_id
  - response.order.status == "cancelled"
  - response.order.cancelled_at is valid ISO 8601
  - Order status in database == "cancelled"
  - Inventory restored: stock increased by order item quantities
- **Gate**: GATE-CONTRACT-001

### CT-ORDER-CANCEL-002: Error — ORDER_NOT_FOUND

- **Input**: non-existent order_id, valid user
- **Expected**: 404, error.code == "ORDER_NOT_FOUND"
- **Assertions**:
  - response.error.code == "ORDER_NOT_FOUND"
  - No state changes in database
- **Gate**: GATE-CONTRACT-001

### CT-ORDER-CANCEL-003: Error — ORDER_NOT_CANCELLABLE (paid)

- **Input**: existing order in status=paid, requested_by=owner
- **Expected**: 409, error.code == "ORDER_NOT_CANCELLABLE"
- **Assertions**:
  - response.error.code == "ORDER_NOT_CANCELLABLE"
  - response.error.details.current_status == "paid"
  - Order status unchanged in database
  - Inventory unchanged
- **Gate**: GATE-CONTRACT-001

### CT-ORDER-CANCEL-004: Error — ORDER_NOT_CANCELLABLE (already cancelled)

- **Input**: existing order in status=cancelled, requested_by=owner
- **Expected**: 409, error.code == "ORDER_NOT_CANCELLABLE"
- **Assertions**:
  - response.error.code == "ORDER_NOT_CANCELLABLE"
  - response.error.details.current_status == "cancelled"
- **Gate**: GATE-CONTRACT-001

### CT-ORDER-CANCEL-005: Error — FORBIDDEN (not owner, not admin)

- **Input**: existing order in status=created, requested_by=other_user (not owner, not admin)
- **Expected**: 403, error.code == "FORBIDDEN"
- **Assertions**:
  - response.error.code == "FORBIDDEN"
  - Order status unchanged
  - Inventory unchanged
- **Gate**: GATE-CONTRACT-001

### CT-ORDER-CANCEL-006: Happy path — admin cancels order

- **Input**: existing order in status=created, requested_by=admin_user
- **Expected**: 200, order with status=cancelled
- **Assertions**:
  - response.order.status == "cancelled"
  - Admin is not owner but has admin role
- **Gate**: GATE-CONTRACT-001

## Property-Based Tests

Each test verifies a contract invariant across a wide range of inputs.

### PBT-CANCEL-INV-001: Terminal state respected

- **Invariant**: terminal(order.status_before) → output.error == ORDER_NOT_CANCELLABLE
- **Generator**: random orders with statuses from [created, paid, cancelled, failed], random users (owner/non-owner/admin)
- **Assertion**: if status in [paid, cancelled, failed] then error.code == "ORDER_NOT_CANCELLABLE", never a successful cancellation
- **Min generations**: 10,000
- **Gate**: GATE-PBT-001

### PBT-CANCEL-INV-002: Valid status transition

- **Invariant**: allowed_transition(status_before, status_after) → only created→cancelled
- **Generator**: random orders in all possible statuses, random valid cancel requests
- **Assertion**: the only successful status change is created→cancelled; all other starting statuses result in error
- **Min generations**: 10,000
- **Gate**: GATE-PBT-001

### PBT-CANCEL-INV-003: Inventory release correctness

- **Invariant**: upon successful cancellation, stock(product_id) increases by exactly quantity for each item
- **Generator**: random orders with 1-10 items, random quantities (1..100), known initial stock
- **Assertion**: for each item, stock_after == stock_before + quantity
- **Min generations**: 10,000
- **Gate**: GATE-PBT-001

## Edge Case Tests

### EC-CANCEL-001: Concurrent cancellation — optimistic locking

- **Setup**: order in status=created, two concurrent cancel requests from owner
- **Expected**: one succeeds (200), one fails (409 ORDER_NOT_CANCELLABLE)
- **Assertion**: exactly 1 status change, inventory restored exactly once
- **Gate**: GATE-CONTRACT-001

### EC-CANCEL-002: Cancel order with many items

- **Setup**: order with 50 items, each with different quantities
- **Expected**: all 50 inventory entries restored atomically
- **Assertion**: stock restored for all items or none
- **Gate**: GATE-CONTRACT-001

## Performance Tests

### PERF-CANCEL-001: Latency under load

- **Target**: p99 <= 150ms
- **Load profile**: 200 RPS sustained for 60 seconds
- **Assertions**:
  - p99_latency_ms <= 150
  - p95_latency_ms <= 100
  - error_rate <= 0.1%
- **Gate**: GATE-PERF-001

### PERF-CANCEL-002: Query time

- **Target**: single query <= 100ms
- **Method**: DB query instrumentation during cancel operations
- **Gate**: GATE-PERF-001

## Security Tests

### SEC-CANCEL-001: Auth required

- **Method**: cancel request without auth token
- **Expected**: 401 Unauthorized
- **Gate**: GATE-SECURITY-001

### SEC-CANCEL-002: Authz enforcement

- **Method**: cancel request from non-owner, non-admin user
- **Expected**: 403 Forbidden
- **Assertion**: no state change
- **Gate**: GATE-SECURITY-001

### SEC-CANCEL-003: PII masking in logs

- **Method**: cancel order, inspect logs
- **Assertion**: user_id and order_id are masked in log output
- **Gate**: GATE-SECURITY-001

## Gate Mappings

| Test ID | Gate |
|---------|------|
| CT-ORDER-CANCEL-001..006 | GATE-CONTRACT-001 |
| PBT-CANCEL-INV-001..003 | GATE-PBT-001 |
| EC-CANCEL-001..002 | GATE-CONTRACT-001 |
| PERF-CANCEL-001..002 | GATE-PERF-001 |
| SEC-CANCEL-001..003 | GATE-SECURITY-001 |
