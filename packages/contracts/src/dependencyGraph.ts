import type { ProviderDescriptorV1 } from "./index";

export interface DependencyIssue {
  providerId: string;
  kind: "missing" | "version-mismatch" | "cycle";
  service: string;
}

export interface DependencyGraphResult {
  activationOrder: string[];
  deactivationOrder: string[];
  issues: DependencyIssue[];
}

export function validateProviderGraph(providers: ProviderDescriptorV1[]): DependencyGraphResult {
  const providersByService = new Map<string, ProviderDescriptorV1[]>();
  for (const provider of providers) {
    const values = providersByService.get(provider.service) ?? [];
    values.push(provider);
    providersByService.set(provider.service, values);
  }

  const issues: DependencyIssue[] = [];
  const dependencies = new Map<string, Set<string>>();
  for (const provider of providers) {
    const providerDependencies = new Set<string>();
    for (const requirement of provider.requires ?? []) {
      const candidates = providersByService.get(requirement.service) ?? [];
      if (candidates.length === 0) {
        issues.push({ providerId: provider.id, kind: "missing", service: requirement.service });
        continue;
      }
      const compatible = candidates.filter(
        (candidate) => candidate.contractVersion === requirement.contractVersion,
      );
      if (compatible.length === 0) {
        issues.push({
          providerId: provider.id,
          kind: "version-mismatch",
          service: requirement.service,
        });
        continue;
      }
      for (const dependency of compatible) providerDependencies.add(dependency.id);
    }
    dependencies.set(provider.id, providerDependencies);
  }

  const order: string[] = [];
  const visiting = new Set<string>();
  const visited = new Set<string>();
  const visit = (providerId: string, path: string[]) => {
    if (visited.has(providerId)) return;
    if (visiting.has(providerId)) {
      for (const id of path.slice(path.indexOf(providerId))) {
        const provider = providers.find((item) => item.id === id);
        issues.push({ providerId: id, kind: "cycle", service: provider?.service ?? "unknown" });
      }
      return;
    }
    visiting.add(providerId);
    for (const dependency of dependencies.get(providerId) ?? []) {
      visit(dependency, [...path, providerId]);
    }
    visiting.delete(providerId);
    visited.add(providerId);
    order.push(providerId);
  };
  for (const provider of [...providers].sort((left, right) => left.id.localeCompare(right.id))) {
    visit(provider.id, []);
  }

  return {
    activationOrder: order,
    deactivationOrder: [...order].reverse(),
    issues: dedupeIssues(issues),
  };
}

function dedupeIssues(issues: DependencyIssue[]): DependencyIssue[] {
  const seen = new Set<string>();
  return issues.filter((issue) => {
    const key = `${issue.providerId}:${issue.kind}:${issue.service}`;
    if (seen.has(key)) return false;
    seen.add(key);
    return true;
  });
}
