# Optional Rust Provider Guidance

Randomizer does not bundle a Rust analyzer. Use this reference only for an external protocol-v1
provider supplied by the repository or developer.

A trustworthy provider should resolve Serde `rename`, `rename_all`, skip rules, defaults,
flattening, enum tagging, and custom `Serialize` implementations. Require explicit evidence for
exact enum values and shapes, `Option<T>` omission versus null, wrapper roots, and serialization of
`chrono`, `time`, UUID, decimal, byte, and custom date/time values.

Rust type declarations alone do not establish the wire contract. If custom serialization cannot be
resolved, use committed OpenAPI/JSON Schema or a serialized fixture instead.
