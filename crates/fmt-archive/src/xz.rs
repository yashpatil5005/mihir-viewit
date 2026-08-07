#[cfg(feature = "xz")]
use crate::tar::list_tar as list_tar_inner;
use crate::{Error, InternalArchiveEntry};
#[cfg(feature = "xz")]
use std::io::{Read, Seek};
#[cfg(feature = "xz")]
use xz2::read::XzDecoder;

#[cfg(feature = "xz")]
pub fn list_xz<R: Read + Seek + Send>(mut reader: R) -> Result<Vec<InternalArchiveEntry>, Error> {
    // Check if it's a tar.xz
    let pos = reader.stream_position()?;
    let mut header = [0u8; 512];
    {
        let mut decoder = XzDecoder::new(&mut reader);
        let n = decoder.read(&mut header)?;
        if n >= 268 && &header[257..265] == b"ustar\0" {
            reader.seek(std::io::SeekFrom::Start(pos))?;
            let manifest = list_tar_inner(reader, false)?; // xz handles decompression
            return Ok(manifest.entries);
        }
    }
    reader.seek(std::io::SeekFrom::Start(pos))?;

    // Plain xz
    let mut decoder = XzDecoder::new(reader);
    let mut data = Vec::new();
    decoder
        .read_to_end(&mut data)
        .map_err(|e| Error::Parse(format!("xz decompress: {}", e)))?;

    let entry =
        InternalArchiveEntry::new("archive-content".to_string(), data.len() as u64, 0, false);

    Ok(vec![entry])
}

#[cfg(not(feature = "xz"))]
use std::io::{Read, Seek};

#[cfg(not(feature = "xz"))]
pub fn list_xz<R: Read + Seek + Send>(_reader: R) -> Result<Vec<InternalArchiveEntry>, Error> {
    Err(Error::Parse("xz support not enabled".into()))
}
