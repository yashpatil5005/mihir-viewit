use crate::archive::ArchiveManifest as ManifestType;
use std::io::{Cursor, Read, Seek};
use viewit_core_types::{Document, Error, Format};

pub mod archive;
#[cfg(feature = "bzip2")]
pub mod bzip2;
pub mod entry;
pub mod gzip;
#[cfg(feature = "lz4")]
pub mod lz4;
#[cfg(feature = "lzma")]
pub mod lzma;
pub mod nested;
#[cfg(feature = "rar")]
pub mod rar;
#[cfg(feature = "sevenz")]
pub mod sevenz;
pub mod tar;
#[cfg(feature = "xz")]
pub mod xz;
pub mod zip;
#[cfg(feature = "zstd")]
pub mod zstd;

#[cfg(target_arch = "wasm32")]
pub mod wasm_entry;

// Re-export
pub use crate::entry::ArchiveEntry as InternalArchiveEntry;
pub use archive::ArchiveManifest;
pub use viewit_core_types::ArchiveEntry;

/// Streaming reader wrapper for HTTP Range support
pub struct RangeReader<R: Read + Seek> {
    inner: R,
    start: u64,
    end: Option<u64>,
    pos: u64,
}

impl<R: Read + Seek> RangeReader<R> {
    pub fn new(inner: R, start: u64, end: Option<u64>) -> std::io::Result<Self> {
        let mut reader = Self {
            inner,
            start,
            end,
            pos: 0,
        };
        reader.inner.seek(std::io::SeekFrom::Start(start))?;
        Ok(reader)
    }
}

impl<R: Read + Seek> Read for RangeReader<R> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        if let Some(end) = self.end {
            let remaining = end.saturating_sub(self.start + self.pos);
            if remaining == 0 {
                return Ok(0);
            }
            let to_read = std::cmp::min(buf.len(), remaining as usize);
            let n = self.inner.read(&mut buf[..to_read])?;
            self.pos += n as u64;
            Ok(n)
        } else {
            let n = self.inner.read(buf)?;
            self.pos += n as u64;
            Ok(n)
        }
    }
}

impl<R: Read + Seek> Seek for RangeReader<R> {
    fn seek(&mut self, pos: std::io::SeekFrom) -> std::io::Result<u64> {
        let new_pos = match pos {
            std::io::SeekFrom::Start(offset) => {
                self.inner
                    .seek(std::io::SeekFrom::Start(self.start + offset))?;
                offset
            }
            std::io::SeekFrom::End(offset) => {
                let end = self.end.unwrap_or_else(|| {
                    let cur = self.inner.stream_position().unwrap_or(0);
                    let len = self.inner.seek(std::io::SeekFrom::End(0)).unwrap_or(0);
                    self.inner.seek(std::io::SeekFrom::Start(cur)).unwrap_or(0);
                    len
                });
                let target = end as i64 + offset;
                if target < self.start as i64 {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidInput,
                        "seek before range start",
                    ));
                }
                self.inner.seek(std::io::SeekFrom::Start(target as u64))?;
                target as u64 - self.start
            }
            std::io::SeekFrom::Current(offset) => {
                let target = self.pos as i64 + offset;
                if target < 0 {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidInput,
                        "seek before range start",
                    ));
                }
                self.inner
                    .seek(std::io::SeekFrom::Start(self.start + target as u64))?;
                target as u64
            }
        };
        self.pos = new_pos;
        Ok(new_pos)
    }
}

/// Peek at the first bytes of a reader to detect format
pub fn peek_format<R: Read + Seek>(reader: &mut R) -> std::io::Result<Format> {
    let mut header = [0u8; 512];
    let pos = reader.stream_position()?;
    let n = reader.read(&mut header)?;
    reader.seek(std::io::SeekFrom::Start(pos))?;

    if n < 4 {
        return Ok(Format::Unsupported);
    }

    // ZIP / Office formats
    if header[..4] == [0x50, 0x4B, 0x03, 0x04]
        || header[..4] == [0x50, 0x4B, 0x05, 0x06]
        || header[..4] == [0x50, 0x4B, 0x07, 0x08]
    {
        return Ok(Format::ArchiveZip);
    }

    // 7z
    if n >= 6 && header[..6] == [0x37, 0x7A, 0xBC, 0xAF, 0x27, 0x1C] {
        return Ok(Format::Archive7z);
    }

    // RAR
    if n >= 7 && header[..7] == [0x52, 0x61, 0x72, 0x21, 0x1A, 0x07, 0x00] {
        return Ok(Format::ArchiveRar);
    }

    // gzip
    if header[..2] == [0x1F, 0x8B] {
        return Ok(Format::ArchiveTarGz);
    }

    // bzip2
    if header[..3] == [0x42, 0x5A, 0x68] {
        return Ok(Format::ArchiveTarBz2);
    }

    // xz/lzma
    if n >= 6 && header[..6] == [0xFD, 0x37, 0x7A, 0x58, 0x5A, 0x00] {
        return Ok(Format::ArchiveTarXz);
    }

    // zstd
    if n >= 4 && header[..4] == [0x28, 0xB5, 0x2F, 0xFD] {
        return Ok(Format::ArchiveTarZstd);
    }

    // lz4
    if n >= 4 && header[..4] == [0x04, 0x22, 0x4D, 0x18] {
        return Ok(Format::ArchiveTarLz4);
    }

    // tar (ustar)
    if n >= 265 && &header[257..263] == b"ustar\0" {
        return Ok(Format::ArchiveTar);
    }

    Ok(Format::Unsupported)
}

