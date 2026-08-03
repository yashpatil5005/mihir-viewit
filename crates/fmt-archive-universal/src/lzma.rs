#[cfg(feature = "lzma")]
use std::io::{Read, Seek};
#[cfg(feature = "lzma")]
use xz2::read::XzDecoder; // xz2 handles both .xz and .lzma
use crate::{InternalArchiveEntry, Error};
#[cfg(feature = "lzma")]
use crate::tar::list_tar as list_tar_inner;

#[cfg(feature = "lzma")]
pub fn list_lzma<R: Read + Seek + Send>(mut reader: R) -> Result<Vec<InternalArchiveEntry>, Error> {
    // Check if it's a tar.lzma
    let pos = reader.stream_position()?;
    let mut header = [0u8; 512];
    {
        let mut decoder = XzDecoder::new(&mut reader);
        let n = decoder.read(&mut header)?;
        if n >= 268 && &header[257..265] == b"ustar\0" {
            reader.seek(std::io::SeekFrom::Start(pos))?;
            let manifest = list_tar_inner(reader, false)?;
            return Ok(manifest.entries);
        }
    }
    reader.seek(std::io::SeekFrom::Start(pos))?;

    // Plain lzma
    let mut decoder = XzDecoder::new(reader);
    let mut data = Vec::new();
    decoder.read_to_end(&mut data)
        .map_err(|e| Error::Parse(format!("lzma decompress: {}", e)))?;

    let entry = InternalArchiveEntry::new(
        "archive-content".to_string(),
        data.len() as u64,
        0,
        false,
    );

    Ok(vec![entry])
}

#[cfg(not(feature = "lzma"))]
use std::io::{Read, Seek};

#[cfg(not(feature = "lzma"))]
pub fn list_lzma<R: Read + Seek + Send>(_reader: R) -> Result<Vec<InternalArchiveEntry>, Error> {
    Err(Error::Parse("lzma support not enabled".into()))
}