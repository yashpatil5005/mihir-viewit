// Phase 5.3 — theme store. Must be .svelte.ts so $state is compiled (not raw in bundle).

type Theme = "light" | "dark" | "system";

const STORAGE_KEY = "viewit-theme";

let current: Theme = "system";

if (typeof localStorage !== "undefined") {
  const saved = localStorage.getItem(STORAGE_KEY) as Theme | null;
  if (saved === "light" || saved === "dark" || saved === "system") {
    current = saved;
  }
}

export const theme = $state({ mode: current });

export function setTheme(t: Theme) {
  theme.mode = t;
  if (typeof localStorage !== "undefined") {
    localStorage.setItem(STORAGE_KEY, t);
  }
  applyTheme();
}

export function toggleTheme() {
  const resolved = resolvedTheme();
  setTheme(resolved === "dark" ? "light" : "dark");
}

export function resolvedTheme(): "light" | "dark" {
  if (theme.mode === "system") {
    if (typeof window !== "undefined" && window.matchMedia) {
      return window.matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "light";
    }
    return "light";
  }
  return theme.mode;
}

export function applyTheme() {
  if (typeof document === "undefined") return;
  document.documentElement.dataset.theme = resolvedTheme();
}

if (typeof window !== "undefined" && window.matchMedia) {
  window.matchMedia("(prefers-color-scheme: dark)").addEventListener("change", () => {
    if (theme.mode === "system") applyTheme();
  });
}
