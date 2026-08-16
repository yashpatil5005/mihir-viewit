# ViewIt Contracts

This package is the source of truth for cross-language runtime and distribution contracts.

## Schema families

- `schemas/*.v1.schema.json`: canonical versioned contracts for new architecture work.
- `schemas/legacy-*.schema.json`: strict adapters for current catalogs/packages during migration. They are not permanent authoring formats.

Current canonical schemas cover:

- service descriptors;
- provider descriptors;
- plugin package manifests;
- catalogs and artifacts.

`src/index.ts` provides TypeScript projections and AJV-backed parsers. Runtime callers must parse untrusted/cross-language values rather than cast them.

## Rules

- JSON Schema owns validation semantics.
- Signed policy fields must not gain language-specific silent defaults.
- Contract-breaking changes require a new schema/contract version.
- Legacy schemas may only support shapes present in the baseline inventory.
- Generated or projected language types must remain mechanically checked against schema fixtures.

Validate the current catalog with:

```bash
npm run test:contracts
```

Catalog generation and plugin package validation invoke the same contract gate.
