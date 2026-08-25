---
name: randomizer-mocks
description: Configure repository-local Randomizer HTTP mocks for named outbound endpoints using authoritative wire schemas, versioned contract providers, serialized fixtures, and deterministic local URL wiring. Use when asked to add, update, or resynchronize an HTTP mock without depending on the application's implementation language.
---

# Configure Randomizer Mocks

Treat a mock as complete only when its response contract is evidence-backed and the application's
local HTTP setting is wired to the Randomizer gateway. Apply the same workflow to existing and new
services. Keep changes limited to the requested outbound endpoints.

## Establish scope

Accept an exact method and path, a service/client symbol, an existing route ID, or a short endpoint
list. Resolve that input to the outbound HTTP method, path template, success/error status being
mocked, owning service, and local configuration setting. If the endpoint or status remains
ambiguous, ask before changing files.

Before initializing or modifying the project, confirm that the invoked binary exposes this
workflow:

```sh
randomizer contract --help
randomizer wiring --help
```

If either command is unavailable, stop and ask the developer to update or build Randomizer. Do not
trust the version string alone; an older binary may report the same prerelease package version.

If `.randomizer/randomizer.yaml` is absent, run `randomizer init`. Otherwise inspect the existing
manifest, contracts, fixtures, contract lock, and application configuration before changing them.

Read these references as needed:

- Always read [references/runtime-capabilities.md](references/runtime-capabilities.md) and
  [references/generic-wire-contract.md](references/generic-wire-contract.md).
- Before importing or changing a generated response, read
  [references/contracts.md](references/contracts.md).
- Read one matching optional-provider guide only when evaluating a repository-supplied or
  developer-supplied adapter: [Java/JVM](references/languages/java.md),
  [TypeScript/JavaScript](references/languages/typescript.md),
  [Python](references/languages/python.md), [Go](references/languages/go.md), or
  [Rust](references/languages/rust.md). Randomizer does not bundle those adapters.

## Choose authoritative response evidence

Prefer inputs in this order:

1. A committed response JSON Schema that explicitly declares Draft 2020-12.
2. The exact operation, status, and media-type response schema in committed OpenAPI 3.1 using its
   default base dialect or explicit Draft 2020-12 dialect.
3. A repository-supplied or developer-supplied executable provider that implements Randomizer's
   versioned provider protocol.
4. A sanitized serialized example imported conservatively, or used as an exact fixture when its
   limits prevent safe randomization.

Do not infer the wire contract from a source-language type name alone. A serialized example proves
the observed shape and values, but not a complete enum, a date-time semantic, requiredness across all
responses, or all wrapper variants.

Before accepting a generated contract, account for every field's JSON name, wire type,
requiredness, nullability, enum/const values, constraints, wrapper location, and evidence. Exact enum
values, date/time representation, aliases, custom serialization, and response wrappers require
explicit evidence. Treat provider error diagnostics, missing fingerprints, unsupported protocol
versions, and conflicting sources as blocking rather than guessing.

## Materialize the contract deterministically

Use Randomizer commands rather than hand-copying provider output:

```sh
randomizer contract import get-user-200 \
  --source specs/users.schema.json \
  --format json-schema \
  --method GET --endpoint /users/{user_id} --status 200

randomizer contract import get-user-200 \
  --source specs/users.openapi.yaml \
  --format openapi \
  --method GET --endpoint /users/{user_id} --status 200

randomizer contract import get-user-200 \
  --source .randomizer/fixtures/get-user-200.json \
  --format serialized-example \
  --method GET --endpoint /users/{user_id} --status 200

randomizer contract analyze get-user-200 \
  --provider ./tools/randomizer-contract-provider \
  --method GET --endpoint /users/{user_id} --status 200 \
  --root-symbol UserEnvelope --source src/client/user-types.ts
```

