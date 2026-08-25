# Optional Python Provider Guidance

Randomizer does not bundle a Python analyzer. Use this reference only for an external protocol-v1
provider supplied by the repository or developer.

A trustworthy provider should use active Pydantic JSON schema/serialization, FastAPI OpenAPI,
Marshmallow, dataclass/attrs configuration, or the application's custom JSON encoder. Require
evidence for field aliases, emitted versus omitted defaults and `None`, exact `Enum` values,
wrapper roots, and the JSON representation of `datetime`, `date`, `time`, `Decimal`, UUID, and
bytes.

Annotations alone do not prove runtime serialization. If the provider cannot resolve model
configuration and JSON mode, use committed OpenAPI/JSON Schema. A serialized fixture is a static
fallback; if randomized/dynamic output was requested, use it only after explicit user agreement and
report that the dynamic goal was not delivered.

When payload samples, active model serialization, and consumer/configuration branches provide
enough corroboration, capture those claims in an auditable `.randomizer/sources/` Draft 2020-12
schema and import it instead of downgrading dynamic intent to a fixture.
