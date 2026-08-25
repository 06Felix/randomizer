# Changelog

## Unreleased

### Features

- Add a language-neutral, versioned provider protocol for deriving response contracts from any
  language-specific tool, with source fingerprints, exact claim-level evidence, diagnostics,
  bounded execution, and strict endpoint/schema validation.
- Add managed contract import, analysis, refresh, and freshness checks for Draft 2020-12 JSON
  Schema, exact OpenAPI 3.1 responses, and conservative serialized examples, with recoverable
  artifact/lock transactions and exact manifest endpoint/media-type association checks.
- Add deterministic application endpoint wiring for dotenv, properties, JSON, and YAML settings,
  including explicit shared-base and client path-resolution assertions, rollback on partial apply,
  idempotent apply, and read-only verification.
- Introduce manifest version 2 for structured wiring while continuing to accept legacy version 1
  manifests that do not declare wiring.
- Update `$randomizer-mocks` to use authoritative wire artifacts and deterministic contract/wiring
  commands for both new and existing services.

## v1.2.0-rc.4 - 2026-08-17

### Features

- Expand the bundled `$randomizer-mocks` skill with language-aware playbooks for Java/JVM,
  TypeScript/JavaScript, Python, Go, and Rust, plus a generic wire-contract fallback.
- Add explicit runtime capability guidance and Java serialization rules for enums, Jackson behavior,
  null/omission semantics, and date/time types such as `LocalDate`, `Instant`, and `OffsetDateTime`.
- Require contract generation choices to stay within Randomizer's supported formats, using fixtures or
  developer clarification for unsupported wire shapes.
- Teach the skill to create standalone Randomizer generation request payloads from explicit variable
  semantics and bounds, such as `completion_time_in_minutes` with an integer range.

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

### Fixes

- Make lifecycle status, stop, and stale-state cleanup tolerate concurrent runtime-state removal.

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
