---
name: randomizer-mocks
description: Add or update repository-local Randomizer HTTP mock endpoints and response contracts by inspecting the actual service specification, outbound clients, response consumers, serialization behavior, enums, booleans, nullability, constraints, tests, and fixtures. Use when asked to mock one or more third-party HTTP endpoints, change an existing mock route or generated response, or synchronize Randomizer mocks after API-client or DTO behavior changes.
---

# Manage Randomizer Mocks

Add or update only the requested HTTP mocks. Keep the operation evidence-based, incremental, and
idempotent across repeated invocations.

## Resolve the requested scope

Accept endpoint scope expressed as methods and paths, service names, client or source files,
features, existing route IDs, or an explicit endpoint list. Treat endpoint details and response
scenarios in the developer's prompt as the requested behavior, but report conflicts with an
authoritative service wire contract instead of silently changing that contract.

If `.randomizer/randomizer.yaml` is absent, run `randomizer init` before continuing. Otherwise load
the existing manifest and fixtures before inspecting application code.

## Inspect the request and actual response contract

Use available repository or codebase tools to trace only the requested outbound calls. Fall back to
direct source inspection when no semantic codebase tool is available. Establish:

- HTTP method, path template, query, headers, and request-body conditions;
- the base-URL setting and its local-development configuration;
- response wrappers, declared fields, and fields consumed by the application;
- serialized field names, exact enum wire values, boolean semantics, requiredness, nullability,
  collections, formats, numeric or length constraints, and error handling;
- tests, fixtures, OpenAPI, JSON Schema, or examples supporting the result.

Prefer evidence in this order:

1. The service's official OpenAPI, JSON Schema, or documentation supplied by the developer or
   available in the repository or through configured tools.
2. Existing recorded service fixtures, examples, or contract tests.
3. Actual serialization metadata, response types, enum definitions, validators, and custom
   serializers.
4. Response-consumer access patterns and error handling.
5. A developer description when no inspectable source is available.

When sources disagree, follow actual local serialization behavior and report the conflict. Ask for
input rather than inventing business states, error behavior, enum values, boolean restrictions,
required fields, or validation bounds.

Build a field evidence table before authoring a contract. For every property record its JSON name,
type, requiredness, nullability, enum or constant values, constraints, and source evidence. Keep
optional and nullable separate: an optional property may be absent, while a nullable property may
be present with `null`.

## Reconcile the manifest

Identify an existing route by service, method, normalized path, and matchers that distinguish it
from sibling routes.

For additions:

- reuse an existing service when its base URL represents the same third party;
- add a service only when required and use a stable lowercase hyphenated ID;
- generate a stable descriptive route ID;
- update the application's local base URL only when adding a new service.

For updates:

- change only the requested routes or response scenarios;
- preserve unrelated services, routes, matchers, fixtures, and application configuration;
- retain existing route IDs unless the developer explicitly requests a rename;
- retain existing scenarios unless the request replaces them;
- do not delete unreferenced fixtures automatically.

Running the same request twice must not create duplicate services, routes, responses, or local
configuration entries.

## Create or update Randomizer contracts

Read [references/contracts.md](references/contracts.md) before creating or changing a contract.
Create a bare Draft 2020-12 JSON Schema under `.randomizer/contracts/` for every requested generated
response shape. Randomizer derives metadata and a canonical content hash when it verifies the
project; do not calculate or add a hash and do not run a contract-import command.

Use stable names such as `<service>-<operation>-<status>.json`. Reuse and update an existing contract
when it represents the same wire response. Preserve contracts unrelated to the requested endpoints.

Represent field behavior exactly:

- use `enum` with exact serialized, case-sensitive wire values;
- use `type: boolean` when both `true` and `false` are accepted, and `const: true` or `const: false`
  only when the service contract fixes the value;
- put properties in `required` only when the response contract guarantees their presence;
- use a type array such as `["string", "null"]` for nullable values;
- use array `items`, object `properties`, numeric bounds, string formats, patterns, and length bounds
  only when supported by evidence;
- use local `$defs` and `$ref` for repeated shapes; never use external references.

Reference the contract from the route response:

```yaml
services:
  - id: users
    config_key: USERS_API_URL

routes:
  - id: get-user
    service: users
    match:
      method: GET
      path: /users/{user_id}
    responses:
      - status: 200
        body:
          contract: .randomizer/contracts/users-get-user-200.json
          mode: valid
```

Use `inline` only for an intentionally fixed body and `fixture` only when the response cannot be
generated safely from supported contract evidence. If a contract cannot be established, stop and
request the missing service specification or example instead of fabricating a response shape.

Point the application's local base URL to
`http://127.0.0.1:<randomizer-port>/mock/<service-id>` while preserving production and shared
environment settings. For browser-originated requests, follow the repository's existing local
proxy or rewrite pattern so requests remain same-origin.

Keep secrets, authorization values, personal data, and production payloads out of committed mocks.
Use deterministic fictional values.

## Verify and report

Run `randomizer verify` after every modification. This compiles each contract and generates a value,
catching invalid schemas and unsupported generation features. Fix all reported contract, manifest,
fixture, and route errors. Run focused application tests when a reliable test seam covers the
changed client behavior.

Report:

- services and routes added or updated;
- contracts, response scenarios, and local configuration changed;
- the field evidence table, including enum and boolean decisions;
- evidence files used;
- unresolved assumptions or conflicts;
- exact commands for `randomizer start`, the application's normal startup, and `randomizer stop`.

Do not start the application or Randomizer unless requested.
