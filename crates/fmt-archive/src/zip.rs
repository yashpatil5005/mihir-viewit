use crate::{ArchiveManifest, Error, InternalArchiveEntry};
use std::io::{Read, Seek};
use zip::ZipArchive;

pub fn list_zip<R: Read + Seek>(mut reader: R) -> Result<ArchiveManifest, Error> {
    let mut archive =
        ZipArchive::new(&mut reader).map_err(|e| Error::Parse(format!("zip: {}", e)))?;

    let mut entries = Vec::with_capacity(archive.len());
    for i in 0..archive.len() {
        let file = archive
            .by_index(i)
            .map_err(|e| Error::Parse(format!("zip entry {}: {}", i, e)))?;

        let name = file.name().to_string();
        let is_dir = file.is_dir();
        let size = file.size();
        let compressed_size = file.compressed_size();
        let modified = file.last_modified().and_then(|dt| {
            // Convert zip DateTime to unix timestamp
            let year = dt.year() as i64;
            let month = dt.month() as u32;
            let day = dt.day() as u32;
            let hour = dt.hour() as u32;
            let minute = dt.minute() as u32;
            let second = dt.second() as u32;
            // Use chrono to convert to timestamp
            use chrono::NaiveDate;
            let naive = NaiveDate::from_ymd_opt(year as i32, month, day)
                .and_then(|d| d.and_hms_opt(hour, minute, second));
            naive.map(|dt| dt.and_utc().timestamp())
        });
        let crc32 = file.crc32();
        let method = format!("{:?}", file.compression());

        let mut entry = InternalArchiveEntry::with_metadata(
            name,
            size,
            compressed_size,
            is_dir,
            modified,
            Some(crc32),
            Some(method),
        );

        // Check if this entry is a nested archive
        if !is_dir {
            let lower = entry.name.to_lowercase();
            entry.set_nested(is_nested_archive(&lower));
        }

        entries.push(entry);
    }

    Ok(ArchiveManifest::new(entries, "zip".to_string()))
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

pub fn extract_entry<R: Read + Seek>(mut reader: R, entry_name: &str) -> Result<Vec<u8>, Error> {
    let mut archive =
        ZipArchive::new(&mut reader).map_err(|e| Error::Parse(format!("zip: {}", e)))?;

    let mut file = archive
        .by_name(entry_name)
        .map_err(|e| Error::Parse(format!("entry '{}': {}", entry_name, e)))?;

    let mut data = Vec::new();
    std::io::copy(&mut file, &mut data)
        .map_err(|e| Error::Parse(format!("extract '{}': {}", entry_name, e)))?;

    Ok(data)
}

pub fn extract_all<R: Read + Seek>(mut reader: R) -> Result<Vec<(String, Vec<u8>)>, Error> {
    let mut archive =
        ZipArchive::new(&mut reader).map_err(|e| Error::Parse(format!("zip: {}", e)))?;

    let mut results = Vec::with_capacity(archive.len());
    for i in 0..archive.len() {
        let mut file = archive
            .by_index(i)
            .map_err(|e| Error::Parse(format!("entry {}: {}", i, e)))?;

        if file.is_dir() {
            continue;
        }

        let name = file.name().to_string();
        let mut data = Vec::new();
        std::io::copy(&mut file, &mut data)
            .map_err(|e| Error::Parse(format!("extract '{}': {}", name, e)))?;

        results.push((name, data));
    }

    Ok(results)
}
