//! ViewIt Android Native Plugin — Compression Universal.
//!
//! Lists and extracts every compression format ViewIt ships a premium
//! license for: zip, tar, gzip, bzip2, xz, zstd, lz4, lzma, 7z, rar and the
//! `tar.*` nested chains. Listing never extracts the whole archive; entry
//! previews and extraction are streamed entry-by-entry (7z decrypts its whole
//! folder, which is what the format itself requires).

use jni::objects::{JClass, JString};
use jni::sys::{jboolean, jobject, jstring};
use jni::JNIEnv;

use serde_json::{json, Value};
use std::fs::File;
use std::io::{self, BufReader, Read};
use std::path::{Component, Path, PathBuf};

// ---------------------------------------------------------------------------
// Format detection
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Ac {
    Zip,
    SevenZ,
    Rar,
    Gzip,
    Bzip2,
    Xz,
    Zstd,
    Lz4,
    Lzma,
    Tar,
    Unknown,
}

impl Ac {
    fn id(self) -> &'static str {
        match self {
            Ac::Zip => "zip",
            Ac::SevenZ => "7z",
            Ac::Rar => "rar",
            Ac::Gzip => "gzip",
            Ac::Bzip2 => "bzip2",
            Ac::Xz => "xz",
            Ac::Zstd => "zstd",
            Ac::Lz4 => "lz4",
            Ac::Lzma => "lzma",
            Ac::Tar => "tar",
            Ac::Unknown => "unknown",
        }
    }
}

fn sniff_head(head: &[u8]) -> Ac {
    if head.len() >= 4
        && (head.starts_with(b"PK\x03\x04")
            || head.starts_with(b"PK\x05\x06")
            || head.starts_with(b"PK\x07\x08"))
    {
        return Ac::Zip;
    }
    if head.len() >= 6 && head[..6] == [0x37, 0x7a, 0xbc, 0xaf, 0x27, 0x1c] {
        return Ac::SevenZ;
    }
    if head.len() >= 8 && head.starts_with(b"Rar!\x1a\x07") {
        return Ac::Rar;
    }
    if head.len() >= 2 && head[0] == 0x1f && head[1] == 0x8b {
        return Ac::Gzip;
    }
    if head.len() >= 3 && head.starts_with(b"BZh") {
        return Ac::Bzip2;
    }
    if head.len() >= 6 && head[..6] == [0xfd, 0x37, 0x7a, 0x58, 0x5a, 0x00] {
        return Ac::Xz;
    }
    if head.len() >= 4 && head[..4] == [0x28, 0xb5, 0x2f, 0xfd] {
        return Ac::Zstd;
    }
    if head.len() >= 4 && head[..4] == [0x04, 0x22, 0x4d, 0x18] {
        return Ac::Lz4;
    }
    if head.len() >= 3 && head[..3] == [0x5d, 0x00, 0x00] {
        return Ac::Lzma;
    }
    if head.len() >= 262 && &head[257..262] == b"ustar" {
        return Ac::Tar;
    }
    Ac::Unknown
}

fn sniff_path(path: &Path) -> Result<Ac, String> {
    let mut f = File::open(path).map_err(|e| format!("open: {e}"))?;
    let mut head = vec![0u8; 512];
    let n = f.read(&mut head).map_err(|e| format!("read: {e}"))?;
    head.truncate(n);
    Ok(sniff_head(&head))
}

fn ext_hint_ac(hint: &str) -> Option<Ac> {
    let l = hint.to_lowercase();
    let l = l.trim_start_matches('.').trim();
    let ac = if l.ends_with(".tar.gz") || l.ends_with(".tgz") {
        Some(Ac::Gzip)
    } else if l.ends_with(".tar.bz2") || l.ends_with(".tbz2") {
        Some(Ac::Bzip2)
    } else if l.ends_with(".tar.xz") || l.ends_with(".txz") {
        Some(Ac::Xz)
    } else if l.ends_with(".tar.zst") || l.ends_with(".tzst") {
        Some(Ac::Zstd)
    } else if l.ends_with(".tar.lz4") {
        Some(Ac::Lz4)
    } else if l.ends_with(".tar.lzma") || l.ends_with(".tlz") {
        Some(Ac::Lzma)
    } else if l.ends_with(".zip") {
        Some(Ac::Zip)
    } else if l.ends_with(".7z") {
        Some(Ac::SevenZ)
    } else if l.ends_with(".rar") {
        Some(Ac::Rar)
    } else if l.ends_with(".tar") {
        Some(Ac::Tar)
    } else if l.ends_with(".gz") {
        Some(Ac::Gzip)
    } else if l.ends_with(".bz2") {
        Some(Ac::Bzip2)
    } else if l.ends_with(".xz") {
        Some(Ac::Xz)
    } else if l.ends_with(".zst") {
        Some(Ac::Zstd)
    } else if l.ends_with(".lz4") {
        Some(Ac::Lz4)
    } else if l.ends_with(".lzma") {
        Some(Ac::Lzma)
    } else {
        None
    };
    ac
}

fn resolve_format(path: &Path, hint: &str) -> Result<(Ac, bool), String> {
    // Magic wins; the extension hint only decides Gzip->tar vs single-file
    // framing (decompression sniff handles that), and unknown files.
    let sn = sniff_path(path)?;
    if sn != Ac::Unknown {
        return Ok((sn, false));
    }
    Ok((ext_hint_ac(hint).unwrap_or(Ac::Unknown), false))
}

const NESTED_SUFFIXES: &[&str] = &[
    ".tar.gz",
    ".tar.bz2",
    ".tar.xz",
    ".tar.zst",
    ".tar.lz4",
    ".tar.lzma",
    ".tgz",
    ".tbz2",
    ".txz",
    ".tzst",
    ".tlz",
    ".zip",
    ".7z",
    ".rar",
    ".tar",
    ".gz",
    ".bz2",
    ".xz",
    ".zst",
    ".lz4",
    ".lzma",
];

fn is_nested_name(name: &str) -> bool {
    let l = name.to_lowercase();
    NESTED_SUFFIXES.iter().any(|s| l.ends_with(s))
}

