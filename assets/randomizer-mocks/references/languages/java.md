# Java/JVM Wire-Contract Playbook

Use for Java or JVM repositories. Map Java types to the serialized JSON wire shape only after
inspecting the active serializer configuration.

## Inspect first

- Jackson/Gson/Moshi/Kotlin serialization configuration;
- `@JsonProperty`, `@JsonValue`, `@JsonFormat`, `@JsonInclude`, `@JsonCreator`, aliases, and naming
  strategies;
- custom serializers/deserializers and registered modules;
- generic wrappers, polymorphic annotations, validation annotations, fixtures, and integration tests;
- `ObjectMapper` settings such as timestamps, null inclusion, Java time modules, and unknown fields.

## High-risk mappings

| Java source | Contract decision |
| --- | --- |
| `enum` | Use exact serialized values from annotations, serializer, or fixture. Constants are not enough. |
| `boolean` / `Boolean` | Determine both-value validity, nullability, and omission separately. |
| `LocalDate` | Use `string` + `format: date` only when ISO date serialization is evidenced. |
| `Instant`, `OffsetDateTime`, `ZonedDateTime` | Use `date-time` only when RFC 3339 text is evidenced. |
| `LocalDateTime` | Do not assume `date-time`; no offset may be serialized. Use evidence-backed string rules. |
| `LocalTime`, `OffsetTime` | Randomizer has no generated `time` format; use a supported string rule or fixture. |
| `Duration` | Inspect whether the wire value is ISO text, numeric, or an object; use a fixture if unsupported. |
| `Date` | Confirm formatted text versus epoch number from mapper configuration. |
| `BigDecimal` | Confirm JSON number versus quoted string and preserve only evidenced bounds. |
| `UUID` | Use `format: uuid` only when serialized as a UUID string. |
| `byte[]` | Confirm Base64 string, numeric array, or custom representation. |

`@JsonInclude(NON_NULL)` affects omission, not merely nullability. Primitive defaults do not prove that
the property is required. `@JsonValue` can make an enum serialize as a string, number, or object.

Before committing a Java contract, write down the annotation/configuration or fixture proving every
enum value, property name, date/time representation, required field, and null/omission decision.
