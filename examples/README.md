# Examples

These files are authoring references for `agent-spec`.

See [`../README.md`](../README.md) for the main workflow and CLI usage.

- `refund.spec`: legacy-style task spec with explicit `测试:` selectors added for the current quality gate
- `no-unwrap.spec`: minimal structural quality example with an explicit selector
- `user-registration-contract.spec`: English Task Contract example with `Decisions`, `Boundaries`, and step tables
- `refactor-payment-service.spec`: Chinese refactor/code-review example that emphasizes allowed changes and forbidden changes

Notes:

- New task specs should declare an explicit `测试:` / `Test:` selector for every scenario.
- These examples focus on DSL shape and contract structure. Replace the example test selector names with real test selectors before using them in a live repository.
