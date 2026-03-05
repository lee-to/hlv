# Test Spec: order.create

derived_from: [human/milestones/001/contracts/order.create.md](../../human/milestones/001/contracts/order.create.md)
contract_version: 1.2.0
generated: 2026-03-04

## Contract Tests

Each test verifies a specific scenario from the contract.

### CT-ORDER-CREATE-001: Happy path — single item

- **Input**: valid user_id, 1 item with sufficient stock
- **Expected**: 200, order with status=created, total > 0
- **Assertions**:
  - response.order.id is UUID
  - response.order.status == "created"
  - response.order.total.amount > 0
  - response.order.total.currency in [USD, EUR, RUB]
  - response.order.items == request.items
  - response.order.user_id == request.user_id
- **Gate**: GATE-CONTRACT-001

### CT-ORDER-CREATE-002: Happy path — multiple items

- **Input**: valid user_id, 3 items with sufficient stock
- **Expected**: 200, order.total == sum of item prices
- **Assertions**:
  - response.order.items.length == 3
  - response.order.total.amount == sum(item.price * item.quantity)
- **Gate**: GATE-CONTRACT-001

### CT-ORDER-CREATE-003: Error — OUT_OF_STOCK

- **Input**: valid user_id, 1 item with quantity > stock
- **Expected**: 409, error.code == "OUT_OF_STOCK"
- **Assertions**:
  - response.error.code == "OUT_OF_STOCK"
  - response.error.details.product_id == requested product
  - response.error.details.available < response.error.details.requested
  - No order created in database
  - Inventory unchanged
- **Gate**: GATE-CONTRACT-001

### CT-ORDER-CREATE-004: Error — USER_NOT_FOUND

- **Input**: non-existent user_id, valid items
- **Expected**: 404, error.code == "USER_NOT_FOUND"
- **Assertions**:
  - response.error.code == "USER_NOT_FOUND"
  - No order created
- **Gate**: GATE-CONTRACT-001

### CT-ORDER-CREATE-005: Error — INVALID_QUANTITY

- **Input**: valid user_id, item with quantity=0
- **Expected**: 400, error.code == "INVALID_QUANTITY"
- **Gate**: GATE-CONTRACT-001

### CT-ORDER-CREATE-006: Error — EMPTY_CART

- **Input**: valid user_id, items=[]
- **Expected**: 400, error.code == "EMPTY_CART"
- **Gate**: GATE-CONTRACT-001

## Property-Based Tests

Each test verifies a contract invariant across a wide range of inputs.

### PBT-ORDER-INV-001: Atomicity

- **Invariant**: order write + order_items write + inventory write — all or nothing
- **Generator**: random valid orders + random injection of failures (DB timeout, constraint violation)
- **Assertion**: after each attempt, either (order exists AND items exist AND stock decremented) OR (no order AND no items AND stock unchanged)
- **Min generations**: 10,000
- **Gate**: GATE-PBT-001

### PBT-ORDER-INV-002: Non-negative total

- **Invariant**: output.order.total.amount >= 0
- **Generator**: random items with random quantities (1..1000) and random prices (0.01..99999.99)
- **Assertion**: response.order.total.amount >= 0 for every successful order
- **Min generations**: 10,000
- **Gate**: GATE-PBT-001

### PBT-ORDER-INV-003: Stock depletion idempotency

- **Invariant**: stock decreases by exactly requested quantity
- **Generator**: random product with known stock, random valid quantity
- **Assertion**: stock_after == stock_before - quantity
- **Min generations**: 10,000
- **Gate**: GATE-PBT-001

## Edge Case Tests

### EC-ORDER-001: Concurrent access — optimistic locking

- **Setup**: product with stock=1, two concurrent requests each with quantity=1
- **Expected**: one succeeds (200), one fails (409 OUT_OF_STOCK)
- **Assertion**: exactly 1 order created, stock == 0
- **Gate**: GATE-CONTRACT-001

### EC-ORDER-002: Duplicate product_id in items

- **Input**: items with same product_id twice (qty=2 + qty=3)
- **Expected**: stock check against sum (5), not individual quantities
- **Gate**: GATE-CONTRACT-001

## Performance Tests

### PERF-ORDER-001: Latency under load

- **Target**: p99 <= 200ms
- **Load profile**: 300 RPS sustained for 60 seconds
- **Assertions**:
  - p99_latency_ms <= 200
  - p95_latency_ms <= 150
  - error_rate <= 0.1%
- **Gate**: GATE-PERF-001

### PERF-ORDER-002: Query time

- **Target**: single query <= 100ms
- **Method**: DB query instrumentation
- **Gate**: GATE-PERF-001

## Security Tests

### SEC-ORDER-001: SQL injection resistance

- **Method**: SAST scan + manual test with injection payloads in user_id and product_id
- **Assertion**: all queries use prepared statements
- **Gate**: GATE-SECURITY-001

### SEC-ORDER-002: Auth required

- **Method**: request without auth token
- **Expected**: 401 Unauthorized
- **Gate**: GATE-SECURITY-001

### SEC-ORDER-003: PII masking in logs

- **Method**: create order, inspect logs
- **Assertion**: user_id is masked in log output
- **Gate**: GATE-SECURITY-001

## Gate Mappings

| Test ID | Gate |
|---------|------|
| CT-ORDER-CREATE-001..006 | GATE-CONTRACT-001 |
| PBT-ORDER-INV-001..003 | GATE-PBT-001 |
| EC-ORDER-001..002 | GATE-CONTRACT-001 |
| PERF-ORDER-001..002 | GATE-PERF-001 |
| SEC-ORDER-001..003 | GATE-SECURITY-001 |
