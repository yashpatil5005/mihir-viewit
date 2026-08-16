export interface BridgeTerminal<T> {
  id: string;
  event: "complete" | "error" | "restart-required";
  result?: T;
  error?: string;
}

export interface BridgeProgress {
  id: string;
  event: "progress";
  progress?: number;
}

export type BridgeEnvelope<T = unknown> = BridgeTerminal<T> | BridgeProgress;

interface PendingRequest<T> {
  resolve: (value: T) => void;
  reject: (error: Error) => void;
  progress?: (value: number) => void;
  timeout: ReturnType<typeof setTimeout>;
}

export class RequestRestartRequiredError extends Error {
  constructor(message: string) {
    super(message);
    this.name = "RequestRestartRequiredError";
  }
}

export class BridgeRequestRegistry {
  private readonly pending = new Map<string, PendingRequest<unknown>>();

  request<T>(
    id: string,
    start: () => void,
    options: {
      timeoutMs?: number;
      signal?: AbortSignal;
      onProgress?: (value: number) => void;
    } = {},
  ): Promise<T> {
    if (this.pending.has(id)) throw new Error(`Duplicate bridge request id: ${id}`);
    return new Promise<T>((resolve, reject) => {
      const finish = (error?: Error) => {
        this.remove(id);
        if (error) reject(error);
      };
      const timeout = setTimeout(
        () => finish(new Error(`Bridge request timed out: ${id}`)),
        options.timeoutMs ?? 30_000,
      );
      this.pending.set(id, {
        resolve: resolve as (value: unknown) => void,
        reject,
        progress: options.onProgress,
        timeout,
      });
      if (options.signal) {
        if (options.signal.aborted) {
          finish(new Error(`Bridge request cancelled: ${id}`));
          return;
        }
        options.signal.addEventListener(
          "abort",
          () => finish(new Error(`Bridge request cancelled: ${id}`)),
          { once: true },
        );
      }
      try {
        start();
      } catch (error) {
        finish(error instanceof Error ? error : new Error(String(error)));
      }
    });
  }

  handle<T>(envelope: BridgeEnvelope<T>): boolean {
    const request = this.pending.get(envelope.id);
    if (!request) return false;
    if (envelope.event === "progress") {
      request.progress?.(envelope.progress ?? 0);
      return true;
    }
    this.remove(envelope.id);
    if (envelope.event === "complete") request.resolve(envelope.result as T);
    else if (envelope.event === "restart-required") {
      request.reject(new RequestRestartRequiredError(envelope.error || "Restart required"));
    } else request.reject(new Error(envelope.error || "Bridge request failed"));
    return true;
  }

  get size(): number {
    return this.pending.size;
  }

  private remove(id: string): void {
    const request = this.pending.get(id);
    if (!request) return;
    clearTimeout(request.timeout);
    this.pending.delete(id);
  }
}
