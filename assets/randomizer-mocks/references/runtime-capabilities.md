# Randomizer Runtime Capabilities

The project gateway receives normal HTTP requests at `/mock/<service-id>/<path>`. It does not
receive source-language DTOs and it does not proxy unmatched requests to the real service.

## Request and response behavior

The gateway matches service ID, method, literal or parameterized path, exact configured query values,
case-insensitive header names with exact values, and exact JSON-body values addressed by JSON
Pointer. Extra values are allowed unless a matcher checks them.

Each response uses exactly one body source: fixed `inline` JSON, a project-local `fixture`, a
managed Draft 2020-12 `contract`, or no body. Responses may set status, headers, delay, and request
bindings. Multiple responses advance in order and then hold the last response. `randomizer reset`
clears response counters and request history.

Bindings run after body creation:

```text
${request.path.<name>}
${request.query.<name>}
${request.header.<name>}
${request.body./json/pointer}
```

Path, query, and header bindings produce strings. Contract responses are validated again after
binding, so the target schema must accept the bound JSON type.

## Contract lifecycle

`randomizer contract import` supports committed standalone JSON Schema, exact OpenAPI 3.1 response
selection, and conservative serialized-example inference. `randomizer contract analyze` supports
external provider protocol version `1`.
`randomizer contract refresh` reproduces a locked contract from its source, and
`randomizer contract check` detects source or content drift. Managed provenance lives in
`.randomizer/contracts.lock.json`.

The generated schema subset and unsafe fallbacks are listed in [contracts.md](contracts.md).
Provider/import output must validate and compile against that subset before it can be used by a
route. Managed contract envelopes carry `name`, `version`, `source`, `schema`, and
`content_hash`; legacy bare response schemas remain loadable.

## Deterministic local wiring

An initialized manifest has empty service and route lists. Add both explicitly; no CLI command
creates routes. A minimal managed-contract route with local wiring is:

```yaml
services:
  - id: users
    wiring:
      - file: .env.local
        format: dotenv
        selector: USERS_API_URL
        target: service_base_url
        service_base_safety: all_calls_mocked
        service_base_path_behavior: preserves_prefix

routes:
  - id: get-user
    service: users
    match:
      method: GET
      path: /users/{user_id}
    responses:
      - status: 200
        body:
          contract: .randomizer/contracts/get-user-200.json
          mode: valid
```

Reuse stable service/route IDs when the endpoint already exists. Optional response bindings belong
under a response:

```yaml
bindings:
  - target: /id
    source: ${request.path.user_id}
```

The target must already exist in the response, and the contract must accept the bound value.

Supported formats are `dotenv`, `properties`, `json`, and `yaml`. Selectors are an environment
or property key, RFC 6901 JSON Pointer, or YAML dot path respectively. `service_base_url` writes
`http://<host>:<port>/mock/<service-id>` and requires `service_base_safety` to be either
`dedicated_setting` or `all_calls_mocked`, plus `service_base_path_behavior: preserves_prefix` after
the application's real HTTP client has been inspected or tested. That assertion confirms endpoint
resolution retains `/mock/<service-id>` even when an endpoint begins with `/`; many URL resolvers
discard the base path in that case. `route_url` instead requires a `route` belonging to the service
with a static matcher path, writes that route's full mock URL, and must omit both service-base
assertions. Dynamic `{parameter}` and `*` route paths are rejected for `route_url` because they are
not concrete client URLs.

If the project bind host is `0.0.0.0` or `::`, wiring uses `127.0.0.1` or `::1` respectively for
the client URL. Unspecified bind addresses are not valid destinations.

`randomizer wiring apply` updates only exact existing string settings in project-relative files.
`randomizer wiring check` verifies the currently stored values without writing. Paths escaping the
project, missing or duplicate selectors, mixed formats for one file, invalid route ownership, and
non-string target values are errors.

Because unmatched gateway requests return an error rather than passing through, a shared
`service_base_url` is safe only when all calls using it are modeled. Prefer `route_url` for an
application setting that names one endpoint.
