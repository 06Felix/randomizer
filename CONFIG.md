# Configuration Guide

This document describes the configuration that exists in Randomizer today.

## Server Configuration

| Environment variable | Default | Description |
| --- | --- | --- |
| `RANDOMIZER_HOST` | `0.0.0.0` | IP address on which the server listens |
| `RANDOMIZER_PORT` | `7263` | TCP port on which the server listens |
| `RANDOMIZER_MAX_CONCURRENT_WS_STREAMS` | `4096` | Maximum simultaneous WebSocket streams; must be greater than zero |
| `RUST_LOG` | `info` | Tracing filter, for example `randomizer=debug` |

Invalid configuration prevents startup and reports the offending setting.

## Standard JSON Schema Contracts

Randomizer supports the requested generation subset of JSON Schema Draft 2020-12 while preserving
the original custom schema. Submit a standard contract to `POST /generate`:

```json
{
  "contract": {
    "name": "customer-created",
    "version": "1.2.0",
    "source": "contracts/customer-created.json",
    "schema": {
      "$schema": "https://json-schema.org/draft/2020-12/schema",
      "$defs": {
        "id": { "type": "string", "format": "uuid" }
      },
      "type": "object",
      "required": ["id", "email"],
      "properties": {
        "id": { "$ref": "#/$defs/id" },
        "email": { "type": "string", "format": "email" },
        "nickname": { "type": ["string", "null"], "pattern": "^[A-Za-z]{2,12}$" }
      }
    }
  },
  "mode": "valid",
  "seed": 12345,
  "sequence": 0
}
```

Contract metadata fields `name`, `version`, and `source` are required and non-empty. Randomizer
canonicalizes the schema, returns its SHA-256 `content_hash`, and accepts an optional input
`content_hash` assertion. The normal top-level `contract_hash` replay assertion is also supported.

Supported generation behavior includes:

- required and optional object properties
- nullable `type` arrays such as `["string", "null"]`
- local JSON Pointer `$ref` values and `$defs`
- `oneOf`, `anyOf`, `const`, and `enum`
- string `pattern`, `minLength`, and `maxLength`
- number/integer bounds and `multipleOf`
- arrays with `items`, `minItems`, and `maxItems`
- `date`, `date-time`, `email`, `uri`, `uri-reference`, and `uuid` formats
- root or property `examples` and the legacy `example` annotation

External HTTP and file references are rejected to keep contracts self-contained and replayable.
Cyclic references and patterns unsupported by the deterministic regex generator return explicit
contract errors. Generated strings and arrays are capped at `100` elements for resource safety.

### Generation modes

| Mode | Behavior |
| --- | --- |
| `valid` | Deterministic valid data within the contract |
| `minimum` | Minimum values and required properties only |
| `maximum` | Maximum bounded values and all declared optional properties |
| `boundary` | Deterministically selects valid lower or upper boundaries |
| `invalid` | Intentionally violates the contract and returns the validator's exact rule |
| `example` | Uses the first `examples`/`example` value, falling back to valid generation |

An invalid result includes:

```json
{
  "violated_rule": {
    "keyword": "required",
    "schema_path": "/required",
    "instance_path": "",
    "message": "..."
  }
}
```

### Contract validation

`POST /validate` validates any value using Draft 2020-12 with format assertions enabled:

```json
{
  "contract": {
    "name": "customer-created",
    "version": "1.2.0",
    "source": "inline",
    "schema": { "type": "string", "format": "email" }
  },
  "value": "not-an-email"
}
```

The response contains `valid`, canonical contract metadata, and every violation with its keyword,
schema JSON Pointer, instance JSON Pointer, and validator message. WebSocket `/stream` accepts the
same `contract`, `mode`, and deterministic replay fields alongside `frequency`.

## Deterministic Generation and Replay

Randomizer derives each event independently from `seed + sequence`. Every response returns:

```json
{
  "value": { "age": 42 },
  "metadata": {
    "seed": 12345,
    "sequence": 0,
    "generator_version": "1",
    "contract_hash": "<sha256>"
  }
}
```

- `seed`: optional unsigned integer. A portable random seed is generated and returned when absent.
- `sequence`: optional unsigned integer, default `0`. WebSocket streams increment it per event.
- `generator_version`: optional behavior version, currently `"1"`. Unsupported versions are rejected.
- `contract_hash`: optional SHA-256 assertion. If supplied, it must match the canonical schema.

Exact replay requires the same schema, seed, sequence, and generator version. Supplying the
returned contract hash additionally protects against accidentally replaying a modified schema.

### Stable RNG contract

Generator version `1` uses the repository-owned SplitMix64 algorithm. An event initializes its
stream with wrapping unsigned `seed + sequence × 0xd1342543de82ef95`; the odd sequence multiplier
assigns a distinct initial 64-bit state to every sequence for a fixed seed. Bounded integer selection
uses rejection sampling. This algorithm does not depend on `rand`'s internal generators. Schemas are
parsed into the typed model, recursively sorted by object key, serialized without insignificant
whitespace, and SHA-256 hashed. Any change to these rules or generation semantics requires a new
generator version. SplitMix64 is deterministic rather than cryptographically secure; generated
values must not be used as passwords, tokens, keys, or other security-sensitive material.

## REST Request Configuration

The REST API accepts an envelope containing `schema` and optional replay fields. The original raw
schema body remains accepted, but the envelope is required to supply replay inputs.

Example:

