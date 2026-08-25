---
name: randomizer-mocks
description: Configure repository-local Randomizer HTTP mocks, including evidence-backed randomized or dynamic responses, for named outbound endpoints using authoritative wire schemas, versioned contract providers, serialized fixtures, and deterministic local URL wiring. Use when asked to add, update, or resynchronize an HTTP mock without depending on the application's implementation language.
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

Record whether the requested response is randomized/dynamic/generated/variable or intentionally
static/example-based. When the request explicitly asks for randomized or dynamic responses, treat
that as an acceptance criterion: an `inline` body or fixture is not an equivalent implementation.

Before initializing or modifying the project, confirm that the invoked binary exposes this
workflow:

```sh
randomizer contract --help
randomizer wiring --help
randomizer verify --help
```

If either contract or wiring command is unavailable, or verify help does not expose
`--require-managed-contract-route`, stop before changing files and ask the developer to update or
build Randomizer. That capability gate identifies the workflow build that also supports typed
binding coercion. Do not trust the version string alone; an older binary may report the same
prerelease package version.

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
4. For dynamic output when no complete artifact/provider exists, an auditable Draft 2020-12 source
   schema derived under `.randomizer/sources/` from corroborated repository evidence.
5. A sanitized serialized example imported conservatively, or used as an exact fixture only for a
   static response or an explicitly approved static fallback.

Do not infer the wire contract from a source-language type name alone. A serialized example proves
the observed shape and values, but not a complete enum, service-wide time-zone/range policy,
cross-field temporal relationships, requiredness across all responses, or all wrapper variants.

Before accepting a generated contract, account for every field's JSON name, wire type,
requiredness, nullability, enum/const values, constraints, wrapper location, and evidence. Exact enum
values, date/time representation, aliases, custom serialization, and response wrappers require
explicit evidence. Treat provider error diagnostics, missing fingerprints, unsupported protocol
versions, and conflicting sources as blocking rather than guessing.

Never invent business values or business constraints to make output look realistic. Do not add
guessed enum/const members, discriminator or status values, examples, defaults, identifiers, names,
currencies, locales, timestamps, regexes, ranges, or wrapper fields to a schema or provider result.
Encode them only when the selected authoritative source evidences them. If valid randomization
depends on missing domain semantics, trace the repository evidence described below. An unknown
plain-string domain that the consumer accepts broadly may remain `{ "type": "string" }`; never
turn it into an invented enum.

## Derive a source schema when dynamic output needs one

When the user requests dynamic output but the repository has no complete JSON Schema, OpenAPI
response, or executable provider, prefer a broad evidence-backed managed schema over a fixture if
the wire shape can be corroborated safely. Create a committed
`.randomizer/sources/<contract-name>.schema.json`, explicitly declare Draft 2020-12, then import that
file with `--format json-schema`. Do not hand-edit the emitted managed contract.

```sh
randomizer contract import <contract-name> \
  --source .randomizer/sources/<contract-name>.schema.json \
  --format json-schema \
  --method <METHOD> --endpoint <path> --status <status>
```

Build the source schema claim by claim:

- Use sanitized serialized examples or response assertions for observed wrapper placement, JSON
  property names, primitive/container wire types, and recognizable format syntax. A sanitized value
  that unambiguously matches RFC 3339 may authorize `format: date-time` for the working mock, matching
  the serialized-example importer. It does not prove time-zone policy, allowed ranges, ordering, or
  cross-field temporal relationships; those require serializer, specification, or test evidence.
- Inspect the application's active serializer configuration and serialized type definitions for
  aliases, omission/null behavior, numeric versus string wire types, date/time encoding, and exact
  serialized enum values.
- Trace consumer decoder branches, discriminator comparisons, and the configuration values feeding
  the request. For example, if Garage compares response `task_slug` with configured slug values to
  derive application `taskType`, use the values reachable from the selected local profile/settings
  that the wiring and application test actually use as the response `task_slug` enum only when those
  paths corroborate that mapping; do not put that enum on `taskType`.
- Separate upstream wire fields from values synthesized or overwritten after the HTTP client
  returns, even when one DTO carries both. Use raw HTTP-boundary samples, not a later enriched
  object. Do not add locally derived output fields to the upstream mock contract merely because the
  shared type declares them. In Garage, constrain upstream `task_slug` so downstream enrichment can
  set `taskType`/`task_type`; omit local `task_type` from the upstream schema unless raw wire evidence
  independently shows that field.
