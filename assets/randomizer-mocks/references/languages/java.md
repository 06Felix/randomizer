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
serializer behavior, use committed OpenAPI/JSON Schema or a serialized fixture instead.
