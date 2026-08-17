# TypeScript/JavaScript Wire-Contract Playbook

Inspect the runtime serializer and validator, not only TypeScript types.

- Check `JSON.stringify` transformations, Axios/fetch wrappers, `toJSON`, and custom mappers.
- Check Zod, Yup, Joi, io-ts, Valibot, class-transformer, and OpenAPI schemas.
- Resolve enum objects, string enums, numeric enums, aliases, and discriminators to their JSON values.
- Distinguish optional properties from `null` unions and defaults.
- Inspect `Date` serialization, usually an ISO string, before selecting `date-time`.
- Treat `bigint`, `Buffer`, Maps, Sets, and custom classes as high-risk serialized shapes.

Use fixtures when runtime transformations cannot be established from repository evidence.
