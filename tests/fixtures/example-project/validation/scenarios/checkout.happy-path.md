# Scenario: Checkout Happy Path

id: scenario.checkout.happy_path
version: 1.0.0
covers_contracts: [order.create]
priority: P0

## Intent

User places an order from cart, all items are in stock, order is created successfully.

## Preconditions

- User exists and is authenticated
- All cart items are in stock with sufficient quantity
- Rate limit is not exceeded

## Steps

| # | Actor | Action | Expected |
|---|-------|--------|----------|
| 1 | user | POST /orders with user_id and items | 200, order with status=created |
| 2 | system | Validate input data | All fields are valid |
| 3 | system | Check inventory stock | stock >= quantity for all items |
| 4 | system | Atomic transaction: orders + order_items + inventory | All three records created |
| 5 | system | Return order payload | order.id, order.total, order.status=created |

## Postconditions

- Exactly one order created in DB with status=created
- Inventory stock decreased by requested quantities
- Order total >= 0
- Response time <= 200ms (p99)

## Acceptance Criteria

| ID | Statement |
|----|-----------|
| AC-CHECKOUT-001 | Checkout returns 200 and order.status=created |
| AC-CHECKOUT-002 | Order is visible in user's order history |
| AC-CHECKOUT-003 | Inventory stock correctly decreased |
