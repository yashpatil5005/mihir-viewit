<script lang="ts">
  let {
    name,
    size = 20,
    strokeWidth = 2,
  }: {
    name:
      | "arrow-left"
      | "arrow-right"
      | "chevron-down"
      | "chevron-up"
      | "file"
      | "file-plus"
      | "folder"
      | "folder-open"
      | "grid"
      | "list"
      | "lock"
      | "moon"
      | "package"
      | "sun"
      | "terminal"
      | "x";
    size?: number;
    strokeWidth?: number;
  } = $props();
</script>

<svg
  xmlns="http://www.w3.org/2000/svg"
  width={size}
  height={size}
  viewBox="0 0 24 24"
  fill="none"
  stroke="currentColor"
  stroke-width={strokeWidth}
  stroke-linecap="round"
  stroke-linejoin="round"
  aria-hidden="true"
  focusable="false"
>
  {#if name === "moon"}
    <path
      d="M20.985 12.486a9 9 0 1 1-9.473-9.472c.405-.022.617.46.402.803a6 6 0 0 0 8.268 8.268c.344-.215.825-.004.803.401"
    />
  {:else if name === "sun"}
    <circle cx="12" cy="12" r="4" />
    <path
      d="M12 2v2M12 20v2M4.93 4.93l1.41 1.41M17.66 17.66l1.41 1.41M2 12h2M20 12h2M6.34 17.66l-1.41 1.41M19.07 4.93l-1.41 1.41"
    />
  {:else if name === "package"}
    <path
      d="M11 21.73a2 2 0 0 0 2 0l7-4A2 2 0 0 0 21 16V8a2 2 0 0 0-1-1.73l-7-4a2 2 0 0 0-2 0l-7 4A2 2 0 0 0 3 8v8a2 2 0 0 0 1 1.73zM12 22V12"
    />
    <path d="m3.29 7 8.71 5 8.71-5M7.5 4.27l9 5.15" />
  {:else if name === "folder-open"}
    <path
      d="m6 14 1.5-2.9A2 2 0 0 1 9.24 10H20a2 2 0 0 1 1.94 2.5l-1.54 6a2 2 0 0 1-1.95 1.5H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h3.9a2 2 0 0 1 1.69.9l.81 1.2a2 2 0 0 0 1.67.9H18a2 2 0 0 1 2 2v2"
    />
  {:else if name === "folder"}
    <path
      d="M20 20H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h3.9a2 2 0 0 1 1.69.9l.81 1.2a2 2 0 0 0 1.67.9H20a2 2 0 0 1 2 2v11a2 2 0 0 1-2 2Z"
    />
  {:else if name === "file"}
    <path d="M14.5 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V7.5Z" />
    <path d="M14 2v6h6" />
  {:else if name === "file-plus"}
    <path d="M14.5 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V7.5Z" />
    <path d="M14 2v6h6M12 18v-6M9 15h6" />
  {:else if name === "terminal"}
    <path d="M12 19h8M4 17l6-6-6-6" />
  {:else if name === "grid"}
    <rect width="7" height="7" x="3" y="3" rx="1" /><rect
      width="7"
      height="7"
      x="14"
      y="3"
      rx="1"
    />
    <rect width="7" height="7" x="14" y="14" rx="1" /><rect
      width="7"
      height="7"
      x="3"
      y="14"
      rx="1"
    />
  {:else if name === "list"}
    <path d="M8 6h13M8 12h13M8 18h13M3 6h.01M3 12h.01M3 18h.01" />
  {:else if name === "x"}
    <path d="M18 6 6 18M6 6l12 12" />
  {:else if name === "arrow-left"}
    <path d="m12 19-7-7 7-7M19 12H5" />
  {:else if name === "arrow-right"}
    <path d="m12 5 7 7-7 7M5 12h14" />
  {:else if name === "chevron-up"}
    <path d="m18 15-6-6-6 6" />
  {:else if name === "chevron-down"}
    <path d="m6 9 6 6 6-6" />
  {:else if name === "lock"}
    <rect width="18" height="11" x="3" y="11" rx="2" /><path d="M7 11V7a5 5 0 0 1 10 0v4" />
  {/if}
</svg>
