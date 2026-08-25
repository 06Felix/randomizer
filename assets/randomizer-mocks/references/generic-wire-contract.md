# Language-Neutral Wire Contracts

Randomizer consumes serialized HTTP evidence. Its core does not need to parse Java, TypeScript,
Python, Go, Rust, or any other application language.

## Source selection

For the requested method, path, status, and media type, select one authoritative input:

1. committed JSON Schema explicitly declaring Draft 2020-12;
2. committed OpenAPI 3.1 response schema using its default base or Draft 2020-12 dialect;
3. a versioned external provider;
4. for dynamic output, a Draft 2020-12 source schema under `.randomizer/sources/` derived from
   corroborated serialized, serializer/type, and consumer/configuration evidence;
5. a sanitized serialized example imported conservatively or, for a static response, used as an
   exact fixture.

Trace the outbound client only far enough to resolve the endpoint, response root/wrapper, source
artifact, and local configuration setting. Consumer access proves application dependency but does
not define the service's complete response.

## Provider protocol version 1

`randomizer contract analyze` launches the selected executable, sends one JSON request on standard
input, and expects one JSON response on standard output. Provider logs belong on standard error.
Randomizer enforces execution, timeout, exit-status, protocol, schema, diagnostic, and fingerprint
checks before writing a contract. The executable path and arguments are retained in
`.randomizer/contracts.lock.json` so `refresh` can reproduce the invocation; never pass secrets or
tokens as provider arguments.

The request contains:

- `protocol_version: "1"`;
- `endpoint`: method, path, status, and optional media type;
- optional `root_symbol` naming the response/wrapper root;
- `source_paths`, limited to the explicitly selected repository sources.

The response contains:

- `protocol_version: "1"`;
- `provider`: stable name and provider version;
- the exact `endpoint` selector echoed from the request;
- `schema`: a Draft 2020-12 response schema;
- optional resolved `root_symbol`;
- SHA-256 `source_fingerprints` for every source used;
- claim-level `evidence` for wrappers, property names, types, enum/const values, formats,
  requiredness, nullability, and constraints;
- structured `diagnostics`, including errors for unresolved or conflicting decisions.

```json
{
  "protocol_version": "1",
  "endpoint": {
    "method": "GET",
    "path": "/users/{user_id}",
    "status": 200,
    "media_type": "application/json"
  },
  "root_symbol": "UserEnvelope",
  "source_paths": ["src/client/user-types.ts"]
}
```

```json
{
  "protocol_version": "1",
  "provider": { "name": "team.typescript-wire", "version": "2.1.0" },
  "endpoint": {
    "method": "GET",
    "path": "/users/{user_id}",
    "status": 200,
    "media_type": "application/json"
  },
  "root_symbol": "UserEnvelope",
  "schema": {
    "$schema": "https://json-schema.org/draft/2020-12/schema",
    "type": "object",
    "required": ["status"],
    "properties": {
      "status": { "type": "string", "enum": ["ACTIVE", "SUSPENDED"] }
    }
  },
  "source_fingerprints": [
    {
      "path": "src/client/user-types.ts",
      "algorithm": "sha256",
      "digest": "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
    }
  ],
  "evidence": [
    { "schema_path": "#", "source_path": "src/client/user-types.ts", "source_location": "UserEnvelope", "kind": "response_wrapper" },
    { "schema_path": "#", "source_path": "src/client/user-types.ts", "source_location": "UserEnvelope", "kind": "root_symbol" },
    { "schema_path": "#/type", "source_path": "src/client/user-types.ts", "source_location": "UserEnvelope", "kind": "type" },
    { "schema_path": "#/required/0", "source_path": "src/client/user-types.ts", "source_location": "UserEnvelope.status", "kind": "requiredness" },
    { "schema_path": "#/properties/status", "source_path": "src/client/user-types.ts", "source_location": "UserEnvelope.status", "kind": "property_name" },
    { "schema_path": "#/properties/status/type", "source_path": "src/client/user-types.ts", "source_location": "UserEnvelope.status", "kind": "type" },
    { "schema_path": "#/properties/status/enum", "source_path": "src/client/user-types.ts", "source_location": "UserStatus", "kind": "enum" }
  ],
  "diagnostics": []
}
```

Each core schema claim must have an entry at that exact `schema_path` with its canonical `kind`:
`response_wrapper`, `root_symbol`, `property_name`, `type`, `enum`, `const`, `format`,
`requiredness`, `nullable`, or `constraint`. `response_wrapper` always targets `#`; `root_symbol` is
required when the response includes one. Required properties target their individual `required`
array entries; optional properties target their property schema with `requiredness`; and nullability
targets the exact null member. Provider-specific kinds may add useful context, but they do not
replace any required core claim.

Do not accept an adapter that changes the endpoint selector, silently defaults ambiguous
enum/date-time/wrapper behavior, analyzes sources outside the declared scope, omits source
fingerprints, or prints non-protocol content to standard output.

Provider output must not add plausible business values that its fingerprinted sources do not
establish. This includes enum/const members, discriminator/status values, examples/defaults,
identifiers, names, currencies, locales, dates, constraints, and wrappers. Report unresolved domain
semantics as diagnostics instead of inventing them.

The same rule applies to a manually derived source schema. Use samples for observed wire shape,
active serializer/type metadata for wire types and exact enum serialization, and consumer branches
plus configuration for accepted discriminator values. Cite those project-relative sources in
schema `$comment` annotations. Unknown plain-string domains may remain broad strings; they must not
be converted into guessed enums.

A sanitized sample that unambiguously matches RFC 3339 may authorize `format: date-time` for the
working mock, consistent with the built-in importer. It does not prove time-zone policy, allowed
ranges, ordering, cross-field temporal relationships, or every response representation; those need
serializer, specification, or authoritative-test corroboration. Scope consumer/config discriminator
values to the selected local profile/settings used by the wiring and test.

Define the contract at the upstream HTTP boundary. Exclude fields synthesized or overwritten by
local post-processing after the client returns, even if a shared DTO/type contains them. Constrain
the evidenced upstream discriminator so downstream enrichment succeeds, and require corroborated
fields that the requested consumer path must receive to avoid null/fallback behavior.

## Serialized examples

An example is strong evidence for the exact observed JSON names, primitive/container types,
wrappers, and values. Alone, it does not establish:

- the complete set of enum values;
- whether a boolean is variable or fixed;
- whether a property is always present, optional, or nullable;
- time-zone policy, allowed ranges, ordering, or cross-field relationships beyond an observed
  unambiguous date/time syntax;
- unobserved collection bounds, variants, or error shapes.

The built-in serialized-example importer preserves the example, infers observed
primitive/container shapes and unambiguous date, date-time, or UUID formats, and emits warning
diagnostics for claims the sample cannot prove. A fixture is a static fallback when those unknowns
matter. If randomization was explicitly requested, use that fallback only after explicit user
agreement and report that the dynamic goal remains unmet. An external provider may add only
constraints backed by additional fingerprinted evidence.

## Optional language adapters

Randomizer does not ship language analyzers. A team may supply a provider executable built on its
native compiler or framework tooling. When evaluating one, load only the matching file under
`languages/` and verify that the adapter reports actual serializer behavior rather than type-name
heuristics. Unknown languages use the same protocol and evidence rules without a new core feature.
