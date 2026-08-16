# ADR 0011: Extension Execution Adapters And Isolation

Status: accepted

Date: 2026-08-16

## Context

ViewIt currently executes downloaded Dex/native extensions in the app process and JavaScript extensions in the main WebView. Catalog signatures and artifact checksums establish approved bytes but do not isolate execution.

The provider runtime now resolves versioned services independently from package/runtime mechanics. An execution seam is required before changing authority or process topology.

## Decision

All provider execution will be described through an execution adapter interface with an explicit runtime descriptor:

- trust class;
- unload capability;
- crash containment;
- cancellation strength;
- request/response limits;
- runtime and contract version.

Supported trust classes are:

1. `built-in`
2. `trusted-in-process`
3. `isolated`

An adapter may claim `isolated` only when an enforceable execution environment provides process/worker containment and narrow RPC. Namespaces, contexts, Activities, catalog signatures, and JavaScript conventions do not qualify as isolation.

## Current Adapter Classification

### Built-in Rust/platform

- trust: built-in;
- crash containment: none;
- cancellation: cooperative;
- unload: scope-only.

### Android Dex/native

- trust: trusted in-process;
- crash containment: none;
- cancellation: cooperative;
- unload: restart required for native libraries.

### Main-WebView JavaScript

- trust: trusted in-process;
- crash containment: none;
- cancellation: cooperative;
- unload: host-managed scope cleanup.

### External worker

- trust: isolated;
- crash containment: process;
- cancellation: termination;
- unload: full.

This is a contract target, not a claim that the worker exists today.

An Android `Messenger` media-worker prototype now exists behind `VIEWIT_MEDIA_WORKER_ENABLED=1` for development/device-test builds. It uses a non-exported same-UID `:media_worker` process, bounded path-only requests, app-cache output, and process termination for hard cancellation. Production builds reject the flag until FFmpeg conformance is proven.

## Isolation Feasibility

### Pure JavaScript providers

An isolated WebView or worker is feasible for providers whose input/output fits bounded structured messages and whose UI can be hosted through a narrow facade. The isolated environment must not receive the full Android bridge.

### Dex/native providers

Isolation requires an Android bound service or separate extension package/process. It cannot be implemented safely by assigning each plugin an Activity. Required prototype questions include:

- whether downloaded Dex and native libraries load reliably in the service process;
- how package/version slots are visible to that process;
- binder transaction limits for document models;
- streaming/file-descriptor transport for large inputs and outputs;
- process death and restart semantics;
- URI grant propagation;
- cancellation and timeouts;
- ABI and classloader behavior.

### FFmpeg/media transcoding

FFmpeg is the strongest candidate for a separate worker because operations are long-running, native, cancellable by process termination, and naturally file-oriented. Current in-process behavior remains available until a worker prototype passes fixture, performance, and failure-containment tests.

### Document parsers

Large structured `Document` JSON can exceed comfortable binder limits. A process-isolated parser should return a file/stream handle or chunked result rather than one binder payload. No migration occurs until this transport is measured.

## Consequences

- Current extensions are documented honestly as trusted in-process code.
- Provider resolution and viewers do not depend on execution topology.
- Isolation can be introduced per provider without rewriting service contracts.
- Per-plugin sandboxed Activities are explicitly rejected as an architecture requirement.
- The first isolated implementation should be a measured prototype, not a blanket migration.

## Prototype Exit Criteria

An isolated adapter may become a default only after:

- conformance with canonical request/response contracts;
- host-grant enforcement;
- timeout and cancellation tests;
- process crash containment;
- URI/file transport validation;
- large payload benchmarks;
- Android 7 through current target compatibility;
- physical-device format fixture verification;
- built-in or trusted-adapter fallback.
