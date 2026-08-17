# Changelog

## v1.2.0-rc.3 - 2026-08-17

### Breaking Changes

- Replace `up`, `down`, and `dev` with framework-neutral managed `start` and `stop` commands.
- Remove Spring Boot adapter flags and Maven/Java DTO contract commands from the executable.

### Features

- Install a reusable repository-local `$randomizer-mocks` skill from `randomizer init` so coding
  agents can incrementally add or update requested HTTP mock routes from application evidence.
- Add versioned `randomizer skill sync` with managed-file hashes, local-edit protection, and an
  explicit `--force` replacement path.
- Teach `$randomizer-mocks` to derive bare Draft 2020-12 response contracts from actual service and
  serialization evidence, including exact enums, boolean semantics, requiredness, nullability,
  collections, formats, and validated constraints.
- Run `randomizer start` in the background by default, with readiness checks, logs, status, stale
  state cleanup, idempotent stop behavior, and an opt-in `--foreground` mode.

## v1.2.0-rc.2 - 2026-08-10

### Features

- Add repository-local HTTP mocking through an explicit project manifest and managed CLI.
- Add deterministic route responses, request bindings, response sequences, request history, and
  management endpoints without source-analysis or container-runtime dependencies.
- Add Maven-based Java DTO contract import, refresh, and freshness checks with an embedded,
  Jackson-aware generic type exporter.

### Fixes

- Keep the release lint gate compatible with current stable Clippy.

## v1.1.0

### Features

- boolean: Implemented

- uuid: Implemented

- list: Implemented

- string: Implemented

- enum: Generalizing the enum attribute

## v1.0.0

### Features

- install: added install script

## v0.0.0

### Breaking Changes

- Generator: Implementing int, float and object generators

- compiler: validate schema ranges and return 400 for bad input

### Features

- schema: add JSON schema parser

- compiler: created

- rest: added api endpoint

- WebSocket: Implementing WebSocket

- general: added doc comments and debug logs

- WebSocket: Added frequency and request config

### Fixes

- generator: float datatype fix
