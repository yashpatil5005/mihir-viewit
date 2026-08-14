export interface FormatProvider {
  id: string;
  name: string;
  formats: string[];
  installed?: boolean;
  runtime?: string;
  base?: string;
}

export type FormatCapability =
  | { kind: "installed-plugin"; ext: string; plugin: FormatProvider; available: FormatProvider[] }
  | { kind: "built-in"; ext: string; available: FormatProvider[] }
  | { kind: "available-plugin"; ext: string; available: FormatProvider[] }
  | { kind: "unsupported"; ext: string; available: [] };

const BUILT_IN_FORMATS = new Set([
  "txt",
  "text",
  "log",
  "md",
  "markdown",
  "mdown",
  "mkdn",
  "mdx",
  "json",
  "jsonc",
  "json5",
  "jsonl",
  "ndjson",
  "csv",
  "tsv",
  "yaml",
  "yml",
  "xml",
  "html",
  "htm",
  "xhtml",
  "css",
  "ini",
  "cfg",
  "conf",
  "toml",
  "pdf",
  "png",
  "jpg",
  "jpeg",
  "webp",
  "gif",
  "bmp",
  "tif",
  "tiff",
  "svg",
  "svgz",
  "heic",
  "heif",
  "avif",
  "psd",
  "dng",
  "cr2",
  "cr3",
  "nef",
  "arw",
  "orf",
  "rw2",
  "raf",
  "srw",
  "pef",
  "ico",
  "icns",
  "jfif",
  "epub",
  "mobi",
  "azw3",
  "fb2",
  "lrf",
  "pdb",
  "snb",
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
  "zip",
  "tar",
  "tgz",
  "gz",
  "tbz2",
  "bz2",
  "txz",
  "xz",
  "tzst",
  "zst",
  "lz4",
  "tlz",
  "lzma",
  "7z",
  "rar",
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
  "odp",
  "doc",
  "ppt",
  "rtf",
  "pages",
  "numbers",
  "key",
  "plist",
  "ics",
  "vcf",
  "mp4",
  "m4v",
  "webm",
  "mkv",
  "mov",
  "avi",
  "mpg",
  "mpeg",
  "3gp",
  "wmv",
  "flv",
  "ts",
  "asf",
  "f4v",
  "hevc",
  "m2ts",
  "m2v",
  "mjpeg",
  "mts",
  "mxf",
  "ogv",
  "rm",
  "swf",
  "vob",
  "wtv",
  "mp3",
  "m4a",
  "aac",
  "flac",
  "ogg",
  "wav",
  "wma",
  "opus",
  "aif",
  "aiff",
  "ac3",
  "au",
  "caf",
  "dts",
  "m4r",
  "mp2",
  "oga",
  "ra",
]);

const MIME_FORMATS: Record<string, string> = {
  "application/pdf": "pdf",
  "application/json": "json",
  "application/rtf": "rtf",
  "application/msword": "doc",
  "application/vnd.ms-excel": "xls",
  "application/vnd.ms-powerpoint": "ppt",
  "application/vnd.openxmlformats-officedocument.wordprocessingml.document": "docx",
  "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet": "xlsx",
  "application/vnd.openxmlformats-officedocument.presentationml.presentation": "pptx",
  "application/vnd.oasis.opendocument.text": "odt",
  "application/vnd.oasis.opendocument.spreadsheet": "ods",
  "application/vnd.oasis.opendocument.presentation": "odp",
  "text/markdown": "md",
  "text/csv": "csv",
};

export function normalizeFormat(value: string | null | undefined): string {
  return (value ?? "").trim().toLowerCase().replace(/^\./, "");
}

export function formatFromFile(name: string | null | undefined, mime?: string | null): string {
  const base = (name ?? "").split(/[?#]/, 1)[0].split(/[/\\]/).pop() ?? "";
  const dot = base.lastIndexOf(".");
  if (dot >= 0 && dot < base.length - 1) return normalizeFormat(base.slice(dot + 1));
  const normalizedMime = (mime ?? "").toLowerCase().split(";", 1)[0].trim();
  return MIME_FORMATS[normalizedMime] ?? "";
}

export function isBuiltInFormat(ext: string): boolean {
  return BUILT_IN_FORMATS.has(normalizeFormat(ext));
}

export function providerSupportsFormat(provider: FormatProvider, ext: string): boolean {
  const normalized = normalizeFormat(ext);
  return provider.formats.some((format) => normalizeFormat(format) === normalized);
}

export function resolveFormatCapability(
  ext: string,
  installed: FormatProvider[] = [],
  catalog: FormatProvider[] = [],
): FormatCapability {
  const normalized = normalizeFormat(ext);
  const installedPlugin = installed.find(
    (plugin) =>
      providerSupportsFormat(plugin, normalized) &&
      (plugin.base ?? "view") === "view" &&
      plugin.runtime !== "js",
  );
  const seen = new Set<string>();
  const available = catalog.filter((plugin) => {
    if (
      !providerSupportsFormat(plugin, normalized) ||
      (plugin.base ?? "view") !== "view" ||
      plugin.runtime === "js" ||
      installed.some((item) => item.id === plugin.id && providerSupportsFormat(item, normalized)) ||
      seen.has(plugin.id)
    ) {
      return false;
    }
    seen.add(plugin.id);
    return true;
  });
  if (installedPlugin)
    return { kind: "installed-plugin", ext: normalized, plugin: installedPlugin, available };
  if (isBuiltInFormat(normalized)) return { kind: "built-in", ext: normalized, available };
  if (available.length > 0) return { kind: "available-plugin", ext: normalized, available };
  return { kind: "unsupported", ext: normalized, available: [] };
}
