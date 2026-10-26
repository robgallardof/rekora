# Fixtures

Synthetic, original sample code used by integration tests (no third-party or private code).
Tests copy a fixture into a temporary Git repository before indexing, so these files are never
modified in place. They are not built by the tests: indexing must not need `dotnet`, `npm` or
`python`.

| Fixture | Used for |
|---|---|
| `csharp-service` | .NET Web API style service: `OrderService`, `OrderLine`, `BusinessScope`, `IOrderRepository` |
| `typescript-web` | Vue + TypeScript front end: `OrderList`, `OrderEditor`, shared API contract |
| `rust-lib` | `trait Storage`, `struct Memory`, two modules |
| `python-package` | classes/functions plus dynamic attributes that must stay `unknown` |
| `hostile-inputs` | prompt-injection text (secret-like values are generated inside tests instead) |