fn strip_compress_suffix(stem: &str) -> String {
    let l = stem.to_lowercase();
    for s in NESTED_SUFFIXES {
        if l.ends_with(s) {
            let cut = stem.len() - s.len();
            return stem[..cut].to_string();
        }
    }
    stem.to_string()
}

// ---------------------------------------------------------------------------
// Decompression stream helpers (single-stream formats + tar.* chains)
// ---------------------------------------------------------------------------

/// Lzma is decoded through lzma-rs (pure Rust, streaming API), which has no
/// `Read` adapter — this bridges it so tar.lzma / .tlz can be consumed like
/// any other decompressed stream. The stateful raw decoder keeps its range
/// coder across chunk boundaries, so input is fed one chunk at a time.
struct LzmaRead<R: Read> {
    inner: Option<BufReader<R>>,
    decoder: Option<(
        lzma_rs::decompress::raw::LzmaDecoder,
        lzma_rs::decompress::Options,
    )>,
    out_buf: Vec<u8>,
    out_pos: usize,
    done: bool,
}

const LZMA_CHUNK: usize = 1 << 16;

impl<R: Read> LzmaRead<R> {
    fn new(inner: R) -> io::Result<Self> {
        let mut br = BufReader::new(inner);
        let opts = lzma_rs::decompress::Options::default();
        let params = lzma_rs::decompress::raw::LzmaParams::read_header(&mut br, &opts)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, format!("lzma header: {e}")))?;
        let decoder = lzma_rs::decompress::raw::LzmaDecoder::new(params, opts.memlimit)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, format!("lzma init: {e}")))?;
        Ok(Self {
            inner: Some(br),
            decoder: Some((decoder, opts)),
            out_buf: Vec::with_capacity(LZMA_CHUNK),
            out_pos: 0,
            done: false,
        })
    }
}

impl<R: Read> Read for LzmaRead<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        while self.out_pos >= self.out_buf.len() {
            if self.done {
                return Ok(0);
            }
            let mut chunk = [0u8; LZMA_CHUNK];
            let n = self.inner.as_mut().unwrap().read(&mut chunk)?;
            if n == 0 {
                self.done = true;
                break;
            }
            self.out_buf.clear();
            self.out_pos = 0;
            let mut input = &chunk[..n];
            let mut output = io::Cursor::new(&mut self.out_buf);
            match self
                .decoder
                .as_mut()
                .unwrap()
                .0
                .decompress(&mut input, &mut output)
            {
                Ok(()) => {
                    if n < LZMA_CHUNK {
                        self.done = true;
                    }
                }
                Err(e) => {
                    self.done = true;
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!("lzma: {e}"),
                    ));
                }
            }
        }
        let avail = self.out_buf.len() - self.out_pos;
        let take = avail.min(buf.len());
        buf[..take].copy_from_slice(&self.out_buf[self.out_pos..self.out_pos + take]);
        self.out_pos += take;
        Ok(take)
    }
}

/// Fresh decompressor over a path, from byte 0 (magic is sniffed by the
/// caller first, files are reopened so partially-consumed readers never leak).
fn stream_from_ac(ac: Ac, path: &Path) -> Result<Box<dyn Read>, String> {
    let f = File::open(path).map_err(|e| format!("open: {e}"))?;
    let r: Box<dyn Read> = match ac {
        Ac::Gzip => Box::new(flate2::read::MultiGzDecoder::new(f)),
        Ac::Bzip2 => Box::new(bzip2::read::MultiBzDecoder::new(f)),
        Ac::Xz => Box::new(xz2::read::XzDecoder::new_multi_decoder(f)),
        Ac::Zstd => {
            Box::new(zstd::stream::read::Decoder::new(f).map_err(|e| format!("zstd: {e}"))?)
        }
        Ac::Lz4 => Box::new(lz4_flex::frame::FrameDecoder::new(f)),
        Ac::Lzma => Box::new(LzmaRead::new(f).map_err(|e| e.to_string())?),
        _ => return Err("not a compressed stream".to_string()),
    };
    Ok(r)
}

/// For gzip/bzip2/xz/zstd/lz4/lzma streams, is the payload a tar archive?
fn stream_is_tar(ac: Ac, path: &Path) -> Result<bool, String> {
    let mut r = stream_from_ac(ac, path)?;
    let mut head = vec![0u8; 512];
    let n = r.read(&mut head).map_err(|e| format!("read: {e}"))?;
    Ok(n >= 262 && &head[257..262] == b"ustar")
}

// ---------------------------------------------------------------------------
// Listing
// ---------------------------------------------------------------------------

fn make_entry(name: String, size: u64, csize: u64, is_dir: bool, method: String) -> Value {
    json!({
        "name": name,
        "size": size,
        "compressed_size": csize,
        "is_dir": is_dir,
        "method": method,
        "nested": (!is_dir) && is_nested_name(&name),
        "encrypted": false,
    })
}

fn list_zip(path: &Path) -> Result<Value, String> {
    let f = File::open(path).map_err(|e| format!("open: {e}"))?;
    let mut zip = zip::ZipArchive::new(f).map_err(|e| format!("zip: {e}"))?;
    let mut entries = Vec::with_capacity(zip.len());
    for i in 0..zip.len() {
        let m = zip.by_index(i).map_err(|e| format!("entry {i}: {e}"))?;
        entries.push(make_entry(
            m.name().to_string(),
            m.size(),
            m.compressed_size(),
            m.is_dir(),
            format!("{:?}", m.compression()),
        ));
    }
    Ok(json!({ "ok": true, "format": "zip", "entries": entries, "notes": [] }))
}

fn open_sevenz(path: &Path) -> Result<sevenz_rust::SevenZReader<std::fs::File>, String> {
    let f = File::open(path).map_err(|e| format!("open: {e}"))?;
    let len = f.metadata().map(|m| m.len()).unwrap_or(0);
    sevenz_rust::SevenZReader::new(f, len, sevenz_rust::Password::empty())
        .map_err(|e| if encrypted_error(&e.to_string()) {
            "This 7z archive is encrypted (password-protected). ViewIt does not support password-protected archives.".to_string()
        } else {
            format!("7z: {e}")
        })
}

