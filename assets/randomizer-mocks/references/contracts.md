# Randomizer Contract Lifecycle

Managed response contracts are Draft 2020-12 JSON Schema envelopes stored under
`.randomizer/contracts/`. Create and update them through the contract commands so the schema,
provenance, source fingerprints, and `.randomizer/contracts.lock.json` remain consistent. Contract
commands commit the artifact and lock as one recoverable transaction. If a command is interrupted,
leave its gitignored transaction journal in place so the next contract command can restore the
previous consistent state.

A managed contract contains:

```json
{
  "name": "get-user-200",
  "version": "1",
  "source": "specs/users.schema.json",
  "schema": {
    "$schema": "https://json-schema.org/draft/2020-12/schema",
    "type": "object"
  },
  "content_hash": "<canonical-schema-sha256>"
}
```

Legacy bare JSON Schema contract files remain supported at runtime. Import or analyze new contracts
to get explicit identity, version, source, and content integrity.

## Import authoritative schemas

Import a standalone schema. Its root must explicitly declare
`"$schema": "https://json-schema.org/draft/2020-12/schema"`; Randomizer will not guess the dialect:

```sh
randomizer contract import get-user-200 \
  --source specs/users.schema.json \
  --format json-schema \
  --method GET --endpoint /users/{user_id} --status 200
```

Select an exact OpenAPI 3.1 operation and response. OpenAPI 3.0 is rejected because its schema
semantics cannot be losslessly treated as Draft 2020-12. The default OpenAPI 3.1 base dialect and
an explicit Draft 2020-12 `jsonSchemaDialect` are supported; custom dialects are rejected:

```sh
randomizer contract import get-user-200 \
  --source specs/users.openapi.yaml \
  --format openapi \
  --method GET --endpoint /users/{user_id} --status 200
```

Do not select an operation by a nearby name or use a request schema for a response. The method,
normalized path, status, and response media type must identify one response unambiguously. Add
`--media-type application/json` when more than one schema-bearing response type exists.

Import a sanitized serialized example conservatively:

```sh
randomizer contract import get-user-200 \
  --source .randomizer/fixtures/get-user-200.json \
  --format serialized-example \
  --method GET --endpoint /users/{user_id} --status 200
```

This importer preserves the example and infers observed primitive/container shapes plus
unambiguous date, date-time, and UUID formats. Its warning diagnostics identify enum, nullability,
optionality, and other claims that one example cannot prove. When those unknowns make variable
generation unsafe, an exact fixture is a static fallback, not randomized output. Use it for an
explicitly dynamic request only after explicit user agreement to that downgrade, and report the
dynamic goal as unmet.

## Derive an auditable source schema

If dynamic output is required and no complete schema/provider exists, build the broadest safe Draft
2020-12 schema at `.randomizer/sources/<contract-name>.schema.json`, then import that file as a JSON
Schema managed contract. This is preferable to a fixture when repository evidence establishes the
wire shape and consumer-safe domains. The derived file is the reviewable source; never edit the
managed artifact to bypass import provenance.

```sh
randomizer contract import <contract-name> \
  --source .randomizer/sources/<contract-name>.schema.json \
  --format json-schema \
  --method <METHOD> --endpoint <path> --status <status>
```

Corroborate its claims across the relevant repository layers:

| Schema decision | Repository evidence |
| --- | --- |
| Wrapper, JSON names, observed types/format syntax | Sanitized serialized responses, fixtures, or response assertions |
| Aliases, omission/null, wire types, exact enums | Active serializer configuration and serialized type definitions |
| Discriminators accepted by the application | Consumer branches plus configuration values reachable by the mocked call |
| Upstream field set | Raw HTTP-boundary payload/decoder behavior before local enrichment or DTO mutation |
| Requiredness and constraints | Specification, serializer guarantees, mandatory consumer access/fallback behavior, validation metadata, or authoritative tests |

A consumer/configuration join is part of the wire contract. For example, when Garage reads
response `task_slug`, compares it with configured slug values, and derives application `taskType`,
trace that flow and include only corroborated values reachable from the selected local
profile/settings used by the wiring and application test in the response `task_slug` enum. Do not
put those values in a `taskType` enum. If the consumer accepts any plain string and no narrower
domain is evidenced, keep `type: string`; do not invent an enum, example, pattern, or plausible
business value.

Do not confuse an upstream response contract with the shape of a DTO after local enrichment. A
field synthesized or overwritten after the client call is not an upstream wire field merely because
the same class/type declares it. For Garage, constrain upstream `task_slug` so the downstream code
can derive `taskType`/`task_type`; omit locally derived `task_type` from the mock schema unless a raw
HTTP-boundary payload independently proves the service returns it.

Include wrappers and fields the consumer must receive for the requested flow in `required` when
their presence is corroborated by samples plus serializer behavior, mandatory decoder access, or
focused tests. Otherwise valid generation may omit them and trigger unrelated null/fallback
behavior. Do not mark unrelated fields required merely to make samples uniform.

Unknown plain-string domains may stay broad strings; never invent an enum.

A sanitized value that unambiguously matches RFC 3339 may authorize `format: date-time` in the
working mock, consistent with the serialized-example importer. That observation does not establish
the service's time-zone policy, allowed ranges, ordering, precision across all responses, or
cross-field temporal relationships; add those constraints only with serializer, specification, or
authoritative-test evidence. A merely timestamp-looking value that does not match the format remains
a plain string.

Put project-relative source paths and precise JSON pointers, symbols, serializer settings, tests,
or consumer branches in root/property `$comment` annotations for every nontrivial derived decision.
The imported source fingerprint then protects the reviewed schema and its evidence citations. Treat
conflicts or a missing consumer-required discriminator domain as blockers, but keep unconstrained
properties broad when that is semantically accepted.

