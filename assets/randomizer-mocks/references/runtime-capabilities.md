# Randomizer Runtime Capabilities

Use this reference before choosing a contract keyword or format. The project gateway receives normal
HTTP requests at `/mock/<service-id>/<path>`; it does not proxy to the real service and it does not
receive source-language DTOs.

## Request mapping

The gateway extracts and matches:

- HTTP method;
- service ID from the first `/mock/` path segment;
- literal paths, `{name}` path parameters, and `*` wildcards;
- exact configured query values;
- exact configured header values with case-insensitive header names;
- exact configured JSON-body values addressed by JSON Pointer.

The request body must be valid JSON when present. Non-JSON bodies are rejected. Extra query values,
headers, and JSON properties are allowed unless a route matcher explicitly checks them.

## Response sources

Each response uses one body source:

- `inline`: fixed JSON in the manifest;
- `fixture`: JSON file inside the project;
- `contract`: bare Draft 2020-12 JSON Schema generated at runtime;
- no body: status and headers only.

Responses may set status, headers, delay, and request bindings. Multiple responses advance in order and
then hold the final response. `randomizer reset` clears response counters and request history.

## Contract generation support

Supported schema types:

- `object`, `array`, `string`, `integer`, `number`, `boolean`, `null`;
- nullable type arrays containing `null`;
- `enum`, `const`, `oneOf`, `anyOf`;
- local `$defs` and `$ref` only;
- object `properties`, `required`, and `additionalProperties` validation;
- arrays with `items`, `minItems`, and `maxItems` up to 100;
- strings with `minLength`, `maxLength`, supported `pattern`, `example`, and `examples`;
- numeric `minimum`, `maximum`, exclusive bounds, and `multipleOf`;
- generated formats `date`, `date-time`, `email`, `uri`, `uri-reference`, and `uuid`.

Generation modes are `valid`, `minimum`, `maximum`, `boundary`, `invalid`, and `example`.

## Custom generation requests

The standalone `/generate` API also accepts Randomizer's custom schema language. Use it when the
developer asks for a generator request payload, not when authoring a project response contract:

```json
{
  "schema": {
    "completion_time_in_minutes": { "type": "int", "min": 0, "max": 60 }
  },
  "seed": 12345,
  "sequence": 0
}
```

Custom field generators are `int`, `float`, `string`, `enum`, `object`, `boolean`, `uuid`, and
`list`. Variable names can suggest units or semantic types, but explicit bounds and repository/domain
evidence take precedence. Do not invent a range solely from a name.

Unsupported or unsafe choices include external references, cyclic references, `allOf`, unsupported
formats such as `time` or `duration`, and constraints the generator cannot satisfy. Use a fixture or
ask for a serialized example instead of fabricating a schema.

## Bindings

Bindings run after inline, fixture, or contract body creation:

```text
${request.path.<name>}
${request.query.<name>}
${request.header.<name>}
${request.body./json/pointer}
```

Path, query, and header bindings are strings. Contract responses are validated again after bindings,
so only bind them into schema locations that accept the bound JSON type.