fn encrypted_error(msg: &str) -> bool {
    let lower = msg.to_lowercase();
    ["aes", "encrypt", "password", "authentication", "cipherdata"]
        .iter()
        .any(|k| lower.contains(k))
}

fn list_7z(path: &Path) -> Result<Value, String> {
    let rz = open_sevenz(path)?;
    let files = rz.archive().files.clone();
    let mut entries = Vec::with_capacity(files.len());
    for fl in files {
        let name = fl.name().to_string();
        let is_dir = fl.is_directory();
        let size = fl.size();
        let csize = if fl.compressed_size > 0 {
            fl.compressed_size
        } else {
            size
        };
        let method = "7z".to_string();
        let crc = fl.crc;
        entries.push(json!({
            "name": name,
            "size": size,
            "compressed_size": csize,
            "is_dir": is_dir,
            "method": method,
            "crc": crc,
            "nested": (!is_dir) && is_nested_name(&name),
            "encrypted": false,
        }));
    }
    Ok(json!({ "ok": true, "format": "7z", "entries": entries, "notes": [] }))
}

fn list_rar(path: &Path) -> Result<Value, String> {
    let mut arc = unrar::Archive::new(path.to_string_lossy().to_string())
        .list()
        .map_err(|e| format!("rar: {e}"))?;
    // Password-protected headers make `list()` hard-fail with
    // MissingPassword/BadPassword — that error is surfaced to the UI and
    // clearly means "password required", so no need to pre-inspect.
    let v = arc.process().map_err(|e| format!("rar: {e}"))?;
    let mut entries = Vec::with_capacity(v.len());
    let notes: Vec<String> = vec![];
    for e in v {
        let name = e.filename.clone();
        let encrypted = e.is_encrypted();
        entries.push(json!({
            "name": name,
            "size": e.unpacked_size as u64,
            "compressed_size": 0,
            "is_dir": e.is_directory(),
            "method": if e.method == 0 { "store".to_string() } else { format!("method {}", e.method) },
            "nested": (!e.is_directory()) && is_nested_name(&name),
            "encrypted": encrypted,
        }));
    }
    Ok(json!({ "ok": true, "format": "rar", "entries": entries, "notes": notes }))
}

fn list_tar_from_reader(r: Box<dyn Read>, format: &str) -> Result<Value, String> {
    let mut archive = tar::Archive::new(r);
    let iter = archive.entries().map_err(|e| format!("tar: {e}"))?;
    let mut entries = Vec::new();
    for en in iter {
        let en = en.map_err(|e| format!("tar entry: {e}"))?;
        let p = en
            .path()
            .map_err(|e| format!("tar path: {e}"))?
            .into_owned();
        let name = normalize_tar_name(&p);
        let is_dir = en.header().entry_type().is_dir();
        let size = en.size();
        entries.push(make_entry(name, size, 0, is_dir, "tar".to_string()));
    }
    Ok(json!({ "ok": true, "format": format, "entries": entries, "notes": [] }))
}

fn list_tar(path: &Path, format: &str) -> Result<Value, String> {
    let f = File::open(path).map_err(|e| format!("open: {e}"))?;
    list_tar_from_reader(Box::new(f), format)
}

fn list_single(ac: Ac, path: &Path, hint: &str) -> Result<Value, String> {
    let format = ac.id();
    // Entries nested *inside* the stream only exist if it is a tar.* chain.
    if stream_is_tar(ac, path)? {
        let r = stream_from_ac(ac, path)?;
        let mut v = list_tar_from_reader(r, format)?;
        if !hint.is_empty() {
            v["outer"] = json!(hint);
        }
        return Ok(v);
    }
    let mut base = strip_compress_suffix(hint);
    if base.is_empty() {
        base = "archive-content".to_string();
    }
    let entry = json!({
        "name": base,
        "size": 0,
        "compressed_size": 0,
        "is_dir": false,
        "method": format.to_string(),
        "nested": false,
        "encrypted": false,
    });
    Ok(json!({
        "ok": true,
        "format": format,
        "entries": [entry],
        "notes": ["single-file stream — uncompressed size is computed on preview"],
        "single": true,
    }))
}

fn list_archive(path: &Path, hint: &str) -> Result<Value, String> {
    let (ac, _) = resolve_format(path, hint)?;
    match ac {
        Ac::Zip => list_zip(path),
        Ac::SevenZ => list_7z(path),
        Ac::Rar => list_rar(path),
        Ac::Tar => list_tar(path, "tar"),
        Ac::Gzip | Ac::Bzip2 | Ac::Xz | Ac::Zstd | Ac::Lz4 | Ac::Lzma => {
            list_single(ac, path, hint)
        }
        Ac::Unknown => {
            let suffix = if hint.is_empty() {
                String::new()
            } else {
                format!(" (.{hint})")
            };
            Err(format!("unrecognized compression format{suffix}"))
        }
    }
}

// ---------------------------------------------------------------------------
// Safe extraction plumbing
// ---------------------------------------------------------------------------

fn safe_join(root: &Path, name: &str) -> Result<PathBuf, String> {
    let mut out = root.to_path_buf();
    for comp in Path::new(name).components() {
        match comp {
            Component::Normal(c) => out.push(c),
            Component::CurDir => {}
            Component::ParentDir => return Err(format!("unsafe path component '..' in '{name}'")),
            Component::RootDir | Component::Prefix(_) => {
                return Err(format!("absolute path rejected in '{name}'"))
            }
        }
    }
    Ok(out)
}

fn ensure_parent(file: &Path) -> io::Result<()> {
    if let Some(p) = file.parent() {
        std::fs::create_dir_all(p)?;
    }
    Ok(())
}

fn read_capped(mut r: impl Read, cap: u64) -> Result<Option<Vec<u8>>, String> {
    let mut out = Vec::new();
    let mut buf = [0u8; 64 * 1024];
    let mut total = 0u64;
    loop {
        let n = r.read(&mut buf).map_err(|e| format!("read: {e}"))?;
        if n == 0 {
            break;
        }
        total += n as u64;
        if total > cap {
            return Ok(None);
        }
        out.extend_from_slice(&buf[..n]);
    }
    Ok(Some(out))
}

