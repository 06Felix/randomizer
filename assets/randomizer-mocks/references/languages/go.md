# Go Wire-Contract Playbook

Inspect `encoding/json` behavior and custom marshaling.

- Resolve `json` tags, `omitempty`, pointer presence, and embedded fields.
- Inspect `MarshalJSON`, `UnmarshalJSON`, `time.Time`, custom time layouts, and aliases.
- Resolve enum constants to the strings or numbers emitted by `MarshalJSON`.
- Distinguish nil pointers, omitted fields, and explicit JSON `null`.
- Check `decimal`, UUID, byte-slice, and map serialization.

Prefer recorded JSON or OpenAPI when custom marshaling is present.
