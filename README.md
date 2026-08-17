# Randomizer

Randomizer is a schema-driven Rust service for generating structured JSON payloads with random values over HTTP and WebSockets.

It can also be initialized inside any application repository as an explicit local HTTP mocking
service. A versioned project manifest maps normal third-party requests to inline, fixture, or
contract-generated responses while the repository keeps control of its local application settings.

## Features

- Generate a single random JSON payload with a REST API.
- Stream random JSON payloads continuously over WebSockets.
- Supports int, float, string, enum, boolean, uuid, object, and list generation.
- Import, validate, and generate from JSON Schema Draft 2020-12 contracts.
- Generate valid, minimum, maximum, boundary, invalid, or example payloads reproducibly.
- Mock normal third-party HTTP requests without changing application code.
- Match requests by service, method, path, query, headers, and JSON body values.
- Bind request values into deterministic fixture or contract responses.
- Install a reusable language-aware repository skill that can add or update mocks from serialized
  source behavior, tests, fixtures, types, and existing API specifications, with a generic fallback.

## Use Cases

- Generate synthetic JSON payloads for API testing.
- Simulate event streams for frontend or backend development.
- Produce sample data for demos and prototypes.
- Test consumers that need structured but variable JSON inputs.

## Installation

Use the below commands to install randomizer binary in your system

Supported release platforms are Linux x86-64, macOS Apple Silicon, and Windows x86-64. Installers
reject other operating system and architecture combinations.

### Linux x86-64 / macOS Apple Silicon

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

### Project mocking

From an application repository:

```sh
randomizer init
# Ask your coding agent: $randomizer-mocks add mocks for the requested endpoints.
randomizer verify
randomizer start
```

`init` creates `.randomizer/randomizer.yaml` and installs the repository-local
`$randomizer-mocks` skill. Invoke the skill whenever endpoints need to be added or updated. It
reconciles only the requested routes, preserves unrelated configuration, inspects the service's
actual serialized response behavior, and creates JSON Schema contracts containing evidenced enums,
booleans, required fields, nullability, collections, formats, and constraints. The Randomizer
runtime remains language- and framework-independent.

After updating the Randomizer binary, synchronize the managed repository copy with
`randomizer skill sync`. The command refuses to replace local skill edits unless `--force` is
provided.

`start` runs Randomizer in the background and writes logs under `.randomizer/runtime/`.
Use `randomizer start --foreground` when attached logs are preferable, and stop a managed
background process with `randomizer stop`.

See [Project Mocking](PROJECT_MOCKING.md) for the technical flow, full manifest format, route
matching and response binding, generated files, and lifecycle commands.

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

Standard contracts use a `contract` envelope instead of `schema`; see [CONFIG.md](CONFIG.md) for
the JSON Schema format, validation endpoint, generation modes, and supported keywords.

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
