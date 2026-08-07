<script lang="ts">
  let { children } = $props();

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