const PREVIEW_CAP: u64 = 64 * 1024 * 1024;

/// Consume a reader to EOF, discarding bytes. Needed to keep sevenz-rust's
/// streaming folder decoder positioned when skipping a solid-archive entry.
fn drain(r: &mut dyn Read) -> std::io::Result<u64> {
    io::copy(r, &mut std::io::sink())
}

/// Normalize a tar member path so listing, single-entry extraction and full
/// extraction all agree on names: GNU `tar .` writes "./x", the UI must not
/// show or require the "./" prefix.
fn normalize_tar_name(p: &std::path::Path) -> String {
    let joined = p
        .iter()
        .map(|c| c.to_string_lossy())
        .collect::<Vec<_>>()
        .join("/");
    if joined == "./" || joined == "." {
        ".".to_string()
    } else {
        joined
            .trim_start_matches("./")
            .trim_start_matches('/')
            .to_string()
    }
}

// ---------------------------------------------------------------------------
// Entry extraction (single)
// ---------------------------------------------------------------------------

fn extract_zip_entry(path: &Path, entry_name: &str) -> Result<Option<Vec<u8>>, String> {
    let f = File::open(path).map_err(|e| format!("open: {e}"))?;
    let mut zip = zip::ZipArchive::new(f).map_err(|e| format!("zip: {e}"))?;
    let name = entry_name;
    // Resolve the exact name, tolerating "./" and leading-slash aliases that
    // some tools bake in. The first lookup's borrow is scoped so a second
    // by_name() borrow is allowed.
    let target = {
        let first = zip.by_name(name);
        match first {
            Ok(_) => name.to_string(),
            Err(_) => {
                let alt = name
                    .trim_start_matches("./")
                    .trim_start_matches('/')
                    .to_string();
                if alt != name {
                    alt
                } else {
                    name.to_string()
                }
            }
        }
    };
    let mut file = match zip.by_name(&target) {
        Ok(f) => f,
        Err(_) => return Err(format!("entry '{entry_name}' not found")),
    };
    if file.is_dir() {
        return Err(format!("'{entry_name}' is a directory"));
    }
    read_capped(&mut file, PREVIEW_CAP)
}

fn extract_7z_entry(path: &Path, entry_name: &str) -> Result<Option<Vec<u8>>, String> {
    let mut rz = open_sevenz(path)?;
    let mut found: Option<Vec<u8>> = None;
    let mut over_cap = false;
    rz.for_each_entries(|entry, reader| {
        if entry.is_directory() {
            return Ok(true);
        }
        if entry.name() == entry_name {
            let data = read_capped(reader, PREVIEW_CAP)
                .map_err(|e| sevenz_rust::Error::other(format!("lzma read: {e}")))?;
            match data {
                Some(b) => found = Some(b),
                None => over_cap = true,
            }
            return Ok(false);
        }
        // Solid archives share one decompression stream per folder, so an
        // entry we skip must still be drained or the next entry reads from
        // the wrong offset (ChecksumVerificationFailed / corrupted bytes).
        drain(reader).map_err(|e| sevenz_rust::Error::other(format!("skip: {e}")))?;
        Ok(true)
    })
    .map_err(|e| format!("7z: {e}"))?;
    if over_cap {
        return Ok(None);
    }
    Ok(Some(found.ok_or_else(|| {
        format!("entry '{entry_name}' not found")
    })?))
}

