use std::io::{Read, Seek};
use tar::Archive;
use crate::{ArchiveManifest, InternalArchiveEntry, Error};

pub fn list_tar<R: Read + Seek + Send>(reader: R, gzipped: bool) -> Result<ArchiveManifest, Error> {
    let mut archive: Archive<Box<dyn Read + Send>> = if gzipped {
        #[cfg(feature = "gzip")]
        {
            use flate2::read::GzDecoder;
            Archive::new(Box::new(GzDecoder::new(reader)))
        }
        #[cfg(not(feature = "gzip"))]
        {
            return Err(Error::Parse("gzip support not enabled".into()));
        }
    } else {
        Archive::new(Box::new(reader))
    };

    let mut entries = Vec::new();
    for entry in archive.entries()
        .map_err(|e| Error::Parse(format!("tar: {}", e)))?
    {
        let entry = entry.map_err(|e| Error::Parse(format!("tar entry: {}", e)))?;
        let header = entry.header();

        let name = header.path()
            .map_err(|e| Error::Parse(format!("tar path: {}", e)))?
            .to_string_lossy()
            .into_owned();

        let size = header.size()
            .map_err(|e| Error::Parse(format!("tar size: {}", e)))?;
        let is_dir = header.entry_type().is_dir();
        let compressed_size = size;
        let modified: Option<i64> = Some(header.mtime()
            .map_err(|e| Error::Parse(format!("tar mtime: {}", e)))? as i64);
        let method = Some(if gzipped { "gzip" } else { "tar" }.to_string());

        let mut entry_data = InternalArchiveEntry::with_metadata(
            name, size, compressed_size, is_dir,
            modified, None, method
        );

        if !is_dir {
            let lower = entry_data.name.to_lowercase();
            entry_data.set_nested(is_nested_archive(&lower));
        }

        entries.push(entry_data);
    }

    let format = if gzipped { "tar.gz" } else { "tar" }.to_string();
    Ok(ArchiveManifest::new(entries, format))
}

fn is_nested_archive(name: &str) -> bool {
    name.ends_with(".tar.gz") || name.ends_with(".tgz") ||
    name.ends_with(".tar.bz2") || name.ends_with(".tbz2") ||
    name.ends_with(".tar.xz") || name.ends_with(".txz") ||
    name.ends_with(".tar.zst") || name.ends_with(".tzst") ||
    name.ends_with(".tar.lz4") ||
    name.ends_with(".tar.lzma") || name.ends_with(".tlz") ||
    name.ends_with(".zip") || name.ends_with(".7z") ||
    name.ends_with(".rar") || name.ends_with(".gz") ||
    name.ends_with(".bz2") || name.ends_with(".xz") ||
    name.ends_with(".zst") || name.ends_with(".lz4") ||
    name.ends_with(".lzma")
}

pub fn extract_entry<R: Read + Seek>(
    reader: R,
    entry_name: &str,
    _gzipped: bool,
) -> Result<Vec<u8>, Error> {
    let mut archive = Archive::new(Box::new(reader));

    for entry in archive.entries()
        .map_err(|e| Error::Parse(format!("tar: {}", e)))?
    {
        let mut entry = entry.map_err(|e| Error::Parse(format!("tar entry: {}", e)))?;
        let header = entry.header();
        let name = header.path()
            .map_err(|e| Error::Parse(format!("tar path: {}", e)))?
            .to_string_lossy()
            .into_owned();

        if name == entry_name {
            if header.entry_type().is_dir() {
                return Err(Error::Parse("cannot extract directory".into()));
            }

            let mut data = Vec::new();
            std::io::copy(&mut entry, &mut data)
                .map_err(|e| Error::Parse(format!("extract '{}': {}", entry_name, e)))?;
            return Ok(data);
        }
    }

    Err(Error::Parse(format!("entry '{}' not found", entry_name)))
}

pub fn extract_all<R: Read + Seek>(
    reader: R,
    _gzipped: bool,
) -> Result<Vec<(String, Vec<u8>)>, Error> {
    let mut archive = Archive::new(Box::new(reader));
    let mut results = Vec::new();

    for entry in archive.entries()
        .map_err(|e| Error::Parse(format!("tar: {}", e)))?
    {
        let mut entry = entry.map_err(|e| Error::Parse(format!("tar entry: {}", e)))?;
        let header = entry.header();
        let name = header.path()
            .map_err(|e| Error::Parse(format!("tar path: {}", e)))?
            .to_string_lossy()
            .into_owned();

        if header.entry_type().is_dir() {
            continue;
        }

        let mut data = Vec::new();
        std::io::copy(&mut entry, &mut data)
            .map_err(|e| Error::Parse(format!("extract '{}': {}", name, e)))?;

        results.push((name, data));
    }

    Ok(results)
}