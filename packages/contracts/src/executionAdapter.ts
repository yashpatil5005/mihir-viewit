import type { ActivationScope, DisposalReport } from "./activationScope";
import type { ProviderDescriptorV1, TrustClass } from "./index";

export interface RuntimeDescriptor {
  id: string;
  version: number;
  trustClass: TrustClass;
  unload: "full" | "scope-only" | "restart-required";
  crashContainment: "none" | "process";
  cancellation: "cooperative" | "terminate";
  maxRequestBytes: number;
  maxResponseBytes: number;
}

export interface ExecutionRequest<T = unknown> {
  id: string;
  service: `viewit.${string}`;
  contractVersion: number;
  payload: T;
  deadlineMs: number;
}

export interface ExecutionResponse<T = unknown> {
  requestId: string;
  result: T;
}

export interface PreparedProvider {
  provider: ProviderDescriptorV1;
  scope: ActivationScope;
  implementation: unknown;
}

export interface ExecutionAdapter {
  describe(): RuntimeDescriptor;
  prepare(provider: ProviderDescriptorV1, scope: ActivationScope): Promise<PreparedProvider>;
  execute<TRequest, TResponse>(
    prepared: PreparedProvider,
    request: ExecutionRequest<TRequest>,
    operationScope: ActivationScope,
  ): Promise<ExecutionResponse<TResponse>>;
  dispose(prepared: PreparedProvider, reason: string): Promise<DisposalReport>;
}

export function validateExecutionRequest(
  runtime: RuntimeDescriptor,
  request: ExecutionRequest,
  encodedBytes: number,
): void {
  if (request.contractVersion < 1) throw new Error("Invalid execution contract version");
  if (request.deadlineMs <= Date.now()) throw new Error("Execution request deadline expired");
  if (encodedBytes > runtime.maxRequestBytes) {
    throw new Error(`Execution request exceeds ${runtime.maxRequestBytes} bytes`);
  }
}

export const BUILTIN_RUNTIME: RuntimeDescriptor = {
  id: "builtin",
  version: 1,
  trustClass: "built-in",
  unload: "scope-only",
  crashContainment: "none",
  cancellation: "cooperative",
  maxRequestBytes: 64 * 1024 * 1024,
  maxResponseBytes: 64 * 1024 * 1024,
};

export const ANDROID_IN_PROCESS_RUNTIME: RuntimeDescriptor = {
  id: "android-dex-jni",
  version: 1,
  trustClass: "trusted-in-process",
  unload: "restart-required",
  crashContainment: "none",
  cancellation: "cooperative",
  maxRequestBytes: 64 * 1024 * 1024,
  maxResponseBytes: 64 * 1024 * 1024,
};

export const WEBVIEW_JS_RUNTIME: RuntimeDescriptor = {
  id: "webview-js",
  version: 1,
  trustClass: "trusted-in-process",
  unload: "scope-only",
  crashContainment: "none",
  cancellation: "cooperative",
  maxRequestBytes: 16 * 1024 * 1024,
  maxResponseBytes: 16 * 1024 * 1024,
};

export const EXTERNAL_WORKER_RUNTIME: RuntimeDescriptor = {
  id: "external-worker",
  version: 1,
  trustClass: "isolated",
  unload: "full",
  crashContainment: "process",
  cancellation: "terminate",
  maxRequestBytes: 64 * 1024 * 1024,
  maxResponseBytes: 64 * 1024 * 1024,
};
