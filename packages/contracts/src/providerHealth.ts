export type ProviderLifecycleState =
  | "registered"
  | "waiting-for-dependencies"
  | "activating"
  | "active"
  | "degraded"
  | "deactivating"
  | "inactive"
  | "failed"
  | "quarantined"
  | "restart-required"
  | "disposed";

export type ProviderFailureKind =
  | "activation"
  | "execution"
  | "timeout"
  | "invalid-output"
  | "cleanup"
  | "transport"
  | "runtime-crash";

export interface ProviderHealthRecord {
  providerId: string;
  state: ProviderLifecycleState;
  consecutiveFailures: number;
  totalFailures: number;
  lastFailureKind?: ProviderFailureKind;
  lastFailureAt?: number;
  updatedAt: number;
}

export type ProviderHealthEvent =
  | { type: "activate"; at: number }
  | { type: "activated"; at: number }
  | { type: "degraded"; at: number }
  | { type: "deactivate"; at: number }
  | { type: "deactivated"; at: number }
  | { type: "restart-required"; at: number }
  | { type: "failure"; kind: ProviderFailureKind; at: number }
  | { type: "retry"; at: number }
  | { type: "dispose"; at: number };

const QUARANTINE_THRESHOLD = 3;

export function initialProviderHealth(providerId: string, at = Date.now()): ProviderHealthRecord {
  return {
    providerId,
    state: "registered",
    consecutiveFailures: 0,
    totalFailures: 0,
    updatedAt: at,
  };
}

export function transitionProviderHealth(
  record: ProviderHealthRecord,
  event: ProviderHealthEvent,
): ProviderHealthRecord {
  if (record.state === "disposed") return record;
  if (event.type === "failure") {
    const consecutiveFailures = record.consecutiveFailures + 1;
    return {
      ...record,
      state: consecutiveFailures >= QUARANTINE_THRESHOLD ? "quarantined" : "failed",
      consecutiveFailures,
      totalFailures: record.totalFailures + 1,
      lastFailureKind: event.kind,
      lastFailureAt: event.at,
      updatedAt: event.at,
    };
  }
  const state: ProviderLifecycleState =
    event.type === "activate"
      ? "activating"
      : event.type === "activated"
        ? "active"
        : event.type === "degraded"
          ? "degraded"
          : event.type === "deactivate"
            ? "deactivating"
            : event.type === "deactivated" || event.type === "retry"
              ? "inactive"
              : event.type === "restart-required"
                ? "restart-required"
                : "disposed";
  return {
    ...record,
    state,
    consecutiveFailures:
      event.type === "activated" || event.type === "retry" ? 0 : record.consecutiveFailures,
    updatedAt: event.at,
  };
}
