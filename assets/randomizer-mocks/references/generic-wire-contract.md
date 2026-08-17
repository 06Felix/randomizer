# Generic Wire-Contract Workflow

Use this when no language playbook applies or when source-level types do not reveal serialization.

## Evidence procedure

1. Locate the outbound HTTP client and the local base-URL setting.
2. Locate the response type and every serializer or decoder configuration.
3. Search for OpenAPI, JSON Schema, fixtures, recorded responses, and contract tests.
4. Inspect response-consumer access patterns only as secondary evidence.
5. Prefer a real serialized example when types and configuration are ambiguous.
6. Record a field evidence table before writing JSON Schema.

## Wire-first rules

- JSON property names come from serialized output, not source member names.
- A source enum constant is not a wire enum value until serialization proves it.
- A primitive or non-null type does not prove JSON requiredness.
- A nullable source type does not prove that `null` is emitted rather than omitted.
- A default value is not a response guarantee.
- A date/time class does not determine whether JSON is text, epoch number, or an object.
- A consumer's field access shows a dependency, not the complete service contract.
- If only a subset is evidenced, leave objects open and report the contract as partial.

## Safe fallback

If the wire shape cannot be represented by Randomizer's supported contract generator, choose one:

1. A fixture containing a sanitized, deterministic serialized response.
2. An inline body for an intentionally fixed response.
3. A developer question requesting an authoritative example or schema.

Never add speculative enum members, date/time formats, required fields, validation bounds, or error
states just to make a response look realistic.
