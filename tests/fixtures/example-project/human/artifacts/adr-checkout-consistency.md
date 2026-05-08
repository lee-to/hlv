---
id: adr-checkout-consistency
type: adr
status: accepted
owners: [backend]
depends_on:
  - spec-checkout
affects:
  - architecture-checkout
  - code-checkout
---
# Checkout Consistency ADR

Use optimistic consistency checks for checkout writes.
