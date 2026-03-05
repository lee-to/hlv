# Contract: order.create

## Purpose
Create an order from cart items.

## Input
- user_id: UserId (required)
- items: OrderItem[] (required, non-empty)

## Output
- order_id: OrderId
- status: OrderStatus (created)

## Errors
| Code | HTTP | Description |
|------|------|-------------|
| OUT_OF_STOCK | 409 | Item out of stock |
| INVALID_QUANTITY | 400 | Quantity must be positive |

## Invariants
- **atomicity**: Order is created atomically — no partial state on failure.

## NFR
- p99 latency: 200ms
- Idempotent: yes
