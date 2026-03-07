## Roadmap Specs

`specs/roadmap/` contains staged self-hosting task specs for future `agent-spec` work.

These files are real task contracts, but they are not part of the default top-level
`agent-spec guard --spec-dir specs --code .` run until they are promoted into
the top-level `specs/` directory.

Promotion rule:

- draft or future-phase contracts stay in `specs/roadmap/`
- active implementation contracts move to top-level `specs/`

Nested roadmap specs still inherit the top-level [`project.spec`](../project.spec).

Use:

```bash
agent-spec contract specs/roadmap/task-phase0-contract-fidelity.spec
```

when you want to inspect or refine a future-phase roadmap contract before promotion.
