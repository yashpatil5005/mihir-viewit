/** Offline viewer — reject before reading bytes into RAM / IPC. */

export const OPEN_BYTES_CAP = 32 * 1024 * 1024;

const VIDEO_EXT = new Set([
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
]);

const AUDIO_EXT = new Set([
  "mp3",
  "m4a",
  "aac",
  "flac",
  "ogg",
  "wav",
  "wma",
  "opus",
  "8svx",
  "ac3",
  "aiff",
  "amb",
  "au",
  "avr",
  "caf",
  "cdda",
  "cvs",
  "cvsd",
  "cvu",
  "dts",
  "dvms",
  "fap",
  "fssd",
  "gsrt",
  "hcom",
  "htk",
  "ima",
  "ircam",
  "m4r",
  "maud",
  "mp2",
  "nist",
  "oga",
  "paf",
  "prc",
  "pvf",
  "ra",
  "sd2",
  "sln",
  "smp",
  "snd",
  "sndr",
  "sndt",
  "sou",
  "sph",
  "spx",
  "tta",
  "txw",
  "vms",
  "voc",
  "vox",
  "w64",
  "wv",
  "wve",
]);

export function fileExtension(name: string | null | undefined): string {
  if (!name) return "";
  const base = name.split(/[/\\]/).pop() ?? name;
  const i = base.lastIndexOf(".");
  return i >= 0 ? base.slice(i + 1).toLowerCase() : "";
}

export type PickerReject =
  { reject: true; reason: string; openWithExternal: boolean } | { reject: false };

const ARCHIVE_EXT = new Set([
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
  "cab",
  "iso",
  "jar",
  "apk",
]);

export function checkFileBeforeRead(file: File): PickerReject {
  const viewitUri = (file as File & { viewitUri?: string }).viewitUri;
  if (viewitUri) return { reject: false };
  const ext = fileExtension(file.name);
  const mb = (file.size / 1_048_576).toFixed(1);

  if (VIDEO_EXT.has(ext) || AUDIO_EXT.has(ext)) {
    return { reject: false };
  }
  if (ext === "pdf" || ext === "txt" || ext === "log" || ext === "csv") {
    return { reject: false };
  }
  if (ARCHIVE_EXT.has(ext)) {
    return { reject: false };
  }
  if (file.size > OPEN_BYTES_CAP) {
    return {
      reject: true,
      openWithExternal: true,
      reason: `File is ${mb} MB — max ${OPEN_BYTES_CAP / 1_048_576} MB loaded at once. Use Share → another app, or split the file.`,
    };
  }
  return { reject: false };
}

export function unsupportedDocument(
  reason: string,
  openWithExternal: boolean,
): {
  kind: "unsupported";
  format: "unsupported";
  reason: string;
  suggestion: "open-with-external" | "none";
} {
  return {
    kind: "unsupported",
    format: "unsupported",
    reason,
    suggestion: openWithExternal ? "open-with-external" : "none",
  };
}
