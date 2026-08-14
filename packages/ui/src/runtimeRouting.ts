import type { PluginInfo } from "./pluginBridge";
import { providerSupportsFormat } from "@viewit/platform";

export const OFFICE_ALL_EXTS = new Set([
  "docx",
  "docm",
  "dotx",
  "dotm",
  "xlsx",
  "xlsm",
  "xlsb",
  "xls",
  "pptx",
  "pptm",
  "potx",
  "odt",
  "ott",
  "ods",
  "ots",
  "odp",
  "otp",
  "doc",
  "ppt",
]);

export const ARCHIVE_EXTS = new Set([
  "zip",
  "7z",
  "rar",
  "tar",
  "gz",
  "tgz",
  "bz2",
  "tbz2",
  "xz",
  "txz",
  "zst",
  "tzst",
  "lz4",
  "lzma",
  "tlz",
]);

export const FONT_EXTS = new Set([
  "ttf",
  "otf",
  "woff",
  "woff2",
  "ttc",
  "pfb",
  "cff",
  "dfont",
  "sfd",
  "ps",
]);

export const OFFICE_KINDS = new Set(["docx", "xlsx", "pptx"]);
export const NON_ARCHIVE_BUNDLE_EXTS = new Set([
  "pages",
  "numbers",
  "key",
  "pages-template",
  "numbers-template",
  "key-template",
]);

export function pluginSupportsFormat(plugin: PluginInfo, ext: string): boolean {
  return providerSupportsFormat(plugin, ext);
}

export function hasPptxJsRenderer(plugin: PluginInfo): boolean {
  return plugin.runtime === "js" && pluginSupportsFormat(plugin, "pptx");
}

export function selectPptxJsPlugin(
  installed: PluginInfo[],
  preferredId: string | null,
): PluginInfo | null {
  if (preferredId === "__builtin__") return null;
  if (preferredId) {
    return (
      installed.find((plugin) => plugin.id === preferredId && hasPptxJsRenderer(plugin)) ?? null
    );
  }
  return installed.find(hasPptxJsRenderer) ?? null;
}

export function selectNativeOfficePlugin(
  installed: PluginInfo[],
  ext: string,
  preferredId: string | null,
  excludedId = "",
): PluginInfo | null {
  if (preferredId === "__builtin__") return null;
  const candidates = installed.filter(
    (plugin) =>
      plugin.id !== excludedId && plugin.runtime !== "js" && pluginSupportsFormat(plugin, ext),
  );
  if (preferredId) return candidates.find((plugin) => plugin.id === preferredId) ?? null;
  return candidates.find((plugin) => plugin.id === "office-universal") ?? candidates[0] ?? null;
}

export function selectDetectedOfficePlugin(
  installed: PluginInfo[],
  detectedKind: string,
  preferredId: string | null,
  failedPluginId = "",
): PluginInfo | null {
  if (preferredId === "__builtin__") return null;
  // A preferred JS renderer still needs a native plugin to produce the parsed
  // Document that the WebView renderer consumes. Missing preferences likewise
  // fall back to automatic native selection after built-in format detection.
  return selectNativeOfficePlugin(installed, detectedKind, null, failedPluginId);
}

export function selectBasePlugin(
  installed: PluginInfo[],
  base: "play" | "edit",
  ext: string,
  allowFallback: boolean,
): PluginInfo | null {
  const candidates = installed.filter((plugin) => plugin.base === base && plugin.runtime === "js");
  return (
    candidates.find((plugin) => pluginSupportsFormat(plugin, ext)) ??
    (allowFallback ? (candidates[0] ?? null) : null)
  );
}