/// Parse an archive from a streaming reader
pub fn parse_stream<R: Read + Seek + Send>(
    mut reader: R,
    filename: &str,
) -> Result<ManifestType, Error> {
    let format = peek_format(&mut reader)?;
    let _ext = filename.split('.').next_back().unwrap_or("").to_lowercase();

    let manifest = match format {
        Format::ArchiveZip => {
            #[cfg(feature = "zip")]
            {
                zip::list_zip(reader)?
            }
            #[cfg(not(feature = "zip"))]
            {
                return Err(Error::Parse("ZIP support not enabled".into()));
            }
        }
        Format::ArchiveTar => {
            #[cfg(feature = "tar")]
            {
                tar::list_tar(reader, false)?
            }
            #[cfg(not(feature = "tar"))]
            {
                return Err(Error::Parse("TAR support not enabled".into()));
            }
        }
        Format::ArchiveTarGz => {
            #[cfg(feature = "tar-gz")]
            {
                let mut entries = gzip::list_gz(reader)?;
                nested::detect_nested_archives(&mut entries);
                ArchiveManifest::new(entries, "tar.gz".to_string())
            }
            #[cfg(not(feature = "tar-gz"))]
            {
                return Err(Error::Parse("tar.gz support not enabled".into()));
            }
        }
        Format::ArchiveTarBz2 => {
            #[cfg(feature = "tar-bz2")]
            {
                let mut entries = bzip2::list_bz2(reader)?;
                nested::detect_nested_archives(&mut entries);
                ArchiveManifest::new(entries, "tar.bz2".to_string())
            }
            #[cfg(not(feature = "tar-bz2"))]
            {
                return Err(Error::Parse("tar.bz2 support not enabled".into()));
            }
        }
        Format::ArchiveTarXz => {
            #[cfg(feature = "tar-xz")]
            {
                let mut entries = xz::list_xz(reader)?;
                nested::detect_nested_archives(&mut entries);
                ArchiveManifest::new(entries, "tar.xz".to_string())
            }
            #[cfg(not(feature = "tar-xz"))]
            {
                return Err(Error::Parse("tar.xz support not enabled".into()));
            }
        }
        Format::ArchiveTarZstd => {
            #[cfg(feature = "tar-zstd")]
            {
                let mut entries = zstd::list_zstd(reader)?;
                nested::detect_nested_archives(&mut entries);
                ArchiveManifest::new(entries, "tar.zst".to_string())
            }
            #[cfg(not(feature = "tar-zstd"))]
            {
                return Err(Error::Parse("tar.zst support not enabled".into()));
            }
        }
        Format::ArchiveTarLz4 => {
            #[cfg(feature = "tar-lz4")]
            {
                let mut entries = lz4::list_lz4(reader)?;
                nested::detect_nested_archives(&mut entries);
                ArchiveManifest::new(entries, "tar.lz4".to_string())
            }
            #[cfg(not(feature = "tar-lz4"))]
            {
                return Err(Error::Parse("tar.lz4 support not enabled".into()));
            }
        }
        Format::ArchiveTarLzma => {
            #[cfg(feature = "tar-lzma")]
            {
                let mut entries = lzma::list_lzma(reader)?;
                nested::detect_nested_archives(&mut entries);
                ArchiveManifest::new(entries, "tar.lzma".to_string())
            }
            #[cfg(not(feature = "tar-lzma"))]
            {
                return Err(Error::Parse("tar.lzma support not enabled".into()));
            }
        }
        Format::Archive7z => {
            #[cfg(feature = "sevenz")]
            {
                sevenz::list_7z(reader)?
            }
            #[cfg(not(feature = "sevenz"))]
            {
                return Err(Error::Parse("7z support not enabled".into()));
            }
        }
        Format::ArchiveRar => {
            #[cfg(feature = "rar")]
            {
                rar::list_rar(reader)?
            }
            #[cfg(not(feature = "rar"))]
            {
                return Err(Error::Parse("RAR support not enabled".into()));
            }
        }
        _ => {
            return Err(Error::UnsupportedFormat(format));
        }
    };

    Ok(manifest)
}

/// Parse archive from bytes (for in-memory / small files)
pub fn parse(bytes: &[u8], format: Format, name: &str) -> Result<Document, Error> {
    let cursor = Cursor::new(bytes);
    let manifest = parse_stream(cursor, name)?;

    let entries: Vec<ArchiveEntry> = manifest
        .entries
        .into_iter()
        .map(|e| ArchiveEntry {
            name: e.name,
            size: e.size,
            compressed_size: e.compressed_size,
            is_dir: e.is_dir,
        })
        .collect();

    Ok(Document::Archive {
        entries,
        format,
        byte_len: bytes.len(),
    })
}

