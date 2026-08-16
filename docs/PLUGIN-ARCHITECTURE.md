# ViewIt Extension And Provider Architecture

ViewIt keeps common viewing capabilities in a small offline base app and distributes heavier or specialized implementations as optional packages. The architecture distinguishes the package that delivers code from the provider that implements one runtime service.

## Current State

Android currently supports:

- Dex/native extensions implementing app-owned Java/Kotlin interfaces;
- WebView JavaScript bundles with conventional exported factories;
- signed catalog delivery and SHA-256 artifact verification;
- ABI and minimum-app-version filtering;
- built-in fallback when recoverable plugin operations fail.

Current selection still combines extension, format, runtime, base, user preference, interface casts, and some plugin-ID-specific rules. The provider runtime renewal replaces those overlapping mechanisms incrementally.

## Target Modules

### Distribution

Owns catalog trust, package identity, compatibility, download, extraction, version slots, activation, rollback, and recovery. The frontend must not reconstruct signed manifests.

### Provider runtime

Owns service/provider registration, deterministic resolution, activation scopes, dependencies, health, quarantine, and fallback. Built-in implementations register as built-in providers without dynamic loading.

### Execution adapters

Hide runtime-specific invocation for built-in Rust/platform code, Android Dex/native code, WebView JavaScript, and future isolated workers/processes.

## Package Versus Provider

A plugin package is a signed/downloaded distribution artifact. One package may expose multiple providers.

A provider implements one versioned ViewIt service, for example:

- `viewit.document.parse`
- `viewit.document.render`
- `viewit.document.edit`
- `viewit.archive.list`
- `viewit.archive.read-entry`
- `viewit.archive.extract`
- `viewit.media.inspect`
- `viewit.media.play`
- `viewit.media.transcode`

Legacy `base` and `capabilities` fields remain descriptive migration metadata until canonical provider descriptors replace them. They are not host permissions.

## Contracts

JSON Schema is the default authority for catalogs, package manifests, provider descriptors, lifecycle messages, bridge envelopes, and validated plugin output. Generated or mechanically validated Rust, Kotlin/Java, TypeScript, and Python representations must consume that authority.

The initial schemas and TypeScript validators live in `packages/contracts`. Canonical `v1` schemas are separate from strict `legacy-*` migration adapters so current package quirks do not become permanent contracts.

The stable interface must include:

- nominal service identity and contract version;
- request/response schema fingerprints;
- package/provider identity;
- structured errors and progress;
- platform, ABI, and app compatibility;
- required provider dependencies;
- trust class and requested host grants.

## Trust Model

Current downloaded extensions are trusted in-process code:

- Dex/native extensions execute in the app process and receive host access required by the legacy interface.
- JavaScript extensions execute in the main WebView.
- Catalog signatures and checksums prove approved metadata and bytes; they do not sandbox execution.

The target trust classes are:

- **built-in**: shipped with the app;
- **trusted in-process extension**: approved code with app-process/WebView authority;
- **isolated extension**: separate enforceable execution environment with narrow RPC and host grants.

There is no requirement for a separate Android Activity per plugin. Isolation belongs behind an execution adapter and is introduced only where threat, feasibility, and product value justify it.

See [`adr/0011-extension-execution-adapters.md`](adr/0011-extension-execution-adapters.md) for current adapter classifications and the measured-prototype requirements for isolated WebView/worker or Android service execution.

## Activation And Cleanup

The target runtime owns nested application, provider, document-session, and operation scopes. Scopes own callbacks, listeners, timers, styles, object URLs, temporary resources, progress, cancellation, and cleanup reports.

Native libraries that Android cannot unload report restart-required residue. Cleanup must not pretend that deleting package files unloads in-process native code.

## Resolution

The resolver will select providers using:

1. service and contract compatibility;
2. platform/ABI/app compatibility;
3. format/MIME/sniffed kind;
4. provider health and dependency state;
5. explicit user preference;
6. trust/host-grant policy;
7. provider priority/fidelity metadata;
8. stable deterministic tie-break.

Every decision produces a trace. Directory, map, and catalog iteration order must not select behavior.

## Current Plugin Families

- Office Universal: enhanced Office parser/renderer.
- PPTX Vanilla: WebView PPTX renderer.
- Compression Universal: archive list, preview, and extract operations.
- Font Universal: font inspection/render metadata.
- iWork Universal: enhanced Pages, Numbers, and Keynote handling.
- Player Base: JavaScript media player experience.
- Editor Base: JavaScript text/Markdown editing experience.
- FFmpeg Transcoder: current arm64 Dex/JNI in-process media conversion provider.

FFmpeg is currently present in the default signed catalog and partially works on tested devices. Older text saying it is intentionally unpublished is obsolete. It remains a trusted in-process extension during migration; future isolation is an evidence-based execution-adapter decision.

## Build Profiles

- `development`: diagnostics, local plugin install, and unsigned local catalog support.
- `device-test`: optimized test artifact with diagnostics/local install, signed catalogs by default.
- `production`: production signing required; WebView debugging, debug intents, and local plugin install disabled.

No production publication is implied by Gradle's `release` build type.

## Invariants

- Base viewers remain useful without plugins or network.
- Plugin failure falls back where the host can recover safely.
- Signed policy metadata is never reconstructed by an untrusted/intermediate layer.
- Failed updates never delete last-known-good packages.
- Runtime behavior is selected by providers, not canonical plugin IDs.
- Plugin output is validated before rendering.
- Trust and isolation claims must match enforceable adapter behavior.
- Heavy format engines remain optional when practical.

See [`PLUGIN-DISTRIBUTION.md`](PLUGIN-DISTRIBUTION.md) for current operations and [`architecture/README.md`](architecture/README.md) for the full file flow.
