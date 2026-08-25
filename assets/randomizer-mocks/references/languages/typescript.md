# Optional TypeScript/JavaScript Provider Guidance

Randomizer does not bundle a TypeScript or JavaScript analyzer. Use this reference only for an
external protocol-v1 provider supplied by the repository or developer.

A trustworthy provider should inspect runtime schemas or transformations such as OpenAPI, Zod, Yup,
Joi, io-ts, Valibot, class-transformer, `toJSON`, Axios/fetch mappers, and `JSON.stringify`
behavior. TypeScript interfaces disappear at runtime and cannot alone establish requiredness or
serialization.

Require explicit evidence for exact enum/discriminator values, optional versus nullable properties,
aliases, wrapper roots, `Date` formatting, and high-risk values such as `bigint`, `Buffer`, maps,
sets, and custom classes. If runtime behavior is unavailable, use a committed schema rather than
accepting provider guesses. A serialized fixture is a static fallback; if randomized/dynamic output
was requested, use it only after explicit user agreement and report that the dynamic goal was not
delivered.

When payload samples, runtime serializer/schema behavior, and consumer/configuration branches
provide enough corroboration, capture those claims in an auditable `.randomizer/sources/` Draft
2020-12 schema and import it instead of downgrading dynamic intent to a fixture.