fn extract_rar_entry(path: &Path, entry_name: &str) -> Result<Option<Vec<u8>>, String> {
    let dest = std::env::temp_dir().join(format!("viewit_rar_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dest);
    std::fs::create_dir_all(&dest).map_err(|e| format!("mkdir: {e}"))?;
    let result = (|| -> Result<Option<Vec<u8>>, String> {
        let mut arc = unrar::Archive::new(path.to_string_lossy().to_string())
            .extract_to(dest.to_string_lossy().to_string())
            .map_err(|e| format!("rar: {e}"))?;
        arc.process().map_err(|e| format!("rar: {e}"))?;
        let target = safe_join(&dest, entry_name)?;
        let f = File::open(&target).map_err(|_| format!("entry '{entry_name}' not found"))?;
        let meta = f.metadata().map_err(|e| format!("stat: {e}"))?;
        if meta.len() > PREVIEW_CAP {
            return Ok(None);
        }
        read_capped(f, PREVIEW_CAP).map_err(|e| format!("read: {e}"))
    })();
    let _ = std::fs::remove_dir_all(&dest);
    result
}

fn extract_tar_entry_from_reader(
    r: Box<dyn Read>,
    entry_name: &str,
) -> Result<Option<Vec<u8>>, String> {
    let mut archive = tar::Archive::new(r);
    let iter = archive.entries().map_err(|e| format!("tar: {e}"))?;
    for en in iter {
        let mut en = en.map_err(|e| format!("tar: {e}"))?;
        let p = en.path().map_err(|e| format!("tar: {e}"))?.into_owned();
        if normalize_tar_name(&p) == entry_name {
            if en.header().entry_type().is_dir() {
                return Err(format!("'{entry_name}' is a directory"));
            }
            return read_capped(&mut en, PREVIEW_CAP);
        }
    }
    Err(format!("entry '{entry_name}' not found"))
}

fn extract_entry(path: &Path, hint: &str, entry_name: &str) -> Result<Option<Vec<u8>>, String> {
    let (ac, _) = resolve_format(path, hint)?;
    match ac {
        Ac::Zip => extract_zip_entry(path, entry_name),
        Ac::SevenZ => extract_7z_entry(path, entry_name),
        Ac::Rar => extract_rar_entry(path, entry_name),
        Ac::Tar => extract_tar_entry_from_reader(
            Box::new(File::open(path).map_err(|e| format!("open: {e}"))?),
            entry_name,
        ),
        Ac::Gzip | Ac::Bzip2 | Ac::Xz | Ac::Zstd | Ac::Lz4 | Ac::Lzma => {
            if stream_is_tar(ac, path)? {
                let r = stream_from_ac(ac, path)?;
                extract_tar_entry_from_reader(r, entry_name)
            } else {
                // single-file stream: the only "entry" is the decompressed payload
                if entry_name != strip_compress_suffix(hint) && entry_name != "archive-content" {
                    return Err(format!("entry '{entry_name}' not found"));
                }
                let r = stream_from_ac(ac, path)?;
                read_capped(r, PREVIEW_CAP)
            }
        }
        Ac::Unknown => Err("unrecognized compression format".to_string()),
    }
}

fn extract_entry_to_file(
    path: &Path,
    hint: &str,
    entry_name: &str,
    out_path: &Path,
) -> Result<u64, String> {
    let (ac, _) = resolve_format(path, hint)?;
    ensure_parent(out_path).map_err(|e| format!("mkdir: {e}"))?;
    let write_stream = |r: &mut dyn Read| -> Result<u64, String> {
        let mut f = File::create(out_path).map_err(|e| format!("create: {e}"))?;
        io::copy(r, &mut f).map_err(|e| format!("copy: {e}"))
    };
    match ac {
        Ac::Zip => {
            let f = File::open(path).map_err(|e| format!("open: {e}"))?;
            let mut zip = zip::ZipArchive::new(f).map_err(|e| format!("zip: {e}"))?;
            let target = {
                let first = zip.by_name(entry_name);
                match first {
                    Ok(_) => entry_name.to_string(),
                    Err(_) => {
                        let alt = entry_name
                            .trim_start_matches("./")
                            .trim_start_matches('/')
                            .to_string();
                        if alt != entry_name {
                            alt
                        } else {
                            entry_name.to_string()
                        }
                    }
                }
            };
            let mut file = match zip.by_name(&target as &str) {
                Ok(f) => f,
                Err(_) => return Err(format!("entry '{entry_name}' not found")),
            };
            if file.is_dir() {
                return Err(format!("'{entry_name}' is a directory"));
            }
            write_stream(&mut file)
        }
        Ac::SevenZ => {
            let mut rz = open_sevenz(path)?;
            let mut bytes_written = 0u64;
            let mut found = false;
            let mut first_err: Option<String> = None;
            rz.for_each_entries(|entry, reader| {
                if entry.is_directory() {
                    return Ok(true);
                }
                if entry.name() == entry_name {
                    found = true;
                    match write_stream(reader) {
                        Ok(n) => bytes_written = n,
                        Err(e) => first_err = Some(e),
                    }
                    return Ok(false);
                }
                // Solid archives share one decompression stream per folder, so an
                // entry we skip must still be drained or the next entry reads from
                // the wrong offset (ChecksumVerificationFailed / corrupted bytes).
                drain(reader).map_err(|e| sevenz_rust::Error::other(format!("skip: {e}")))?;
                Ok(true)
            })
            .map_err(|e| format!("7z: {e}"))?;
            if let Some(e) = first_err {
                return Err(e);
            }
            if !found {
                return Err(format!("entry '{entry_name}' not found"));
            }
            Ok(bytes_written)
        }
        Ac::Rar => match extract_rar_entry(path, entry_name)? {
            Some(b) => {
                std::fs::write(out_path, &b).map_err(|e| format!("write: {e}"))?;
                Ok(b.len() as u64)
            }
            None => Err(format!("'{entry_name}' is too large to save directly")),
        },
        Ac::Tar => {
            let f = File::open(path).map_err(|e| format!("open: {e}"))?;
            let mut archive = tar::Archive::new(f);
            let iter = archive.entries().map_err(|e| format!("tar: {e}"))?;
            for en in iter {
                let mut en = en.map_err(|e| format!("tar: {e}"))?;
                let p = en.path().map_err(|e| format!("tar: {e}"))?.into_owned();
                if normalize_tar_name(&p) == entry_name {
                    if en.header().entry_type().is_dir() {
                        return Err(format!("'{entry_name}' is a directory"));
                    }
                    return write_stream(&mut en);
                }
            }
            Err(format!("entry '{entry_name}' not found"))
        }
        Ac::Gzip | Ac::Bzip2 | Ac::Xz | Ac::Zstd | Ac::Lz4 | Ac::Lzma => {
            if stream_is_tar(ac, path)? {
                let r = stream_from_ac(ac, path)?;
                let mut archive = tar::Archive::new(r);
                let iter = archive.entries().map_err(|e| format!("tar: {e}"))?;
                for en in iter {
                    let mut en = en.map_err(|e| format!("tar: {e}"))?;
                    let p = en.path().map_err(|e| format!("tar: {e}"))?.into_owned();
                    if normalize_tar_name(&p) == entry_name {
                        if en.header().entry_type().is_dir() {
                            return Err(format!("'{entry_name}' is a directory"));
                        }
                        return write_stream(&mut en);
                    }
                }
                Err(format!("entry '{entry_name}' not found"))
            } else {
                let mut r = stream_from_ac(ac, path)?;
                write_stream(&mut r)
            }
        }
        Ac::Unknown => Err("unrecognized compression format".to_string()),
    }
}

// ---------------------------------------------------------------------------
// Full extraction
// ---------------------------------------------------------------------------

fn extract_zip_all(path: &Path, dest: &Path) -> Result<Option<(u64, u64, Vec<String>)>, String> {
    let f = File::open(path).map_err(|e| format!("open: {e}"))?;
    let mut zip = zip::ZipArchive::new(f).map_err(|e| format!("zip: {e}"))?;
    let mut extracted = 0u64;
    let mut bytes = 0u64;
    let mut notes = vec![];
    for i in 0..zip.len() {
        let name = zip
            .by_index(i)
            .map_err(|e| format!("entry {i}: {e}"))?
            .name()
            .to_string();
        let out = match safe_join(dest, &name) {
            Ok(ready) => ready,
            Err(e) => {
                notes.push(format!("{name}: {e}"));
                continue;
            }
        };
        match zip.by_index(i).map_err(|e| format!("entry {i}: {e}")) {
            Ok(m) => {
                if m.is_dir() {
                    let _ = std::fs::create_dir_all(&out);
                } else {
                    let mut m = m;
                    if let Err(e) = ensure_parent(&out).and_then(|_| {
                        let mut f = File::create(&out)?;
                        io::copy(&mut m, &mut f).map(|n| {
                            bytes += n as u64;
                            extracted += 1;
                        })
                    }) {
                        notes.push(format!("{name}: {e}"));
                    }
                }
            }
            Err(e) => notes.push(format!("{name}: {e}")),
        }
    }
    Ok(Some((extracted, bytes, notes)))
}

fn extract_7z_all(path: &Path, dest: &Path) -> Result<Option<(u64, u64, Vec<String>)>, String> {
    let mut rz = open_sevenz(path)?;
    let mut extracted = 0u64;
    let mut bytes = 0u64;
    let mut notes = vec![];
    let mut hard_err: Option<String> = None;
    rz.for_each_entries(|entry, reader| {
        let name = entry.name().to_string();
        if entry.is_directory() {
            let out = match safe_join(dest, &name) {
                Ok(ready) => ready,
                Err(e) => {
                    notes.push(format!("{name}: {e}"));
                    return Ok(true);
                }
            };
            let _ = std::fs::create_dir_all(out);
            return Ok(true);
        }
        let out = match safe_join(dest, &name) {
            Ok(ready) => ready,
            Err(e) => {
                notes.push(format!("{name}: {e}"));
                return Ok(true);
            }
        };
        match ensure_parent(&out).and_then(|_| {
            let mut f = File::create(&out)?;
            io::copy(reader, &mut f).map(|n| {
                bytes += n as u64;
                extracted += 1;
            })
        }) {
            Ok(_) => {}
            Err(e) => {
                notes.push(format!("{name}: {e}"));
                if hard_err.is_none() {
                    hard_err = Some(e.to_string());
                }
                return Ok(false);
            }
        }
        Ok(true)
    })
    .map_err(|e| format!("7z: {e}"))?;
    Ok(Some((extracted, bytes, notes)))
}

fn extract_rar_all(path: &Path, dest: &Path) -> Result<Option<(u64, u64, Vec<String>)>, String> {
    std::fs::create_dir_all(dest).map_err(|e| format!("mkdir: {e}"))?;
    let mut arc = unrar::Archive::new(path.to_string_lossy().to_string())
        .extract_to(dest.to_string_lossy().to_string())
        .map_err(|e| format!("rar: {e}"))?;
    let v = arc.process().map_err(|e| format!("rar: {e}"))?;
    let mut extracted = 0u64;
    let mut bytes = 0u64;
    for e in v {
        if e.is_directory() {
            continue;
        }
        let out = safe_join(dest, &e.filename)?;
        match std::fs::metadata(&out) {
            Ok(m) if m.is_file() => {
                extracted += 1;
                bytes += m.len();
            }
            _ => {}
        }
    }
    Ok(Some((extracted, bytes, vec![])))
}

fn extract_tar_all_from_reader(
    r: Box<dyn Read>,
    dest: &Path,
) -> Result<Option<(u64, u64, Vec<String>)>, String> {
    let mut archive = tar::Archive::new(r);
    let iter = archive.entries().map_err(|e| format!("tar: {e}"))?;
    let mut extracted = 0u64;
    let mut bytes = 0u64;
    let mut notes = vec![];
    for en in iter {
        let mut en = en.map_err(|e| format!("tar: {e}"))?;
        let p = en.path().map_err(|e| format!("tar: {e}"))?.into_owned();
        let name = normalize_tar_name(&p);
        if en.header().entry_type().is_dir() {
            let out = match safe_join(dest, &name) {
                Ok(ready) => ready,
                Err(e) => {
                    notes.push(format!("{name}: {e}"));
                    continue;
                }
            };
            let _ = std::fs::create_dir_all(out);
            continue;
        }
        let out = match safe_join(dest, &name) {
            Ok(ready) => ready,
            Err(e) => {
                notes.push(format!("{name}: {e}"));
                continue;
            }
        };
        match ensure_parent(&out).and_then(|_| {
            let mut f = File::create(&out)?;
            io::copy(&mut en, &mut f).map(|n| {
                bytes += n as u64;
                extracted += 1;
            })
        }) {
            Ok(_) => {}
            Err(e) => notes.push(format!("{name}: {e}")),
        }
    }
    Ok(Some((extracted, bytes, notes)))
}

fn extract_all(path: &Path, hint: &str, dest: &Path) -> Result<Value, String> {
    std::fs::create_dir_all(dest).map_err(|e| format!("mkdir: {e}"))?;
    let (ac, _) = resolve_format(path, hint)?;
    let result: Option<(u64, u64, Vec<String>)> = match ac {
        Ac::Zip => extract_zip_all(path, dest)?,
        Ac::SevenZ => extract_7z_all(path, dest)?,
        Ac::Rar => extract_rar_all(path, dest)?,
        Ac::Tar => extract_tar_all_from_reader(
            Box::new(File::open(path).map_err(|e| format!("open: {e}"))?),
            dest,
        )?,
        Ac::Gzip | Ac::Bzip2 | Ac::Xz | Ac::Zstd | Ac::Lz4 | Ac::Lzma => {
            if stream_is_tar(ac, path)? {
                let r = stream_from_ac(ac, path)?;
                extract_tar_all_from_reader(r, dest)?
            } else {
                // single-file stream → one decompressed file named after the archive
                let name = strip_compress_suffix(hint);
                let out = if name.is_empty() || safe_join(dest, &name).is_err() {
                    dest.join("archive-content")
                } else {
                    dest.join(name)
                };
                let r = stream_from_ac(ac, path)?;
                let mut f = File::create(&out).map_err(|e| format!("create: {e}"))?;
                let n =
                    io::copy(&mut BufReader::new(r), &mut f).map_err(|e| format!("copy: {e}"))?;
                Some((1, n, vec![]))
            }
        }
        Ac::Unknown => return Err("unrecognized compression format".to_string()),
    };
    let (extracted, bytes, notes) = result.unwrap_or((0, 0, vec![]));
    Ok(json!({
        "ok": true,
        "extracted": extracted,
        "bytes": bytes,
        "dir": dest.to_string_lossy().to_string(),
        "format": ac.id(),
        "notes": notes,
    }))
}

// ---------------------------------------------------------------------------
// JNI bindings
// ---------------------------------------------------------------------------

fn js_str(env: &mut JNIEnv, s: &JString) -> String {
    env.get_string(s).expect("Couldn't get java string!").into()
}

fn native_list_archive(path: &str, hint: &str) -> String {
    match list_archive(Path::new(path), hint) {
        Ok(v) => v.to_string(),
        Err(e) => json!({ "ok": false, "error": e }).to_string(),
    }
}

fn native_detect_format(path: &str) -> String {
    match sniff_path(Path::new(path)) {
        Ok(ac) => ac.id().to_string(),
        Err(_) => "unknown".to_string(),
    }
}

#[no_mangle]
pub extern "system" fn Java_ai_viewit_plugins_compressionuniversal_CompressionUniversalPlugin_nativeCanHandleMimeType(
    mut env: JNIEnv,
    _class: JClass,
    mime: JString,
) -> jboolean {
    let mime = js_str(&mut env, &mime);
    let ok = matches!(
        mime.as_str(),
        "application/zip"
            | "application/x-7z-compressed"
            | "application/vnd.rar"
            | "application/x-rar-compressed"
            | "application/x-tar"
            | "application/gzip"
            | "application/x-gzip"
            | "application/x-bzip2"
            | "application/x-xz"
            | "application/zstd"
            | "application/x-lz4"
            | "application/x-lzma"
    );
    ok as jboolean
}

#[no_mangle]
pub extern "system" fn Java_ai_viewit_plugins_compressionuniversal_CompressionUniversalPlugin_nativeCanHandleExt(
    mut env: JNIEnv,
    _class: JClass,
    ext: JString,
) -> jboolean {
    let ext = js_str(&mut env, &ext).to_lowercase();
    let ext = ext.trim_start_matches('.').to_string();
    let ok = matches!(
        ext.as_str(),
        "zip"
            | "7z"
            | "rar"
            | "tar"
            | "gz"
            | "tgz"
            | "bz2"
            | "tbz2"
            | "xz"
            | "txz"
            | "zst"
            | "tzst"
            | "lz4"
            | "lzma"
            | "tlz"
            | "tar.gz"
            | "tar.bz2"
            | "tar.xz"
            | "tar.zst"
            | "tar.lz4"
            | "tar.lzma"
    );
    ok as jboolean
}

/// JSON manifest for a materialized archive: {ok, format, entries[], notes[]}.
#[no_mangle]
pub extern "system" fn Java_ai_viewit_plugins_compressionuniversal_CompressionUniversalPlugin_nativeListArchive(
    mut env: JNIEnv,
    _class: JClass,
    path: JString,
    hint: JString,
) -> jstring {
    let path = js_str(&mut env, &path);
    let hint = js_str(&mut env, &hint);
    let json = native_list_archive(&path, &hint);
    env.new_string(json)
        .expect("Couldn't create java string!")
        .into_raw()
}

/// Detected container id ("zip", "7z", …) or "unknown".
#[no_mangle]
pub extern "system" fn Java_ai_viewit_plugins_compressionuniversal_CompressionUniversalPlugin_nativeDetectFormat(
    mut env: JNIEnv,
    _class: JClass,
    path: JString,
) -> jstring {
    let path = js_str(&mut env, &path);
    let json = native_detect_format(&path);
    env.new_string(json)
        .expect("Couldn't create java string!")
        .into_raw()
}

/// Preview one entry as raw bytes. Returns null when the entry is missing,
/// too large, or encrypted/unsupported.
#[no_mangle]
pub extern "system" fn Java_ai_viewit_plugins_compressionuniversal_CompressionUniversalPlugin_nativeExtractEntry(
    mut env: JNIEnv,
    _class: JClass,
    path: JString,
    hint: JString,
    entry: JString,
) -> jobject {
    let path = js_str(&mut env, &path);
    let hint = js_str(&mut env, &hint);
    let entry = js_str(&mut env, &entry);
    let result = match extract_entry(Path::new(&path), &hint, &entry) {
        Ok(Some(bytes)) => Some(bytes),
        Ok(None) => None,
        Err(_e) => None,
    };
    match result {
        Some(bytes) => match env.byte_array_from_slice(&bytes) {
            Ok(arr) => arr.into_raw(),
            Err(_) => std::ptr::null_mut(),
        },
        None => std::ptr::null_mut(),
    }
}

/// Stream one entry (no 64 MB cap) to a destination file path.
#[no_mangle]
pub extern "system" fn Java_ai_viewit_plugins_compressionuniversal_CompressionUniversalPlugin_nativeExtractEntryToFile(
    mut env: JNIEnv,
    _class: JClass,
    path: JString,
    hint: JString,
    entry: JString,
    out: JString,
) -> jstring {
    let path = js_str(&mut env, &path);
    let hint = js_str(&mut env, &hint);
    let entry = js_str(&mut env, &entry);
    let out = js_str(&mut env, &out);
    let result = extract_entry_to_file(Path::new(&path), &hint, &entry, Path::new(&out))
        .map(|n| json!({ "ok": true, "bytes": n }));
    let json = match result {
        Ok(v) => v.to_string(),
        Err(e) => json!({ "ok": false, "error": e }).to_string(),
    };
    env.new_string(json)
        .expect("Couldn't create java string!")
        .into_raw()
}

/// Full extraction to a directory.
#[no_mangle]
pub extern "system" fn Java_ai_viewit_plugins_compressionuniversal_CompressionUniversalPlugin_nativeExtractAll(
    mut env: JNIEnv,
    _class: JClass,
    path: JString,
    hint: JString,
    dest: JString,
) -> jstring {
    let path = js_str(&mut env, &path);
    let hint = js_str(&mut env, &hint);
    let dest = js_str(&mut env, &dest);
    let json = match extract_all(Path::new(&path), &hint, Path::new(&dest)) {
        Ok(v) => v.to_string(),
        Err(e) => json!({ "ok": false, "error": e }).to_string(),
    };
    env.new_string(json)
        .expect("Couldn't create java string!")
        .into_raw()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn gt_7z(fixture: &str, entry: &str) -> Vec<u8> {
        use std::sync::atomic::{AtomicU64, Ordering};
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let dir = std::env::temp_dir().join(format!(
            "viewit_gt_7z_{}_{}_{}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::Relaxed),
            entry.replace(['/', '.'], "_")
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let st = std::process::Command::new("7z")
            .arg("e")
            .arg("-y")
            .arg(format!("-o{}", dir.display()))
            .arg(fixture)
            .arg(entry)
            .output()
            .unwrap();
        assert!(
            st.status.success(),
            "7z failed: {}",
            String::from_utf8_lossy(&st.stderr)
        );
        let base = entry.rsplit('/').next().unwrap();
        std::fs::read(dir.join(base)).unwrap()
    }

    fn gt_tar(fixture: &str, entry: &str) -> Vec<u8> {
        let out = std::process::Command::new("tar")
            .args(["-xOf", fixture])
            .arg(&format!("./{}", entry.trim_start_matches("./")))
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "tar failed: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        out.stdout
    }

    #[test]
    fn entry_to_file_matches_ground_truth() {
        let fixtures = std::env::var("VIEWIT_FIXTURES").unwrap_or_default();
        if fixtures.is_empty() {
            eprintln!("set VIEWIT_FIXTURES=/tmp/opencode to run");
            return;
        }
        let base = std::path::Path::new(&fixtures);
        let cases: &[(&str, &str, &str)] = &[
            ("sample.7z", "files/hello.txt", "7z"),
            ("sample.7z", "files/numbers.txt", "7z"),
            ("sample.7z", "files/data.json", "7z"),
            ("big.7z", "rand1m.bin", "7z"),
            ("big.7z", "rand5m.bin", "7z"),
            ("big.tar.gz", "rand1m.bin", "tar.gz"),
            ("big.tar.gz", "nested/deep/note.txt", "tar.gz"),
            ("big.zip", "rand1m.bin", "zip"),
        ];
        for (fx, entry, hint) in cases {
            let fixture = base.join(fx);
            let gt = if *hint == "tar.gz" {
                gt_tar(fixture.to_str().unwrap(), entry)
            } else {
                gt_7z(fixture.to_str().unwrap(), entry)
            };
            let out = std::env::temp_dir().join(format!(
                "out_{}_{}",
                fx.replace(['.', '/'], "_"),
                entry.replace('/', "_")
            ));
            let _ = std::fs::remove_file(&out);
            let n = extract_entry_to_file(&fixture, hint, entry, &out).unwrap();
            let got = std::fs::read(&out).unwrap();
            assert_eq!(
                gt,
                got,
                "MISMATCH {fx} {entry} (len {} vs {})",
                gt.len(),
                got.len()
            );
            assert_eq!(gt.len() as u64, n);
            eprintln!("OK {fx} {entry} -> {} bytes", got.len());
        }
    }

    #[test]
    fn preview_entry_matches_ground_truth() {
        let fixtures = std::env::var("VIEWIT_FIXTURES").unwrap_or_default();
        if fixtures.is_empty() {
            eprintln!("set VIEWIT_FIXTURES=/tmp/opencode to run");
            return;
        }
        let base = std::path::Path::new(&fixtures);
        let cases: &[(&str, &str, &str)] = &[
            ("big.7z", "rand1m.bin", "7z"),
            ("big.7z", "rand5m.bin", "7z"),
            ("big.7z", "nested/deep/note.txt", "7z"),
            ("big.zip", "rand1m.bin", "zip"),
            ("big.zip", "rand5m.bin", "zip"),
            ("big.zip", "nested/deep/note.txt", "zip"),
            ("big.tar.gz", "rand1m.bin", "tar.gz"),
            ("big.tar.gz", "nested/deep/note.txt", "tar.gz"),
        ];
        for (fx, entry, hint) in cases {
            let fixture = base.join(fx);
            let got = extract_entry(&fixture, hint, entry)
                .map_err(|e| format!("{fx} {entry}: {e}"))
                .unwrap();
            let got = got.expect(&format!("{fx} {entry}: preview returned None (too large?)"));
            let gt = if *hint == "tar.gz" {
                gt_tar(fixture.to_str().unwrap(), entry)
            } else {
                let dir = std::env::temp_dir().join(format!("viewit_pv_{}", std::process::id()));
                let _ = std::fs::remove_dir_all(&dir);
                std::fs::create_dir_all(&dir).unwrap();
                let st = std::process::Command::new("7z")
                    .arg("e")
                    .arg("-y")
                    .arg(format!("-o{}", dir.display()))
                    .arg(&fixture)
                    .arg(entry)
                    .output()
                    .unwrap();
                assert!(
                    st.status.success(),
                    "{fx}: 7z failed {entry}: {}",
                    String::from_utf8_lossy(&st.stderr)
                );
                std::fs::read(dir.join(entry.rsplit('/').next().unwrap())).unwrap()
            };
            assert_eq!(
                gt,
                got,
                "MISMATCH preview {fx} {entry} (len {} vs {})",
                gt.len(),
                got.len()
            );
            eprintln!("OK preview {fx} {entry} -> {} bytes", got.len());
        }
    }

    #[test]
    fn extract_all_matches_ground_truth() {
        let fixtures = std::env::var("VIEWIT_FIXTURES").unwrap_or_default();
        if fixtures.is_empty() {
            return;
        }
        let base = std::path::Path::new(&fixtures);
        let dest = std::env::temp_dir().join(format!("viewit_all_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dest);
        let v = extract_all(&base.join("sample.7z"), "7z", &dest).unwrap();
        assert_eq!(v["ok"], true);
        eprintln!("extract_all: {}", v);
        for (entry, gt) in [
            (
                "files/hello.txt",
                gt_7z(base.join("sample.7z").to_str().unwrap(), "files/hello.txt"),
            ),
            (
                "files/numbers.txt",
                gt_7z(
                    base.join("sample.7z").to_str().unwrap(),
                    "files/numbers.txt",
                ),
            ),
        ] {
            let got = std::fs::read(dest.join(entry)).unwrap();
            assert_eq!(gt, got, "MISMATCH all {entry}");
            eprintln!("OK all {entry} -> {} bytes", got.len());
        }
    }
}
