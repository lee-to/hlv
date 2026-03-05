# Scenario: Checkout Partial Failure

id: scenario.checkout.partial_failure
version: 1.0.0
covers_contracts: [order.create]
priority: P0

## Intent

Order placement fails due to out-of-stock items. System rolls back all changes, no order is created.

## Preconditions

- User exists and is authenticated
- At least one cart item has insufficient stock

## Steps

| # | Actor | Action | Expected |
|---|-------|--------|----------|
| 1 | user | POST /orders with items where quantity > stock | Request accepted |
| 2 | system | Check inventory stock | stock < quantity for at least 1 item |
| 3 | system | Roll back all side effects | No partial writes |
| 4 | system | Return error | 409 OUT_OF_STOCK with details |

## Postconditions

- Order count in DB unchanged
- Inventory stock unchanged
- Response contains product_id, available, requested
- Response is deterministic and machine-parseable

## Acceptance Criteria

| ID | Statement |
|----|-----------|
| AC-CHECKOUT-FAIL-001 | Response 409 with code=OUT_OF_STOCK |
| AC-CHECKOUT-FAIL-002 | No partial writes in DB after failure |
| AC-CHECKOUT-FAIL-003 | Error details contain product_id, available, requested |
