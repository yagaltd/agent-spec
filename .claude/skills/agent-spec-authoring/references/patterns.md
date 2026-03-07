# Patterns

## Minimal task contract

```spec
spec: task
name: "Example Task"
inherits: project
tags: [example]
---

## Intent

Describe what to do and why.

## Decisions

- State technical choices that are already fixed

## Boundaries

### Allowed Changes
- crates/example/**

### Forbidden
- Do not change the public API shape

## Completion Criteria

Scenario: Happy path
  Test:
    Package: example-crate
    Filter: test_happy_path
  Given a stable precondition
  When the user performs the action
  Then the expected result occurs
```

## Selector patterns

Simple selector:

```spec
Scenario: Happy path
  Test: test_happy_path
```

Structured selector:

```spec
Scenario: Happy path
  Test:
    Package: example-crate
    Filter: test_happy_path
```

## Boundary patterns

Machine-enforced paths:

```spec
### Allowed Changes
- crates/spec-parser/**
- tests/parser_contract.rs
```

Natural-language prohibitions:

```spec
### Forbidden
- Do not break the existing JSON shape
- Do not introduce `.unwrap()`
```

Use both when needed. The path-like entries are the ones boundary verification can enforce mechanically.
