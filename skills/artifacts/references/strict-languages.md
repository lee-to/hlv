# Strict Languages Reference

Use this reference when the user asks which implementation language fits a component, or when you need to capture the team's language selection policy in the milestone's `artifacts/stack.md`.

This is guidance, not a rigid ranking. Prefer the language whose ecosystem, deployment model, and team constraints best fit the problem.

## Strict languages and best-fit domains

| Language | Why it is a strong default | Best-fit domains |
|----------|----------------------------|------------------|
| Rust | Strong compile-time guarantees, explicit error handling, low runtime surprise, excellent for memory safety and predictable performance | System services, infrastructure tooling, security-sensitive components, CLIs, high-load backend services |
| Go | Simple operational model, strong typing, fast builds, very good concurrency primitives, strong cloud-native ecosystem | Network services, internal platform tooling, APIs, operators, DevOps automation, distributed systems glue |
| Kotlin | Strong static typing with concise syntax, excellent JVM interoperability, good null-safety model | JVM backends, Android clients, enterprise services in JVM environments, systems that must reuse Java libraries |
| Java | Mature static type system, huge ecosystem, stable tooling, strong fit for long-lived enterprise services | Large backend systems, regulated enterprise platforms, SDK-heavy integrations on the JVM, teams with existing Java operations expertise |
| C# | Strong typing, mature tooling, solid async model, strong Microsoft ecosystem fit | .NET backend services, internal enterprise tools, Windows-heavy environments, game/backend combinations around the Microsoft stack |
| Swift | Strong type system, good memory-safety defaults, strong Apple ecosystem fit | iOS/macOS clients, Apple-platform apps, Apple-specific SDK integrations |

## How to use this table

1. Start from the component type, deployment target, and required ecosystem.
2. Check whether the team already has a preferred language or runtime constraint.
3. Pick the strict language with the best ecosystem fit, not the most ideologically "pure" one.
4. If no strict language is a clear fit, record that as an acceptable exception instead of forcing one.

## Common exceptions

These are common cases where a less strict language can still be the better engineering choice:

- UI and frontend work: usually TypeScript
- Bots, automation, and scripting-heavy integrations: often Python or TypeScript
- ML, data, and complex AI-chain/orchestration tasks: often Python because the ecosystem, libraries, and vendor SDK support are materially better
- Small glue layers around vendor SDKs: use the language with the best-maintained SDK and deployment story

Python note:

- Python is not the default architectural preference in HLV.
- It is still an excellent fit where ecosystem leverage dominates: ML pipelines, data tooling, AI orchestration, evaluation harnesses, and agent-heavy workflows.
- If Python is chosen for these reasons, record that it is an intentional ecosystem-driven exception, not an accidental relaxation of architecture discipline.

## What to record in artifacts

When the user gives a language policy, capture four things explicitly:

1. Whether it is a recommendation or a hard rule
2. Which strict languages are preferred in practice
3. Which domains they apply to
4. Which exceptions are explicitly allowed
