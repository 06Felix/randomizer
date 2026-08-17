# Python Wire-Contract Playbook

Inspect the active response serializer and model configuration.

- Check Pydantic field aliases, validators, optional/default behavior, and JSON mode.
- Check dataclasses, Marshmallow, attrs, FastAPI/OpenAPI, and custom `json.dumps` encoders.
- Resolve Python `Enum` values, not member names.
- Inspect `datetime`, `date`, `time`, `Decimal`, `UUID`, bytes, and timezone handling.
- Confirm whether `None` is emitted, omitted, or converted by an encoder.

Use the serialized fixture or API schema when Python annotations and runtime output differ.
