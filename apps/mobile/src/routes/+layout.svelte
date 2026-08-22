<script lang="ts">
  let { children } = $props();

  // Surface unhandled effect-loop rejections with full stacks in logcat
  // (registered at module scope so it precedes the Viewer mount).
  if (typeof window !== "undefined") {
    window.addEventListener("unhandledrejection", (event) => {
      const reason = event.reason as Error | undefined;
      console.error(
        `[viewit] unhandled rejection: ${reason?.message ?? String(event.reason)}\n${reason?.stack ?? "(no stack)"}`,
      );
    });
    window.addEventListener("error", (event) => {
      console.error(`[viewit] error: ${event.message}\n${event.error?.stack ?? "(no stack)"}`);
    });
  }

  // Polyfill Promise.withResolvers for older WebView versions (ES2024 feature)
  // pdfjs-dist v4 uses this internally, so we must polyfill before any module imports it.
  if (typeof Promise.withResolvers !== "function") {
    Promise.withResolvers = function <T>() {
      let resolve!: (value: T | PromiseLike<T>) => void;
      let reject!: (reason?: any) => void;
      const promise = new Promise<T>((res, rej) => {
        resolve = res;
        reject = rej;
      });
      return { promise, resolve, reject };
    };
  }
</script>

{@render children()}