/// Extract a single entry from an archive stream
pub fn extract_entry_stream<R: Read + Seek + Send>(
    mut reader: R,
    _filename: &str,
    entry_name: &str,
) -> Result<Vec<u8>, Error> {
    let format = peek_format(&mut reader)?;

    match format {
        Format::ArchiveZip => {
            #[cfg(feature = "zip")]
            {
                zip::extract_entry(reader, entry_name)
            }
            #[cfg(not(feature = "zip"))]
            {
                Err(Error::Parse("ZIP support not enabled".into()))
            }
        }
        Format::ArchiveTar => {
            #[cfg(feature = "tar")]
            {
                tar::extract_entry(reader, entry_name, false)
            }
            #[cfg(not(feature = "tar"))]
            {
                Err(Error::Parse("TAR support not enabled".into()))
            }
        }
        Format::ArchiveTarGz => {
            #[cfg(feature = "tar-gz")]
            {
                tar::extract_entry(gzip::GzDecoderWrapper::new(reader), entry_name, true)
            }
            #[cfg(not(feature = "tar-gz"))]
            {
                Err(Error::Parse("tar.gz support not enabled".into()))
            }
        }
        Format::Archive7z => {
            #[cfg(feature = "sevenz")]
            {
                sevenz::extract_entry(reader, entry_name)
            }
            #[cfg(not(feature = "sevenz"))]
            {
                Err(Error::Parse("7z support not enabled".into()))
            }
        }
        Format::ArchiveRar => {
            #[cfg(feature = "rar")]
            {
                rar::extract_entry(reader, entry_name)
            }
            #[cfg(not(feature = "rar"))]
            {
                Err(Error::Parse("RAR support not enabled".into()))
            }
        }
        _ => Err(Error::UnsupportedFormat(format)),
    }
}

/// Extract all entries from an archive stream
pub fn extract_all_stream<R: Read + Seek + Send>(
    mut reader: R,
    _filename: &str,
) -> Result<Vec<(String, Vec<u8>)>, Error> {
    let format = peek_format(&mut reader)?;

    match format {
        Format::ArchiveZip => {
            #[cfg(feature = "zip")]
            {
                zip::extract_all(reader)
            }
            #[cfg(not(feature = "zip"))]
            {
                Err(Error::Parse("ZIP support not enabled".into()))
            }
        }
        Format::ArchiveTar => {
            #[cfg(feature = "tar")]
            {
                tar::extract_all(reader, false)
            }
            #[cfg(not(feature = "tar"))]
            {
                Err(Error::Parse("TAR support not enabled".into()))
            }
        }
        Format::ArchiveTarGz => {
            #[cfg(feature = "tar-gz")]
            {
                tar::extract_all(gzip::GzDecoderWrapper::new(reader), true)
            }
            #[cfg(not(feature = "tar-gz"))]
            {
                Err(Error::Parse("tar.gz support not enabled".into()))
            }
        }
        Format::Archive7z => {
            #[cfg(feature = "sevenz")]
            {
                sevenz::extract_all(reader)
            }
            #[cfg(not(feature = "sevenz"))]
            {
                Err(Error::Parse("7z support not enabled".into()))
            }
        }
        Format::ArchiveRar => {
            #[cfg(feature = "rar")]
            {
                rar::extract_all(reader)
            }
            #[cfg(not(feature = "rar"))]
            {
                Err(Error::Parse("RAR support not enabled".into()))
            }
        }
        _ => Err(Error::UnsupportedFormat(format)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn test_peek_zip() {
        let zip_data = vec![0x50, 0x4B, 0x03, 0x04];
        let mut cursor = Cursor::new(zip_data);
        assert_eq!(peek_format(&mut cursor).unwrap(), Format::ArchiveZip);
    }

    #[test]
    fn test_peek_7z() {
        let sevenz_data = vec![0x37, 0x7A, 0xBC, 0xAF, 0x27, 0x1C];
        let mut cursor = Cursor::new(sevenz_data);
        assert_eq!(peek_format(&mut cursor).unwrap(), Format::Archive7z);
    }

    #[test]
    fn test_peek_rar() {
        let rar_data = vec![0x52, 0x61, 0x72, 0x21, 0x1A, 0x07, 0x00];
        let mut cursor = Cursor::new(rar_data);
        assert_eq!(peek_format(&mut cursor).unwrap(), Format::ArchiveRar);
    }

    #[test]
    fn test_peek_gzip() {
        let gz_data = vec![0x1F, 0x8B, 0x08, 0x00]; // minimum gzip header
        let mut cursor = Cursor::new(gz_data);
        assert_eq!(peek_format(&mut cursor).unwrap(), Format::ArchiveTarGz);
    }

    #[test]
    fn test_peek_tar() {
        let mut tar_data = vec![0u8; 512];
        tar_data[257..263].copy_from_slice(b"ustar\0");
        let mut cursor = Cursor::new(tar_data);
        assert_eq!(peek_format(&mut cursor).unwrap(), Format::ArchiveTar);
    }
}
