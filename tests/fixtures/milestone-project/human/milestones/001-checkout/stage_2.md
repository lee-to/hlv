# Stage 2: Integration + Observability (~20K)

## Contracts
- order.create (from stage 1)
- order.cancel (from stage 1)

## Tasks

TASK-005 Integration Tests
  depends_on: []
  contracts: [order.create, order.cancel]
  output: llm/tests/integration/

TASK-006 Observability Setup
  depends_on: []
  contracts: [order.create, order.cancel]
  output: llm/src/observability/

## Remediation
