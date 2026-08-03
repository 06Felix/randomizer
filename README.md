# Randomizer

Randomizer is a schema-driven Rust service for generating structured JSON payloads with random values over HTTP and WebSockets.

## Features

- Generate a single random JSON payload with a REST API.
- Stream random JSON payloads continuously over WebSockets.
- Supports int, float, string, enum, boolean, uuid, object, and list generation.

## Use Cases

- Generate synthetic JSON payloads for API testing.
- Simulate event streams for frontend or backend development.
- Produce sample data for demos and prototypes.
- Test consumers that need structured but variable JSON inputs.

## Installation

Use the below commands to install randomizer binary in your system

### Linux / Mac

```sh
curl -sSf https://raw.githubusercontent.com/06Felix/randomizer/main/install.sh | bash
```

### Windows

```PowerShell
irm https://raw.githubusercontent.com/06Felix/randomizer/main/install.ps1 | iex
```

## Getting Started

After installation, run the below command. This starts the service at `0.0.0.0:7263`

```sh
randomizer
```

The bind address, port, WebSocket connection limit, and log filter can be configured with
environment variables documented in [CONFIG.md](CONFIG.md).

The endpoint for the REST API is `/generate` and for WebSocket is `/stream`. For more details on configuration go to [CONFIG.md](CONFIG.md)

## Usage

### REST API

Generate one random JSON payload:

```bash
curl -X POST http://localhost:7263/generate \
  -H "Content-Type: application/json" \
  -d '{
    "schema": {
      "type": "object",
      "properties": {
        "age": { "type": "int", "min": 18, "max": 65 },
        "score": { "type": "float", "min": 0.5, "max": 9.5, "precision": 2 }
      }
    },
    "seed": 12345,
    "sequence": 0
  }'
```

Every result contains the generated value plus replay metadata. Omit `seed` to generate one,
then reuse the returned `seed`, `sequence`, and `generator_version` with the same schema to
reproduce the value exactly.

### WebSocket API

Connect to the websocket endpoint at `ws://localhost:7263/stream` and send a request  
shaped like:

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
      "device_id": { "type": "int", "min": 1000, "max": 9999 }
    }
  },
  "frequency": 1000
}
```

`frequency` is in milliseconds and must be between `100` and `10000`.

### Supported Schema Types

- `int`
- `float`
- `string`
- `enum`
- `object`
- `boolean`
- `uuid`
- `list`

## Known Issues

Only generator version `1` is currently supported; older versions must remain available before
generation semantics can evolve without breaking replay.

## Up Next

- Request resource limits and graceful shutdown

## Changelog (Latest Version)

Full history: [CHANGELOG.md](CHANGELOG.md)
