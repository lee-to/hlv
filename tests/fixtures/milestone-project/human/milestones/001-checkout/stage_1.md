# Stage 1: Foundation (~25K)

## Contracts
- order.create (this milestone)
- order.cancel (this milestone)

## Tasks

TASK-001 Domain Types & Glossary
  contracts: [order.create, order.cancel]
  output: llm/src/domain/

TASK-002 order.create handler
  depends_on: [TASK-001]
  contracts: [order.create]
  output: llm/src/features/order_create/

TASK-003 order.cancel handler
  depends_on: [TASK-001]
  contracts: [order.cancel]
  output: llm/src/features/order_cancel/

TASK-004 Global constraints
  depends_on: [TASK-002, TASK-003]
  contracts: [order.create, order.cancel]
  output: llm/src/middleware/

## Remediation
