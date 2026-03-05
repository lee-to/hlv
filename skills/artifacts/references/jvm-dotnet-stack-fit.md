# JVM and .NET Stack Fit Reference

Use this reference when the user is considering Java, Kotlin, C#, or another JVM/.NET-adjacent stack and the question is no longer "which language class fits?" but "which runtime/framework stack best matches the task?"

This is a fit guide, not a ranking. Pick the stack that matches the deployment model, team expertise, SDK requirements, and operational constraints.

## Application stack fit

| Stack | Strong default for | Best-fit domains | Main tradeoff |
|-------|---------------------|------------------|---------------|
| Java + Spring Boot | Conservative default for JVM backend teams | Enterprise backends, integration-heavy services, regulated systems, organizations with existing Spring expertise | Heavier footprint and more framework surface area than lighter JVM options |
| Kotlin + Spring Boot | JVM backend teams that want Spring ecosystem plus better ergonomics | Enterprise services, API backends, teams already standardized on Spring but preferring Kotlin syntax and null-safety | Still inherits Spring complexity and runtime characteristics |
| Kotlin + Ktor | Kotlin-first teams that want a lighter, code-centric stack | API services, internal tools, smaller services, teams that want less framework magic | Smaller ecosystem and fewer out-of-the-box enterprise conventions than Spring |
| Java or Kotlin + Micronaut | Services where startup time and memory profile matter but JVM ecosystem is still required | Microservices, containerized backends, moderate serverless use, services with many small instances | More opinionated than Ktor, smaller mindshare than Spring |
| Java or Kotlin + Quarkus | JVM services optimized for container startup and cloud-native deployments | Kubernetes workloads, services targeting low startup/memory overhead, teams considering native images | Build/runtime model can be more specialized; native-image constraints may add friction |
| C# + ASP.NET Core | Default for modern .NET web/service work | APIs, internal platforms, enterprise backends, Microsoft-heavy organizations, auth-heavy systems | Best fit often assumes .NET hosting/tooling competence in the team |
| C# + .NET Worker Service | Background processing and integration-heavy backend jobs | Queue consumers, schedulers, ETL, daemon processes, long-running integrations | Not a web stack by itself; often paired with ASP.NET Core or separate service hosting |
| F# + ASP.NET Core / Giraffe | Teams that explicitly want functional style on .NET | Domain-heavy backends, smaller expert teams, internal systems with strong modeling needs | Niche skill set; weaker default choice unless the team already knows F# |

## Runtime and deployment fit

| Runtime mode | Use when | Good fit | Main caution |
|-------------|----------|----------|--------------|
| Standard JVM (HotSpot/OpenJDK) | You want the most boring and compatible deployment path | Long-lived services, enterprise backends, broad library compatibility | Higher cold-start and memory footprint than native/AOT options |
| GraalVM Native Image | Startup time and memory matter enough to justify extra build constraints | CLIs, serverless handlers, scale-to-zero services, container workloads with tight startup budgets | Reflection, proxies, and some libraries need extra care |
| Standard .NET runtime | You want the default operational path with full framework compatibility | APIs, workers, enterprise services, internal tools | Larger deployment/runtime footprint than Native AOT |
| .NET Native AOT | Startup time, binary size, or cold-start latency matter and the app can live within AOT constraints | CLIs, serverless, edge workers, simple APIs, small service binaries | Reflection-heavy libraries and dynamic features can become painful |

## Quick selection heuristics

1. If the organization already runs Spring well and needs boring enterprise delivery, stay in Spring unless there is a concrete reason not to.
2. If the team is Kotlin-first and wants less framework magic, look at Ktor before adopting a heavier framework by default.
3. If startup time and container density matter on the JVM, compare Micronaut and Quarkus before defaulting to Spring.
4. If the environment is Microsoft-heavy, `C# + ASP.NET Core` is usually the boring default.
5. If the service is mostly background jobs or integration plumbing on .NET, consider `Worker Service` rather than forcing a web framework.
6. If native/AOT is attractive, validate library compatibility early and record it as a constraint in artifacts.

## What to capture in artifacts

When the user chooses a JVM/.NET stack, record:

1. Language and platform: `Java`, `Kotlin`, `C#`, etc.
2. Framework: `Spring Boot`, `Ktor`, `ASP.NET Core`, `Micronaut`, `Quarkus`, `Worker Service`, etc.
3. Runtime mode: standard runtime, native image, AOT, serverless runtime
4. Why this stack fits better than nearby alternatives
5. Any operational or library constraints that forced the choice