```json
{
  "schema": {
    "type": "object",
    "properties": {
      "age": {
        "type": "int",
        "min": 18,
        "max": 65
      },
      "score": {
        "type": "float",
        "min": 0.5,
        "max": 9.5,
        "precision": 2
      }
    }
  },
  "seed": 12345,
  "sequence": 0
}
```

## WebSocket Request Configuration

The WebSocket API expects a single initial JSON message containing:

- `schema`: the generation schema
- `frequency`: generation interval in milliseconds
- `seed`, `sequence`, `generator_version`, `contract_hash`: optional deterministic replay fields

Example:

```json
{
  "schema": {
    "type": "object",
    "properties": {
      "temperature": {
        "type": "float",
        "min": 20.0,
        "max": 35.0,
        "precision": 1
      },
      "device_id": {
        "type": "int",
        "min": 1000,
        "max": 9999
      }
    }
  },
  "frequency": 1000
}
```

### Frequency Rules

- Unit: milliseconds
- Minimum supported value: `100`
- Maximum supported value: `10000`

Requests outside the supported range are rejected.

## Supported Schema Types

Randomizer currently supports these schema variants:

- `int`
- `float`
- `string`
- `enum`
- `object`
- `boolean`
- `uuid`
- `list`

## Schema Field Reference

### `int`

Integer generator configuration:

```json
{
  "type": "int",
  "min": 1,
  "max": 100
}
```

Fields:

- `min`: optional, defaults to `-2147483648`
- `max`: optional, defaults to `2147483647`

Rules:

- `min` must be less than or equal to `max`
- `precision` must be between `0` and `9`

### `float`

Floating-point generator configuration:

```json
{
  "type": "float",
  "min": 0.0,
  "max": 1.0,
  "precision": 2
}
```

Fields:

- `min`: optional, defaults to `0.0`
- `max`: optional, defaults to `1.0`
- `precision`: optional, defaults to `2`

Rules:

- `min` must be less than or equal to `max`

### `string`

String generator configuration with exact length:

```json
{
  "type": "string",
  "length": 8,
  "prefix": "usr_",
  "suffix": "_x",
  "string_type": "alphabetic"
}
```

String generator configuration with custom characters:

```json
{
  "type": "string",
  "min_length": 4,
  "max_length": 8,
  "string_type": "custom",
  "custom_charset": "abc123"
}
```

Fields:

- `length`: optional exact string length
- `min_length`: optional minimum string length
- `max_length`: optional maximum string length
- `prefix`: optional string added before the generated value
- `suffix`: optional string added after the generated value
- `string_type`: required string mode, one of `alphabetic`, `numeric`, `alphanumeric`, `custom`
- `custom_charset`: optional charset used only when `string_type` is `custom`

Rules:

- For `alphabetic`, `numeric`, `alphanumeric`, and `custom`, provide either `length` or both `min_length` and `max_length`
- For `custom`, `custom_charset` is required and must not be empty
- For non-`custom` strings, `custom_charset` is ignored
- `min_length` must be less than or equal to `max_length`
- String lengths cannot exceed `100`

### `enum`

Primitive enum generator configuration:

```json
{
  "type": "enum",
  "values": ["pending", 1, true]
}
```

Fields:

- `values`: required non-empty list of primitive JSON values

Rules:

- Supported values are strings, numbers, and booleans
- Objects, arrays, and `null` are not supported in `enum`
- `enum` may mix primitive types in the same `values` list

### `object`

Nested object generator configuration:

```json
{
  "type": "object",
  "properties": {
    "id": {
      "type": "int",
      "min": 1,
      "max": 10
    },
    "value": {
      "type": "float",
      "min": 10.5,
      "max": 99.5,
      "precision": 1
    }
  }
}
```

Fields:

- `properties`: required map of field names to nested schemas

### `boolean`

Boolean generator configuration:

```json
{
  "type": "boolean",
  "true_probability": 50
}
```

Fields:

- `true_probability`: required integer from `0` to `100`; invalid values are rejected

### `uuid`

UUID generator configuration:

```json
{
  "type": "uuid",
  "prefix": "user_",
  "suffix": "_prod"
}
```

Fields:

- `prefix`: optional string added before the generated UUID
- `suffix`: optional string added after the generated UUID

### `list`

List generator configuration with exact length:

```json
{
  "type": "list",
  "length": 3,
  "items": {
    "type": "int",
    "min": 1,
    "max": 10
  }
}
```

List generator configuration with a length range:

```json
{
  "type": "list",
  "min_length": 2,
  "max_length": 5,
  "items": {
    "type": "object",
    "properties": {
      "id": {
        "type": "uuid"
      },
      "active": {
        "type": "boolean",
        "true_probability": 75
      }
    }
  }
}
```

Fields:

- `length`: optional exact list length
- `min_length`: optional minimum list length
- `max_length`: optional maximum list length
- `items`: required nested schema used for every item in the list

Rules:

- Provide either `length` or both `min_length` and `max_length`
- `min_length` must be less than or equal to `max_length`
- List lengths cannot exceed `100`
- All items in a list use the same schema, but that schema can itself be another `list`, an `object`, or any supported primitive type

## Error Responses

REST errors, WebSocket upgrade errors, and WebSocket protocol errors use the same JSON shape:

```json
{
  "code": "invalid_schema",
  "message": "true_probability must be between 0 and 100, got 101"
}
```

Stable codes currently include `invalid_request`, `invalid_schema`, `invalid_frequency`,
`capacity_exceeded`, and `internal_error`.