- Put wrappers and fields the consumer must receive for the requested code path in `required` when
  samples plus serializer behavior, decoder access, or focused tests corroborate their presence.
  This prevents generated omission from driving the application into unrelated null/fallback
  behavior. Leave other requiredness, bounds, patterns, `additionalProperties`, and unknown domains
  broad unless repository evidence narrows them. A plain string may stay a plain string. Never
  promote one observed value into an enum or const.
- Resolve conflicts in favor of actual wire serialization and consumer behavior; if material
  evidence conflicts or a consumer-required discriminator domain remains unknown, report the
  blocker rather than guessing.

Add `$comment` annotations at the root and relevant property schemas naming the project-relative
evidence paths plus JSON pointers, symbols, serializer settings, tests, or consumer branches that
support each nontrivial decision. These citations make the derived source and its locked fingerprint
auditable. Do not include secrets, personal data, or unsanitized payloads in the comments.

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
prove. If those warnings leave required contract decisions unresolved, do not fabricate
variability. For an explicitly static request, an exact sanitized fixture is valid. For an explicit
randomized/dynamic request, first derive the broadest safe source schema from corroborated
serializer, type, sample, and consumer evidence. Use a fixture only when that cannot produce
semantically accepted output and after the user explicitly agrees to a static fallback; report that
the original dynamic goal was not delivered.

An explicit randomized/dynamic response must be materialized as a managed contract through
`contract import` or `contract analyze`, recorded in `.randomizer/contracts.lock.json`, and
referenced from the route through `body.contract` with an appropriate generation mode (normally
`valid`). A legacy bare schema, `body.inline`, `body.fixture`, or `mode: example` does not by itself
satisfy that request. Do not report completion while any requested dynamic route lacks its managed
contract and matching lock entry.

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

Request-derived bindings preserve strings by default. When a numeric path parameter is bound into
an integer response property, use explicit coercion so post-binding contract validation sees the
evidenced wire type:

```yaml
bindings:
  - target: /taskId
    source: ${request.path.task_id}
    coerce: integer
```

Use `coerce: integer` only when the target contract type is integer and the application path value
is expected to parse as a JSON integer; invalid or out-of-range values must fail rather than being
silently rewritten.

Run:

```sh
randomizer wiring apply --service <service-id>
randomizer wiring check --service <service-id>
```

The gateway does not proxy unmatched traffic. Before applying `service_base_url`, find every call
sharing that base setting and ensure its routes are modeled; otherwise prefer an evidenced
route-specific setting or ask the developer how local routing should work.

## Verify and report

Run `randomizer contract check`, `randomizer wiring check`, and `randomizer verify`. For each route
requested as dynamic, also run:

```sh
randomizer verify --require-managed-contract-route <route-id>
```

Repeat the option for multiple dynamic routes. Inspect the reported counts and manifest after
checking: zero managed contracts, routes, or wiring entries can be valid for an empty/legacy
project, but does not prove that the requested mock is complete. Run focused application tests when
they cover the changed HTTP client or decoder.

For every explicitly randomized/dynamic response, exercise at least three contract-backed runtime
samples using representative request inputs. Each sample must return the locked status and media
type, pass Randomizer's post-binding contract validation, and pass the application's decoder or
focused consumer test when that seam exists. Inspect the fields the user expects to vary and at
least one non-bound, non-constant generated field. Observe at least two distinct valid values within
at most ten samples; if the contract permits no variation or variation is not observed, report that
randomized behavior was not demonstrated instead of declaring success. Start Randomizer temporarily
when needed for this check, reuse but do not stop an instance you did not start, and stop an instance
you started after collecting the samples. Do not start the application unless requested.

Report the endpoint/status handled, contract source or provider identity, material enum/date-time/
wrapper evidence, route and wiring entries changed, and unresolved ambiguity. For each dynamic
route, also report the managed contract name/path and lock entry, any derived source schema and its
evidence citations, generation mode, the exact `--require-managed-contract-route` command, exact
sample count and validation command, fields checked for variation, and distinct values or hashes
observed. State `fixture fallback: none` or identify the user's explicit static-fallback approval
and mark the dynamic goal as unmet. Include the exact `randomizer start` and `randomizer stop`
commands. Include the application-start command only when requested or actually run; otherwise
state `application not started` and `application-start command: not run/not applicable`.
