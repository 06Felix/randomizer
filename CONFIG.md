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
- `object`
- `boolean`

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

### boolean

Boolean generator configuration:

```json
{
  "type": "boolean",
  "true_probability": 50
}
```

Fields:

- `true_probability`: integer from 0 to 100 (values outside this range will be clamped)
