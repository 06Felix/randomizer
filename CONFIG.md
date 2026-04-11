# Configuration Guide

This document describes the configuration that exists in Randomizer today.

## REST Request Configuration

The REST API accepts a JSON schema directly as the request body.

Example:

```json
{
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
}
```

## WebSocket Request Configuration

The WebSocket API expects a single initial JSON message containing:

- `schema`: the generation schema
- `frequency`: generation interval in milliseconds

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

Requests below `100` are rejected.

## Supported Schema Types

Randomizer currently supports these schema variants:

- `int`
- `float`
- `string`
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

String generator configuration with enum values:

```json
{
  "type": "string",
  "string_type": "enum",
  "enum_values": ["pending", "approved", "rejected"],
  "prefix": "status_"
}
```

Fields:

- `length`: optional exact string length
- `min_length`: optional minimum string length
- `max_length`: optional maximum string length
- `prefix`: optional string added before the generated value
- `suffix`: optional string added after the generated value
- `string_type`: required string mode, one of `alphabetic`, `numeric`, `alphanumeric`, `custom`, `enum`
- `custom_charset`: optional charset used only when `string_type` is `custom`
- `enum_values`: optional list of candidate values used only when `string_type` is `enum`

Rules:

- For `alphabetic`, `numeric`, `alphanumeric`, and `custom`, provide either `length` or both `min_length` and `max_length`
- For `custom`, `custom_charset` is required and must not be empty
- For `enum`, `enum_values` is required and must not be empty
- For non-`custom` strings, `custom_charset` is ignored
- For non-`enum` strings, `enum_values` is ignored
- For `enum`, `length`, `min_length`, and `max_length` are ignored
- `min_length` must be less than or equal to `max_length`
- String lengths cannot exceed `100`

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

- `true_probability`: integer from 0 to 100 (values outside this range will be clamped)

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
