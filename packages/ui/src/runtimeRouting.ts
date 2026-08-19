import type { PluginInfo } from "./pluginBridge";
import { providerSupportsFormat } from "@viewit/platform";
import {
  providersForPackages,
  resolveProvider,
  type ProviderCandidate,
  type ProviderHealth,
  type ProviderRequirementKey,
  type ProviderResolutionDecision,
} from "@viewit/contracts/resolver";

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

export type IworkFormat = "pages" | "numbers" | "key";

export function resolveIworkFormat(
  ext: string | null | undefined,
  sourceFormat?: unknown,
): IworkFormat | null {
  for (const value of [sourceFormat, ext]) {
    if (typeof value !== "string") continue;
    const normalized = value
      .trim()
      .toLowerCase()
      .replace(/^iwork-/, "")
      .replace(/^\./, "");
    if (normalized === "pages" || normalized === "numbers" || normalized === "key") {
      return normalized;
    }
  }
  return null;
}

export function pluginSupportsFormat(plugin: PluginInfo, ext: string): boolean {
  return providerSupportsFormat(plugin, ext);
}

export function hasMeaningfulPresentationContent(document: unknown): boolean {
  const slides = (document as { slides?: unknown[] } | null)?.slides;
  if (!Array.isArray(slides)) return false;
  const meaningful = (value: unknown) =>
    typeof value === "string" && value.replace(/<[^>]+>/g, "").trim().length > 2;
  return slides.some((slide) => {
    if (!slide || typeof slide !== "object") return false;
    const record = slide as Record<string, unknown>;
    if (meaningful(record.title) || meaningful(record.body)) return true;
    const elements = Array.isArray(record.elements) ? record.elements : [];
    return elements.some((element) => {
      if (!element || typeof element !== "object") return false;
      const item = element as Record<string, unknown>;
      if (meaningful(item.text)) return true;
      const paragraphs = Array.isArray(item.paragraphs) ? item.paragraphs : [];
      return paragraphs.some((paragraph) => {
        if (!paragraph || typeof paragraph !== "object") return false;
        const runs = Array.isArray((paragraph as Record<string, unknown>).runs)
          ? ((paragraph as Record<string, unknown>).runs as unknown[])
          : [];
        return runs.some(
          (run) =>
            run && typeof run === "object" && meaningful((run as Record<string, unknown>).text),
        );
      });
    });
  });
}

export function resolveInstalledProvider(
  installed: PluginInfo[],
  service: `viewit.${string}`,
  ext: string,
  preferredPackageId: string | null,
  availableServices: ProviderRequirementKey[] = [],
): ProviderResolutionDecision {
  const candidates: ProviderCandidate[] = providersForPackages(
    installed.map((plugin) => plugin.id),
    "installed",
  ).map((provider) => ({
    ...provider,
    version: installed.find((plugin) => plugin.id === provider.packageId)?.version,
    health: ((): ProviderHealth => {
      const state = installed.find((plugin) => plugin.id === provider.packageId)?.providerHealth?.[
        provider.id
      ]?.state;
      return state === "quarantined" || state === "failed" || state === "degraded"
        ? state
        : "active";
    })(),
  }));
  const preferredProviderId = preferredPackageId
    ? candidates.find((provider) => provider.packageId === preferredPackageId)?.id
    : undefined;
  return resolveProvider(
    {
      service,
      contractVersion: 1,
      format: ext,
      preferredProviderId,
      availableServices,
    },
    candidates,
  );
}
