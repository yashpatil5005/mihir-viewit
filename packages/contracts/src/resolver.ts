import providersCatalog from "../catalog/providers.v1.json" with { type: "json" };
import { parseProviderDescriptorV1, type ProviderDescriptorV1 } from "./index";

export type ProviderAvailability = "built-in" | "installed" | "downloadable";
export type ProviderHealth = "active" | "degraded" | "failed" | "disabled" | "quarantined";

export interface ProviderCandidate extends ProviderDescriptorV1 {
  availability: ProviderAvailability;
  health: ProviderHealth;
  version?: string;
}

export interface ProviderResolutionRequest {
  service: `viewit.${string}`;
  contractVersion: number;
  format?: string;
  mimeType?: string;
  preferredProviderId?: string;
  availableServices?: ProviderRequirementKey[];
}

export interface ProviderRequirementKey {
  service: `viewit.${string}`;
  contractVersion: number;
}

export type ProviderRejectionReason =
  | "service-mismatch"
  | "contract-version-mismatch"
  | "format-mismatch"
  | "mime-mismatch"
  | "unhealthy"
  | "missing-dependency";

export interface ProviderTraceEntry {
  providerId: string;
  eligible: boolean;
  reasons: ProviderRejectionReason[];
}

export interface ProviderResolutionDecision {
  selected: ProviderCandidate | null;
  candidates: ProviderCandidate[];
  trace: ProviderTraceEntry[];
}

export const registeredProviders = providersCatalog.map(parseProviderDescriptorV1);

const rankAvailability = (availability: ProviderAvailability) =>
  availability === "installed" ? 2 : availability === "built-in" ? 1 : 0;

const hasRequirement = (available: ProviderRequirementKey[], required: ProviderRequirementKey) =>
  available.some(
    (item) =>
      item.service === required.service && item.contractVersion === required.contractVersion,
  );

export function resolveProvider(
  request: ProviderResolutionRequest,
  candidates: ProviderCandidate[],
): ProviderResolutionDecision {
  const trace = candidates.map((provider): ProviderTraceEntry => {
    const reasons: ProviderRejectionReason[] = [];
    if (provider.service !== request.service) reasons.push("service-mismatch");
    if (provider.contractVersion !== request.contractVersion) {
      reasons.push("contract-version-mismatch");
    }
    if (request.format && provider.formats?.length && !provider.formats.includes(request.format)) {
      reasons.push("format-mismatch");
    }
    if (
      request.mimeType &&
      provider.mimeTypes?.length &&
      !provider.mimeTypes.includes(request.mimeType)
    ) {
      reasons.push("mime-mismatch");
    }
    if (
      provider.health === "failed" ||
      provider.health === "disabled" ||
      provider.health === "quarantined"
    ) {
      reasons.push("unhealthy");
    }
    if (
      provider.requires?.some(
        (required) => !hasRequirement(request.availableServices ?? [], required),
      )
    ) {
      reasons.push("missing-dependency");
    }
    return { providerId: provider.id, eligible: reasons.length === 0, reasons };
  });

  const eligible = candidates.filter((_, index) => trace[index].eligible);
  eligible.sort((left, right) => {
    const leftPreferred = left.id === request.preferredProviderId ? 1 : 0;
    const rightPreferred = right.id === request.preferredProviderId ? 1 : 0;
    return (
      rightPreferred - leftPreferred ||
      (right.priority ?? 0) - (left.priority ?? 0) ||
      rankAvailability(right.availability) - rankAvailability(left.availability) ||
      left.id.localeCompare(right.id) ||
      (right.version ?? "").localeCompare(left.version ?? "", undefined, { numeric: true })
    );
  });

  return { selected: eligible[0] ?? null, candidates: eligible, trace };
}

export function providersForPackages(
  packageIds: Iterable<string>,
  availability: ProviderAvailability,
): ProviderCandidate[] {
  const ids = new Set(packageIds);
  return registeredProviders
    .filter((provider) => ids.has(provider.packageId))
    .map((provider) => ({ ...provider, availability, health: "active" }));
}
