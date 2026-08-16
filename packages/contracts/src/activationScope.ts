export type ScopeState = "active" | "disposing" | "disposed";

export interface DisposalFailure {
  label: string;
  error: unknown;
}

export interface DisposalReport {
  reason: string;
  failures: DisposalFailure[];
}

interface OwnedResource {
  label: string;
  dispose: () => void | Promise<void>;
}

export class ActivationScope {
  readonly signal: AbortSignal;
  private readonly controller = new AbortController();
  private readonly resources: OwnedResource[] = [];
  private disposePromise: Promise<DisposalReport> | null = null;
  state: ScopeState = "active";

  constructor(
    readonly id: string,
    readonly parent?: ActivationScope,
  ) {
    this.signal = this.controller.signal;
    parent?.register(`child:${id}`, async () => {
      await this.dispose("parent-disposed");
    });
  }

  register(label: string, dispose: OwnedResource["dispose"]): () => void {
    if (this.state !== "active") throw new Error(`Scope ${this.id} is ${this.state}`);
    const resource = { label, dispose };
    this.resources.push(resource);
    return () => {
      const index = this.resources.indexOf(resource);
      if (index >= 0) this.resources.splice(index, 1);
    };
  }

  child(id: string): ActivationScope {
    return new ActivationScope(id, this);
  }

  ownObjectUrl(url: string): string {
    this.register(`object-url:${url}`, () => URL.revokeObjectURL(url));
    return url;
  }

  ownEventListener(
    target: EventTarget,
    type: string,
    listener: EventListenerOrEventListenerObject,
    options?: AddEventListenerOptions | boolean,
  ): void {
    target.addEventListener(type, listener, options);
    this.register(`event:${type}`, () => target.removeEventListener(type, listener, options));
  }

  dispose(reason = "disposed"): Promise<DisposalReport> {
    if (this.disposePromise) return this.disposePromise;
    this.disposePromise = this.disposeOwned(reason);
    return this.disposePromise;
  }

  private async disposeOwned(reason: string): Promise<DisposalReport> {
    this.state = "disposing";
    this.controller.abort(reason);
    const failures: DisposalFailure[] = [];
    for (const resource of [...this.resources].reverse()) {
      try {
        await resource.dispose();
      } catch (error) {
        failures.push({ label: resource.label, error });
      }
    }
    this.resources.length = 0;
    this.state = "disposed";
    return { reason, failures };
  }
}
