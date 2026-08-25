# Optional Go Provider Guidance

Randomizer does not bundle a Go analyzer. Use this reference only for an external protocol-v1
provider supplied by the repository or developer.

A trustworthy provider should model active `encoding/json` behavior, including `json` tags,
`omitempty`, pointers, embedded fields, and `MarshalJSON`. Require explicit evidence for exact
string or numeric enum values, nil/omitted/null behavior, wrapper roots, `time.Time` layouts,
custom time types, decimals, UUIDs, byte slices, and maps.

Go field types and constants alone do not prove their wire form. Prefer committed OpenAPI/JSON
Schema or a serialized fixture when custom marshaling cannot be executed or inspected reliably.
