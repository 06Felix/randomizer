# Optional Java/JVM Provider Guidance

Randomizer does not bundle a Java analyzer. Use this reference only to evaluate or drive an external
protocol-v1 provider supplied by the repository or developer.

A trustworthy provider should use the application's real Jackson, Gson, Moshi, or Kotlin
serialization configuration. It should resolve property aliases, naming policies, inclusion rules,
generic wrappers, polymorphism, custom serializers, validation metadata, and `ObjectMapper`
modules/settings.

Require explicit evidence for:

- exact serialized enum values, including `@JsonValue` and custom representations;
- omission versus explicit null, especially `@JsonInclude` and boxed values;
- `LocalDate`, `Instant`, offset/zoned/local date-time types, `Date`, and custom formats;
- `Duration`, time-only values, `BigDecimal`, UUID, bytes, and wrapper roots;
- property names changed by annotations, strategies, or mixins.

Java class names and enum constants are not wire evidence. If the provider cannot observe active
serializer behavior, use committed OpenAPI/JSON Schema. A serialized fixture is a static fallback;
if randomized/dynamic output was requested, use it only after explicit user agreement and report
that the dynamic goal was not delivered.

When payload samples, the active serializer/type model, and consumer/configuration branches provide
enough corroboration, capture those claims in an auditable `.randomizer/sources/` Draft 2020-12
schema and import it instead of downgrading dynamic intent to a fixture.
