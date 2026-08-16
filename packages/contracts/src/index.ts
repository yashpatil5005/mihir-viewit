import Ajv2020, { type ErrorObject, type ValidateFunction } from "ajv/dist/2020.js";
import catalogV1Schema from "../schemas/catalog.v1.schema.json" with { type: "json" };
import legacyCatalogSchema from "../schemas/legacy-catalog.schema.json" with { type: "json" };
import legacyPluginEntrySchema from "../schemas/legacy-plugin-entry.schema.json" with { type: "json" };
import packageManifestV1Schema from "../schemas/plugin-package-manifest.v1.schema.json" with { type: "json" };
import providerV1Schema from "../schemas/provider-descriptor.v1.schema.json" with { type: "json" };
import serviceV1Schema from "../schemas/service-descriptor.v1.schema.json" with { type: "json" };

export type TrustClass = "built-in" | "trusted-in-process" | "isolated";
export type ProviderRuntime =
  "builtin" | "android-dex" | "android-dex-jni" | "webview-js" | "wasm" | "external-worker";

export interface ServiceDescriptorV1 {
  id: `viewit.${string}`;
  contractVersion: number;
  requestSchema: string;
  responseSchema: string;
  cardinality: "single" | "many";
  scope: "app" | "document" | "operation";
  defaultTimeoutMs?: number;
}

export interface ProviderRequirementV1 {
  service: `viewit.${string}`;
  contractVersion: number;
}

export interface ProviderDescriptorV1 {
  id: string;
  packageId: string;
  service: `viewit.${string}`;
  contractVersion: number;
  runtime: ProviderRuntime;
  trustClass: TrustClass;
  formats?: string[];
  mimeTypes?: string[];
  priority?: number;
  requires?: ProviderRequirementV1[];
  optional?: ProviderRequirementV1[];
  hostGrants?: `viewit.host.${string}`[];
}

export interface PluginPackageManifestV1 {
  schemaVersion: 1;
  id: string;
  name: string;
  version: string;
  description?: string;
  publisher?: string;
  minAppVersion: number;
  runtime: Exclude<ProviderRuntime, "builtin">;
  entryClass?: string;
  jsEntry?: string;
  cssEntry?: string;
  abi?: string;
  abiVersion?: number;
  providers: ProviderDescriptorV1[];
}

export interface LegacyPluginEntry {
  id: string;
  name: string;
  version: string;
  description?: string;
  minAppVersion: number;
  entryClass: string;
  supportedFormats: string[];
  capabilities?: string[];
  base?: "view" | "play" | "edit" | "tool";
  runtime: string;
  abi?: string;
  abiVersion: number;
  downloadUrl: string;
  sizeBytes: number;
  installedSizeBytes: number;
  checksum: string;
  jsEntry?: string;
  cssEntry?: string;
  [key: string]: unknown;
}

export interface LegacyCatalog {
  plugins: LegacyPluginEntry[];
}

export class ContractValidationError extends Error {
  readonly contract: string;
  readonly errors: ErrorObject[];

  constructor(contract: string, errors: ErrorObject[]) {
    super(
      `${contract} validation failed: ${errors
        .map((error) => `${error.instancePath || "/"} ${error.message ?? "is invalid"}`)
        .join("; ")}`,
    );
    this.name = "ContractValidationError";
    this.contract = contract;
    this.errors = errors;
  }
}

const ajv = new Ajv2020({
  allErrors: true,
  strict: true,
  strictRequired: false,
  validateFormats: false,
});
for (const schema of [
  serviceV1Schema,
  providerV1Schema,
  packageManifestV1Schema,
  catalogV1Schema,
  legacyPluginEntrySchema,
  legacyCatalogSchema,
]) {
  ajv.addSchema(schema);
}

const validators = {
  serviceV1: ajv.getSchema<ServiceDescriptorV1>(serviceV1Schema.$id)!,
  providerV1: ajv.getSchema<ProviderDescriptorV1>(providerV1Schema.$id)!,
  packageManifestV1: ajv.getSchema<PluginPackageManifestV1>(packageManifestV1Schema.$id)!,
  legacyPluginEntry: ajv.getSchema<LegacyPluginEntry>(legacyPluginEntrySchema.$id)!,
  legacyCatalog: ajv.getSchema<LegacyCatalog>(legacyCatalogSchema.$id)!,
};

function parse<T>(contract: string, validate: ValidateFunction<T>, value: unknown): T {
  if (validate(value)) return value;
  throw new ContractValidationError(contract, validate.errors ?? []);
}

export const parseServiceDescriptorV1 = (value: unknown) =>
  parse("ServiceDescriptorV1", validators.serviceV1, value);

export const parseProviderDescriptorV1 = (value: unknown) =>
  parse("ProviderDescriptorV1", validators.providerV1, value);

export const parsePluginPackageManifestV1 = (value: unknown) =>
  parse("PluginPackageManifestV1", validators.packageManifestV1, value);

export const parseLegacyCatalog = (value: unknown) =>
  parse("LegacyCatalog", validators.legacyCatalog, value);

export const parseLegacyPluginEntry = (value: unknown) =>
  parse("LegacyPluginEntry", validators.legacyPluginEntry, value);
