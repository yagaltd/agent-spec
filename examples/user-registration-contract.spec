spec: task
name: "User Registration API"
tags: [example, api, contract]
---

## Intent

Implement a deterministic user registration API contract that an agent can code against
and a verifier can check with explicit test selectors.

## Decisions

- Use `POST /api/v1/users/register` as the only public entrypoint
- Persist a new user only after password hashing succeeds
- Return stable error codes instead of free-form error strings

## Boundaries

### Allowed Changes
- crates/api/**
- crates/domain/user/**
- tests/integration/register_api.rs

### Forbidden
- Do not change the existing login endpoint contract
- Do not create a session during registration
- Do not return raw database errors to clients

## Completion Criteria

Scenario: Successful registration
  Test: test_register_api_returns_201_for_new_user
  Given no user with email "alice@example.com" exists
  When client submits the registration request:
    | field    | value             |
    | email    | alice@example.com |
    | password | Str0ng!Pass#2026  |
  Then response status should be 201
  And response body should contain "user_id"

Scenario: Duplicate email is rejected
  Test: test_register_api_rejects_duplicate_email
  Given a user with email "alice@example.com" already exists
  When client submits the registration request:
    | field    | value             |
    | email    | alice@example.com |
    | password | Str0ng!Pass#2026  |
  Then response status should be 409
  And response body should contain "USER_ALREADY_EXISTS"

## Out of Scope

- Email verification delivery
- Session creation
- Password reset flow
