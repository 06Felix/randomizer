---
name: randomizer-mocks
description: Add or update repository-local Randomizer HTTP mock endpoints and response contracts by inspecting the real serialized wire behavior, language/framework serializers, enums, date and time values, booleans, nullability, constraints, tests, fixtures, and API specifications. Use when asked to mock HTTP endpoints, change an existing mock route or response, or synchronize Randomizer mocks after client or DTO changes.
---

# Manage Randomizer Mocks

Add or update only the requested HTTP mocks. Keep the operation evidence-based, incremental, and
idempotent. The runtime is language-independent; this skill is the repository-understanding layer.

## Resolve scope and language

Accept endpoint scope as methods and paths, service names, client/source files, features, route IDs,
or an explicit endpoint list. An optional prompt hint may select a playbook:

```text
$randomizer-mocks language=java Add GET /users/{id}
```

If no hint is supplied, detect the language and framework from repository files and from the owner of
the requested outbound client. If several languages are present, inspect only the requested client.
If detection is uncertain or conflicting, ask before making language-specific assumptions.

Load [references/runtime-capabilities.md](references/runtime-capabilities.md) and
[references/generic-wire-contract.md](references/generic-wire-contract.md) for every task. Load one
language playbook only when it matches the detected source:

- Java/JVM: [references/languages/java.md](references/languages/java.md)
- TypeScript/JavaScript: [references/languages/typescript.md](references/languages/typescript.md)
- Python: [references/languages/python.md](references/languages/python.md)
- Go: [references/languages/go.md](references/languages/go.md)
- Rust: [references/languages/rust.md](references/languages/rust.md)

For any other language, use the generic wire-contract workflow and do not invent language mappings.

If `.randomizer/randomizer.yaml` is absent, run `randomizer init` before continuing. Otherwise load
the existing manifest and fixtures before inspecting application code.

## Inspect the actual wire contract

Trace only the requested outbound calls. Establish:

- HTTP method, path template, query, headers, and request-body conditions;
- the base-URL setting and local-development configuration;
- response wrappers, declared fields, and fields consumed by the application;
- serialized JSON names and actual JSON primitive/container types;
- exact enum wire values, boolean semantics, requiredness, omission, nullability, collections,
  date/time representation, formats, numeric/length constraints, and error behavior;
- tests, fixtures, OpenAPI, JSON Schema, or examples supporting every decision.

Prefer evidence in this order:

1. Official OpenAPI, JSON Schema, or service documentation supplied by the developer or available in
   the repository or through configured tools.
2. Recorded service fixtures, examples, contract tests, and integration tests.
3. Actual serializer metadata, response types, enum definitions, validators, and custom serializers.
4. Response-consumer access patterns and error handling.
5. A developer description when no inspectable source exists.

The authoritative object is the serialized wire response, not a source-language type name. When
sources disagree, follow actual local serialization behavior and report the conflict. Ask rather than
inventing business states, enum values, date/time formats, boolean restrictions, required fields, or
validation bounds.

Build a field evidence table before authoring a contract. For every property record:

| JSON name | Wire type | Required? | Nullable? | Enum/const | Constraints | Evidence |
| --- | --- | --- | --- | --- | --- | --- |

Keep optional and nullable separate. A property may be absent, present as `null`, both, or neither.
Language primitives, default values, and consumer dereferences do not prove requiredness.

## Create Randomizer generation request payloads

When the developer asks for a Randomizer `/generate` request payload rather than a project response
contract, create the custom generator schema from explicit constraints and meaningful variable names.
Keep this separate from Draft 2020-12 response contracts.

For example, the name and stated bounds below describe an integer duration in minutes:

```json
{
  "schema": {
    "completion_time_in_minutes": {
      "type": "int",
      "min": 0,
      "max": 60
    }
  }
}
```

Use name semantics such as units (`_in_minutes`, `_in_seconds`), counts, percentages, IDs, flags,
and lists to select an appropriate generator type only when the surrounding code or developer
constraints support that interpretation. Preserve explicit `min`, `max`, `precision`, enum values,
and list bounds. A variable name alone does not justify fabricated numeric ranges; ask for bounds or
use the generator's documented defaults when the developer explicitly requests a generated payload.

Use the custom generator types `int`, `float`, `string`, `enum`, `object`, `boolean`, `uuid`, and
`list`. Include `seed`, `sequence`, or `frequency` only when the requested REST or WebSocket payload
needs deterministic replay or streaming.

## Reconcile the manifest

Identify an existing route by service, method, normalized path, and distinguishing matchers.

For additions:

- reuse a service when its base URL represents the same third party;
- add a service only when required, using a stable lowercase hyphenated ID;
- generate a stable descriptive route ID;
- update the application's local base URL only when adding a new service.

For updates:

- change only requested routes or response scenarios;
- preserve unrelated services, routes, matchers, fixtures, and application configuration;
- retain existing route IDs and scenarios unless explicitly asked to replace them;
- never delete unreferenced fixtures automatically.

Repeated execution must not create duplicate services, routes, responses, or configuration entries.

## Create or update contracts

Read [references/contracts.md](references/contracts.md) before creating or changing a contract. Create
a bare Draft 2020-12 JSON Schema under `.randomizer/contracts/` for every requested generated response.
Do not add Randomizer metadata or hashes and do not run a contract-import command.

Before saving the contract, compare every selected keyword and format with the runtime capability
reference. If a wire shape cannot be safely represented by supported generation, use a fixture or ask
for a serialized example. Never emit an unsupported format merely because a language has a matching
type.

Represent behavior exactly:

- `enum` contains exact serialized, case-sensitive wire values;
- `type: boolean` permits both values; `const` fixes a contractually fixed value;
- only guaranteed properties belong in `required`;
- nullable values include `null` in their type or union branch;
- arrays use evidenced `items` and bounds;
- dates, times, numbers, strings, and custom formats follow observed wire serialization;
- local `$defs` and `$ref` are allowed; external references are forbidden.

Reference contracts from the route:

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

Use `inline` only for intentionally fixed JSON and `fixture` when a supported generated contract
cannot safely represent the actual response. Point the local application base URL to
`http://127.0.0.1:<randomizer-port>/mock/<service-id>` while preserving production settings. For
browser requests, follow the repository's existing local proxy or rewrite convention.

Keep secrets, authorization values, personal data, and production payloads out of committed mocks.
Use deterministic fictional values.

## Verify and report

Run `randomizer verify` after every modification. It compiles each contract and generates a value,
catching invalid schemas and unsupported generation features. Run focused application tests when a
reliable test seam covers the changed client behavior.

Report:

- detected language/framework and playbook used;
- services, routes, scenarios, contracts, fixtures, and local configuration changed;
- the field evidence table, especially enum, boolean, optional/nullable, and date/time decisions;
- evidence files used;
- unsupported features, fixture fallbacks, unresolved assumptions, or conflicts;
- exact commands for `randomizer start`, the application's normal startup, and `randomizer stop`.

Do not start the application or Randomizer unless requested.
