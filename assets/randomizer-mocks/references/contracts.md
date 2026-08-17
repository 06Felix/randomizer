# Randomizer Contract Authoring

Create contracts as bare JSON Schema Draft 2020-12 documents. Store them under
`.randomizer/contracts/` and reference them from `randomizer.yaml`. Randomizer supplies the contract
name, source, version, and content hash at verification and runtime.

## Contract example

```json
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "type": "object",
  "required": ["id", "status", "active", "retryable", "roles"],
  "properties": {
    "id": { "type": "string", "format": "uuid" },
    "status": { "type": "string", "enum": ["ACTIVE", "SUSPENDED"] },
    "active": { "type": "boolean" },
    "retryable": { "const": false },
    "display_name": { "type": ["string", "null"], "minLength": 1 },
    "roles": {
      "type": "array",
      "minItems": 1,
      "maxItems": 5,
      "items": { "$ref": "#/$defs/role" }
    }
  },
  "$defs": {
    "role": { "type": "string", "enum": ["OWNER", "MEMBER", "VIEWER"] }
  },
  "additionalProperties": false
}
```

```yaml
body:
  contract: .randomizer/contracts/users-get-user-200.json
  mode: valid
```

## Evidence mapping

Map the service's wire contract, not merely language-level type names:

| Evidence | JSON Schema |
| --- | --- |
| Serialized enum values | `enum` containing exact JSON values |
| Boolean may be true or false | `type: boolean` |
| Boolean is contractually fixed | `const: true` or `const: false` |
| Property guaranteed present | Include its JSON name in `required` |
| Property may be absent | Omit it from `required` |
| Property may be JSON null | Include `null` in its type or union branch |
| Repeated list | `type: array` with `items` |
| Nested DTO or object | `type: object` with `properties` |
| Closed response object | `additionalProperties: false` only with authoritative evidence |
| Fixed discriminator or event kind | `const` |
| Alternative wire shapes | `oneOf` or `anyOf` |

Resolve annotations, custom serializers, aliases, naming policies, and enum conversion before using
source-language field or symbol names. Primitive language types do not prove JSON requiredness.
Consumer dereferences show what the application expects but do not override an authoritative service
specification.

When an authoritative service schema or serialized response type is available, model its complete
wire response. When only consumer usage is available, model the evidenced subset, leave the object
open, and report that the contract is partial; do not set `additionalProperties: false` or claim the
remaining response fields are known.

## Supported generation features

Use these features when supported by evidence:

- types: `object`, `array`, `string`, `integer`, `number`, `boolean`, and `null`;
- composition: local `$defs`/`$ref`, `oneOf`, and `anyOf`;
- exact choices: `enum` and `const`, including string, number, boolean, and null values;
- objects: `properties`, `required`, and validation of `additionalProperties`;
- arrays: `items`, `minItems`, and `maxItems` up to 100 generated elements;
- strings: `minLength`, `maxLength`, generatable `pattern`, and `examples`/`example`;
- formats: `date`, `date-time`, `email`, `uri`, `uri-reference`, and `uuid`;
- numbers: `minimum`, `maximum`, `exclusiveMinimum`, `exclusiveMaximum`, and `multipleOf`;
- nullable values: type arrays containing `null`;
- response modes: `valid`, `minimum`, `maximum`, `boundary`, `invalid`, and `example`.

Avoid external `$ref`, cyclic references, `allOf`, unsupported string formats, unbounded dynamic maps,
and relying on `default` for generation. Use an inline response or fixture when the actual shape
cannot be represented by supported generation features.

## Contract decisions

- Default route responses to `mode: valid`.
- Use `mode: example` only when the route should reproduce the contract's example rather than vary.
- Use separate contracts when success and error statuses have different response shapes.
- Do not add speculative enum members so responses appear more varied.
- Do not constrain a boolean to one value just because one fixture contains that value.
- Do not mark every declared field required unless presence is guaranteed by serialization or the
  authoritative service specification.
- Do not add numeric, length, array, or pattern bounds solely to make generated output look realistic.
- Keep the schema valid after request bindings. Path, query, and header bindings produce strings, so
  bind them only into schema locations that accept strings.
