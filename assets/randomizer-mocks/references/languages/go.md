# Optional Go Provider Guidance

Randomizer does not bundle a Go analyzer. Use this reference only for an external protocol-v1
provider supplied by the repository or developer.

A trustworthy provider should model active `encoding/json` behavior, including `json` tags,
`omitempty`, pointers, embedded fields, and `MarshalJSON`. Require explicit evidence for exact
string or numeric enum values, nil/omitted/null behavior, wrapper roots, `time.Time` layouts,
custom time types, decimals, UUIDs, byte slices, and maps.

Go field types and constants alone do not prove their wire form. Prefer committed OpenAPI/JSON
Schema when custom marshaling cannot be executed or inspected reliably. A serialized fixture is a
static fallback; if randomized/dynamic output was requested, use it only after explicit user
agreement and report that the dynamic goal was not delivered.

When samples, active `encoding/json` behavior, and consumer/configuration branches provide enough
corroboration, capture those claims in an auditable `.randomizer/sources/` Draft 2020-12 schema and
import it instead of downgrading dynamic intent to a fixture.
