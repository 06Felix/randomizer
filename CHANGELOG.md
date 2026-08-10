# Changelog

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