## Analyze through an external provider

Use a provider when authoritative wire metadata lives behind framework or language tooling:

```sh
randomizer contract analyze get-user-200 \
  --provider ./tools/randomizer-contract-provider \
  --provider-arg --project --provider-arg app \
  --method GET --endpoint /users/{user_id} --status 200 \
  --root-symbol UserEnvelope \
  --source src/client/user-types.ts
```

The provider must implement protocol version `1` described in
[generic-wire-contract.md](generic-wire-contract.md). It is an optional repository or developer
tool, not a bundled language adapter. Reject provider output when the protocol version is
unsupported, the schema is invalid, declared sources lack SHA-256 fingerprints, diagnostics contain
errors, or the response cannot compile against Randomizer's supported schema subset.
Each core schema claim must use the canonical protocol evidence kind at its exact schema path;
provider-specific generic evidence is additional context only and cannot satisfy the claim.
The executable path and every `--provider-arg` value are stored verbatim in the contract lock, so
arguments must never contain secrets or tokens.

## Refresh and check

```sh
randomizer contract refresh get-user-200
randomizer contract check get-user-200
randomizer contract check
```

`refresh` reruns the locked import/provider provenance for one contract. `check` compares source
fingerprints, managed contract content, and lock metadata without changing files. `refresh`
preserves the locked contract version. Re-import/analyze defaults to version `1`, so read and pass
the existing locked `contract_version` explicitly unless changing it intentionally.

## Evidence requirements

Map the service's serialized wire contract, not source-language type names:

| Decision | Required evidence |
| --- | --- |
| JSON property or wrapper name | Schema name, serializer alias, provider evidence, or serialized payload |
| Enum/const | Complete exact JSON values from a schema, serializer, or authoritative tests |
| Observed date/time format | Exact JSON primitive with unambiguous syntax, schema, serializer, or authoritative test |
| Time-zone/range/temporal policy | Serializer configuration, specification, or authoritative tests |
| Required | Specification or serialization behavior guaranteeing presence |
| Nullable | Specification or serialization behavior allowing explicit JSON `null` |
| Optional | Specification or serialization behavior allowing omission |
| Collection/object shape | Item/property schema and wrapper evidence |
| Numeric/string bounds | Contract or validation evidence, not realism guesses |

Keep optional and nullable separate. A sample containing one enum value does not prove the enum set;
an unambiguous RFC 3339 sample may prove observed `format: date-time` syntax but not time-zone policy,
ranges, or temporal relationships; a declared non-null source type does not prove wire presence.

Never introduce guessed business-domain values or constraints. Enum members, consts,
discriminators, status values, examples/defaults, identifiers, names, currencies, locales,
date-time semantics, regexes, ranges, and wrappers must come from fingerprinted authoritative
evidence. Primitive type inference is not permission to author a plausible-looking domain.

When only a serialized example is authoritative, retain its warning diagnostics and do not promote
observed values into enum members, requiredness rules, bounds, or unobserved wrapper variants. Use a
sanitized deterministic fixture when the request is static. For an explicitly dynamic request,
report the evidence blocker or obtain explicit agreement to a static fallback; never silently
substitute the fixture.

## Randomized-response acceptance

An explicit randomized/dynamic request is complete only when every requested response shape and
status has a managed contract envelope, a matching `.randomizer/contracts.lock.json` entry, and a
route `body.contract` reference using a generating mode (normally `valid`). Bare schemas,
`body.inline`, `body.fixture`, and `mode: example` do not establish dynamic behavior.

Enforce that invariant for each dynamic route with
`randomizer verify --require-managed-contract-route <route-id>`; repeat the option when several
routes are in scope. Ordinary zero-count verification is not sufficient evidence.

After contract, route, and wiring checks pass, collect at least three successful runtime samples per
dynamic response. Validate the locked status and media type for every sample and exercise the
application decoder when available. Across no more than ten samples, require
at least two distinct valid values for each field the user named as variable and for at least one
non-bound, non-constant generated field. A contract containing only constants can be correct but
cannot satisfy a randomization request; report that limitation. The final handoff must name the
managed contract, source/provider and lock entry, sample count, validation command, fields tested,
and observed variation without inventing domain values.

## Supported schema subset

Supported generation features include:

- types `object`, `array`, `string`, `integer`, `number`, `boolean`, and `null`;
- nullable type arrays, `enum`, `const`, `oneOf`, and `anyOf`;
- local `$defs` and `$ref` only;
- `properties`, `required`, and `additionalProperties` validation;
- `items`, `minItems`, and `maxItems` up to 100 generated elements;
- `minLength`, `maxLength`, supported `pattern`, `example`, and `examples`;
- `minimum`, `maximum`, exclusive bounds, and `multipleOf`;
- generated formats `date`, `date-time`, `email`, `uri`, `uri-reference`, and `uuid`.

External references, cycles, `allOf`, unsupported formats such as `time` or `duration`, and
unbounded dynamic maps are not safe generated contracts. An exact fixture is available for a static
response, but switching to it requires explicit approval when randomization was requested and must
be reported as an unmet dynamic goal.

Route responses reference the emitted contract path and normally use `mode: valid`. Use separate
contracts for materially different response statuses or shapes. `randomizer verify` requires each
managed artifact to be referenced by its locked method, path, status, and media type; set an explicit
response `content-type` for a selected non-`application/json` OpenAPI response. Contract modes are
`valid`, `minimum`, `maximum`, `boundary`, `invalid`, and `example`.