Pass `--root-symbol <symbol>` when the evidenced response/wrapper root must be recorded explicitly,
and `--media-type <type>` when an operation has multiple schema-bearing response types. Pass
`--provider-arg <arg>` repeatedly only when the provider requires fixed arguments. Use
`--contract-version <version>` for a deliberately versioned new contract. Before re-importing or
reanalyzing an existing contract, read its locked `contract_version` and pass that value explicitly;
the command default is always `1`. `import` and `analyze` write the managed contract and update
`.randomizer/contracts.lock.json` with provenance and source fingerprints as one recoverable
transaction. Leave any transaction journal in place after an interrupted command; the next contract
command restores the previous consistent state automatically.

Provider program names and arguments are persisted verbatim in that lock; never put secrets,
tokens, or personal data in them.

Do not import a standalone schema with an undeclared dialect, OpenAPI 3.0, or an OpenAPI 3.1 document
with a custom `jsonSchemaDialect`; obtain a supported authoritative artifact instead of assuming
keyword semantics.

The serialized-example importer infers only observed primitive/container shapes and unambiguous
date, date-time, or UUID formats, preserves the example, and reports the claims one sample cannot
prove. If those warnings leave required contract decisions unresolved, use the sanitized example as
an exact fixture instead of fabricating variability.

Use `randomizer contract refresh <name>` after a locked source changes, and
`randomizer contract check [<name>]` to reject stale or manually drifted contracts.

## Reconcile routes and local wiring

Identify routes by service, method, normalized path, and distinguishing matchers. Reuse stable
service and route IDs, preserve unrelated scenarios, and never delete unreferenced fixtures
automatically. Repeated execution must not create duplicate entries. Reference every managed
contract from a route with the same locked method, path, response status, and media type. When an
OpenAPI response selected a non-`application/json` type, set the response `content-type` header to
that exact media type.

For every requested service, including an existing service, add or reconcile structured `wiring`
entries that name the exact local configuration file, supported format, selector, and target. Use
`service_base_url` only with `service_base_safety: dedicated_setting` when the setting is exclusive
to modeled calls, or `service_base_safety: all_calls_mocked` when every call sharing it has a route.
Before selecting either service-base assertion, inspect or test the actual HTTP client's base-URL
resolution with the endpoint spelling used by the application. Set
`service_base_path_behavior: preserves_prefix` only when it retains the configured
`/mock/<service-id>` path, including for a leading-slash endpoint. Many standard URL resolvers drop
that prefix. Use `route_url` only when the application has a route-specific setting and the route
matcher path is static; omit both service-base assertions for that target. A route containing
`{parameter}` or `*` cannot become a literal configuration URL, so use an evidenced base setting or
ask the developer instead. If adding wiring to a legacy version 1 manifest, update it to version 2.
Never redirect production configuration.

Wiring files are application configuration. Never point a wiring entry into `.randomizer/` or at a
manifest, contract, lock, fixture, skill, or runtime-state file.

Keep secrets, authorization values, personal data, and unsanitized production payloads out of the
manifest, contracts, fixtures, provider diagnostics, and committed local configuration.

Run:

```sh
randomizer wiring apply --service <service-id>
randomizer wiring check --service <service-id>
```

The gateway does not proxy unmatched traffic. Before applying `service_base_url`, find every call
sharing that base setting and ensure its routes are modeled; otherwise prefer an evidenced
route-specific setting or ask the developer how local routing should work.

## Verify and report

Run `randomizer contract check`, `randomizer wiring check`, and `randomizer verify`. Inspect the
reported counts and manifest after checking: zero managed contracts, routes, or wiring entries can
be valid for an empty/legacy project, but does not prove that the requested mock is complete. Run
focused application tests when they cover the changed HTTP client or decoder.

Report the endpoint/status handled, contract source or provider identity, material enum/date-time/
wrapper evidence, route and wiring entries changed, fixture fallbacks, unresolved ambiguity, and the
exact `randomizer start`, application start, and `randomizer stop` commands. Do not start either
process unless requested.
