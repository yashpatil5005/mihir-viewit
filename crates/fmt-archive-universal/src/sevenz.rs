use std::io::{Read, Seek};
#[cfg(feature = "sevenz")]
use sevenz_rust::{SevenZReader, Password};
use crate::{ArchiveManifest, Error};
#[cfg(feature = "sevenz")]
use crate::InternalArchiveEntry;

#[cfg(feature = "sevenz")]
pub fn list_7z<R: Read + Seek>(mut reader: R) -> Result<ArchiveManifest, Error> {
    let pos = reader.stream_position()?;
    let len = reader.seek(std::io::SeekFrom::End(0))?;
    reader.seek(std::io::SeekFrom::Start(pos))?;

    let password = Password::empty();
    let sevenz_reader = SevenZReader::new(reader, len, password)
        .map_err(|e| Error::Parse(format!("7z: {}", e)))?;

    let files = sevenz_reader.archive().files.clone();
    let mut entries = Vec::with_capacity(files.len());

    for f in files {
        let name = f.name().to_string();
        let size = f.size();
        let is_dir = f.is_directory();
        let compressed_size = f.compressed_size().unwrap_or(size);
        let method = f.method().map(|m| format!("{:?}", m));
        let crc32 = f.crc32();

        let mut entry = InternalArchiveEntry::with_metadata(
            name, size, compressed_size, is_dir,
            None, crc32, method
        );

        if !is_dir {
            let lower = entry.name.to_lowercase();
            entry.set_nested(is_nested_archive(&lower));
        }

        entries.push(entry);
    }

    Ok(ArchiveManifest::new(entries, "7z".to_string()))
}

#[cfg(not(feature = "sevenz"))]
pub fn list_7z<R: Read + Seek>(_reader: R) -> Result<ArchiveManifest, Error> {
    Err(Error::Parse("7z support not enabled".into()))
}

#[allow(dead_code)]
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

#[cfg(feature = "sevenz")]
pub fn extract_entry<R: Read + Seek>(
    mut reader: R,
    entry_name: &str,
) -> Result<Vec<u8>, Error> {
    let pos = reader.stream_position()?;
    let len = reader.seek(std::io::SeekFrom::End(0))?;
    reader.seek(std::io::SeekFrom::Start(pos))?;

    let password = Password::empty();
    let mut sevenz_reader = SevenZReader::new(reader, len, password)
        .map_err(|e| Error::Parse(format!("7z: {}", e)))?;

    let archive = sevenz_reader.archive();
    let file_idx = archive.files.iter().position(|f| f.name() == entry_name)
        .ok_or_else(|| Error::Parse(format!("entry '{}' not found", entry_name)))?;

    let mut data = Vec::new();
    sevenz_reader.extract_file(file_idx, &mut data)
        .map_err(|e| Error::Parse(format!("extract '{}': {}", entry_name, e)))?;

    Ok(data)
}

#[cfg(not(feature = "sevenz"))]
pub fn extract_entry<R: Read + Seek>(_reader: R, _entry_name: &str) -> Result<Vec<u8>, Error> {
    Err(Error::Parse("7z support not enabled".into()))
}

#[cfg(feature = "sevenz")]
pub fn extract_all<R: Read + Seek>(
    mut reader: R,
) -> Result<Vec<(String, Vec<u8>)>, Error> {
    let pos = reader.stream_position()?;
    let len = reader.seek(std::io::SeekFrom::End(0))?;
    reader.seek(std::io::SeekFrom::Start(pos))?;

    let password = Password::empty();
    let mut sevenz_reader = SevenZReader::new(reader, len, password)
        .map_err(|e| Error::Parse(format!("7z: {}", e)))?;

    let archive = sevenz_reader.archive();
    let mut results = Vec::new();

    for (idx, file) in archive.files.iter().enumerate() {
        if file.is_directory() {
            continue;
        }

        let name = file.name().to_string();
        let mut data = Vec::new();
        sevenz_reader.extract_file(idx, &mut data)
            .map_err(|e| Error::Parse(format!("extract '{}': {}", name, e)))?;

        results.push((name, data));
    }

    Ok(results)
}

#[cfg(not(feature = "sevenz"))]
pub fn extract_all<R: Read + Seek>(_reader: R) -> Result<Vec<(String, Vec<u8>)>, Error> {
    Err(Error::Parse("7z support not enabled".into()))
}