#[cfg(feature = "bzip2")]
use std::io::{Read, Seek};
#[cfg(feature = "bzip2")]
use bzip2::read::BzDecoder;
use crate::{InternalArchiveEntry, Error};
#[cfg(feature = "bzip2")]
use crate::tar::list_tar as list_tar_inner;

#[cfg(feature = "bzip2")]
pub fn list_bz2<R: Read + Seek + Send>(mut reader: R) -> Result<Vec<InternalArchiveEntry>, Error> {
    // Check if it's a tar.bz2
    let pos = reader.stream_position()?;
    let mut header = [0u8; 512];
    {
        let mut decoder = BzDecoder::new(&mut reader);
        let n = decoder.read(&mut header)?;
        if n >= 268 && &header[257..265] == b"ustar\0" {
            reader.seek(std::io::SeekFrom::Start(pos))?;
            let manifest = list_tar_inner(reader, false)?; // bzip2 handles decompression
            return Ok(manifest.entries);
        }
    }
    reader.seek(std::io::SeekFrom::Start(pos))?;

    // Plain bz2
    let mut decoder = BzDecoder::new(reader);
    let mut data = Vec::new();
    decoder.read_to_end(&mut data)
        .map_err(|e| Error::Parse(format!("bz2 decompress: {}", e)))?;

    let entry = InternalArchiveEntry::new(
        "archive-content".to_string(),
        data.len() as u64,
        0,
        false,
    );

    Ok(vec![entry])
}

#[cfg(not(feature = "bzip2"))]
use std::io::{Read, Seek};

#[cfg(not(feature = "bzip2"))]
pub fn list_bz2<R: Read + Seek + Send>(_reader: R) -> Result<Vec<InternalArchiveEntry>, Error> {
    Err(Error::Parse("bzip2 support not enabled".into()))
}