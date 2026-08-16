import type { PluginInfo } from "./pluginBridge";
import { providerSupportsFormat } from "@viewit/platform";
import {
  providersForPackages,
  resolveProvider,
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

export function resolveInstalledProvider(
  installed: PluginInfo[],
  service: `viewit.${string}`,
  ext: string,
  preferredPackageId: string | null,
  availableServices: ProviderRequirementKey[] = [],
): ProviderResolutionDecision {
  const candidates = providersForPackages(
    installed.map((plugin) => plugin.id),
    "installed",
  ).map((provider) => ({
    ...provider,
    version: installed.find((plugin) => plugin.id === provider.packageId)?.version,
    health: (() => {
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
