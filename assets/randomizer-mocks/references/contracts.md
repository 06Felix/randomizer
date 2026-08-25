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
optionality, and other claims that one example cannot prove. Use the example as an exact fixture
when those unknowns make variable generation unsafe.

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
| Date/time | Exact JSON primitive plus format/encoding and timezone behavior |
| Required | Specification or serialization behavior guaranteeing presence |
| Nullable | Specification or serialization behavior allowing explicit JSON `null` |
| Optional | Specification or serialization behavior allowing omission |
| Collection/object shape | Item/property schema and wrapper evidence |
| Numeric/string bounds | Contract or validation evidence, not realism guesses |

Keep optional and nullable separate. A sample containing one enum value does not prove the enum set;
a timestamp-looking string does not prove `format: date-time`; a declared non-null source type does
not prove wire presence.

When only a serialized example is authoritative, retain its warning diagnostics and do not promote
observed values into enum members, requiredness rules, bounds, or unobserved wrapper variants. Use a
sanitized deterministic fixture when the conservative imported schema is still unsafe.

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
unbounded dynamic maps are not safe generated contracts. Use an exact fixture when the real wire
shape cannot be generated without losing required behavior.

Route responses reference the emitted contract path and normally use `mode: valid`. Use separate
contracts for materially different response statuses or shapes. `randomizer verify` requires each
managed artifact to be referenced by its locked method, path, status, and media type; set an explicit
response `content-type` for a selected non-`application/json` OpenAPI response. Contract modes are
`valid`, `minimum`, `maximum`, `boundary`, `invalid`, and `example`.
