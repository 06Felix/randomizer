# Rust Wire-Contract Playbook

Inspect Serde attributes and custom serializers.

- Check `rename`, `rename_all`, `skip`, `skip_serializing_if`, defaults, flattening, and tagging.
- Resolve enum externally/internally/adjacently tagged representations and exact values.
- Inspect `chrono`, `time`, `uuid`, `Decimal`, bytes, and custom date/time serializers.
- Distinguish `Option<T>` omission from explicit `null` according to serializer attributes.
- Treat `#[serde(default)]` as deserialization behavior unless response serialization proves presence.

Use actual JSON fixtures when custom `Serialize` implementations change the wire shape.
