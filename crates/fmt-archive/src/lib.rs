//! Phase 2.9 — archive listing + extract.
//! Supports zip / tar.gz. 7z deferred to Phase 4.4 (sevenz-rust 0.6
//! requires full extraction for listing). RAR5 omitted per ADR 0004.

use viewit_core_types::{ArchiveEntry, Document, Error, Format};

pub fn parse(bytes: &[u8], format: Format, _name: &str) -> Result<Document, Error> {
    let entries = match format {
        Format::ArchiveZip => list_zip(bytes)?,
        Format::ArchiveTar => list_tar(bytes, false)?,
        Format::ArchiveTarGz => list_tar(bytes, true)?,
        Format::Archive7z => {
            return Ok(Document::Unsupported {
                format,
                reason: "7z listing is planned for Phase 4.4 (sevenz-rust 0.6 requires full extraction for listing).".into(),
                suggestion: viewit_core_types::Suggestion::None,
            });
        }
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
