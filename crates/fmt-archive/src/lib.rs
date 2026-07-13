//! Phase 2.9 — archive listing + extract.
//! Supports zip / tar.gz. 7z deferred to Phase 4.4 (sevenz-rust 0.6
//! requires full extraction for listing). RAR5 omitted per ADR 0004.

use viewit_core_types::{ArchiveEntry, Document, Error, Format};

pub fn parse(bytes: &[u8], format: Format, _name: &str) -> Result<Document, Error> {
    let entries = match format {
        Format::ArchiveZip => list_zip(bytes)?,
        Format::ArchiveTar => list_tar(bytes, false)?,
        Format::ArchiveTarGz => list_tar(bytes, true)?,
        Format::Archive7z => list_7z(bytes)?,
        _ => return Err(Error::UnsupportedFormat(format)),
    };
    Ok(Document::Archive {
        entries,
        format,
        byte_len: bytes.len(),
    })
}

fn list_zip(bytes: &[u8]) -> Result<Vec<ArchiveEntry>, Error> {
    let cursor = std::io::Cursor::new(bytes);
    let mut archive = zip::ZipArchive::new(cursor).map_err(|e| Error::Parse(format!("zip: {}", e)))?;
    let mut out: Vec<ArchiveEntry> = Vec::with_capacity(archive.len() as usize);
    for i in 0..archive.len() {
        if let Ok(file) = archive.by_index(i) {
            out.push(ArchiveEntry {
                name: file.name().to_string(),
                size: file.size(),
                is_dir: file.is_dir(),
                compressed_size: file.compressed_size(),
            });
        }
    }
    Ok(out)
}

fn list_tar(bytes: &[u8], gzipped: bool) -> Result<Vec<ArchiveEntry>, Error> {
    let cursor = std::io::Cursor::new(bytes);
    let mut entries: Vec<ArchiveEntry> = Vec::new();
    let mut archive: tar::Archive<Box<dyn std::io::Read>> = if gzipped {
        tar::Archive::new(Box::new(flate2::read::GzDecoder::new(cursor)))
    } else {
        tar::Archive::new(Box::new(cursor))
    };
    for e in archive.entries().map_err(|e| Error::Parse(format!("tar: {}", e)))? {
        if let Ok(entry) = e {
            let header = entry.header();
            entries.push(ArchiveEntry {
                name: header.path()?.to_string_lossy().into_owned(),
                size: header.size()?,
                is_dir: header.entry_type().is_dir(),
                compressed_size: header.size()?,
            });
        }
    }
    Ok(entries)
}

/// Phase 4.4 — 7z listing via `sevenz-rust`. Uses `archive().files` to
/// read entry metadata without decompressing folder payloads.
fn list_7z(bytes: &[u8]) -> Result<Vec<ArchiveEntry>, Error> {
    let cursor = std::io::Cursor::new(bytes.to_vec());
    let password = sevenz_rust::Password::empty();
    let reader = sevenz_rust::SevenZReader::new(cursor, bytes.len() as u64, password)
        .map_err(|e| Error::Parse(format!("7z: {}", e)))?;
    let files = reader.archive().files.clone();
    let entries = files
        .iter()
        .map(|f| ArchiveEntry {
            name: f.name().to_string(),
            size: f.size(),
            is_dir: f.is_directory(),
            compressed_size: f.size(), // 7z doesn't expose per-file compressed size
        })
        .collect();
    Ok(entries)
}
