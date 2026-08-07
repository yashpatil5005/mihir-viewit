use crate::InternalArchiveEntry;

pub fn detect_nested_archives(entries: &mut [InternalArchiveEntry]) {
    for entry in entries {
        if entry.is_dir {
            continue;
        }
        let lower = entry.name.to_lowercase();
        entry.set_nested(is_nested_archive(&lower));
    }
}

fn is_nested_archive(name: &str) -> bool {
    name.ends_with(".tar.gz")
        || name.ends_with(".tgz")
        || name.ends_with(".tar.bz2")
        || name.ends_with(".tbz2")
        || name.ends_with(".tar.xz")
        || name.ends_with(".txz")
        || name.ends_with(".tar.zst")
        || name.ends_with(".tzst")
        || name.ends_with(".tar.lz4")
        || name.ends_with(".tar.lzma")
        || name.ends_with(".tlz")
        || name.ends_with(".zip")
        || name.ends_with(".7z")
        || name.ends_with(".rar")
        || name.ends_with(".gz")
        || name.ends_with(".bz2")
        || name.ends_with(".xz")
        || name.ends_with(".zst")
        || name.ends_with(".lz4")
        || name.ends_with(".lzma")
}

/// Try to parse a nested archive from entry bytes
pub fn try_parse_nested<R: std::io::Read + std::io::Seek>(
    data: Vec<u8>,
    filename: &str,
) -> Result<crate::ArchiveManifest, crate::Error> {
    let cursor = std::io::Cursor::new(data);
    crate::parse_stream(cursor, filename)
}
