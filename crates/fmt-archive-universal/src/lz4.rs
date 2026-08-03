#[cfg(feature = "lz4")]
use std::io::{Read, Seek};
#[cfg(feature = "lz4")]
use lz4::Decoder as Lz4Decoder;
use crate::{InternalArchiveEntry, Error};
#[cfg(feature = "lz4")]
use crate::tar::list_tar as list_tar_inner;

#[cfg(feature = "lz4")]
pub fn list_lz4<R: Read + Seek + Send>(mut reader: R) -> Result<Vec<InternalArchiveEntry>, Error> {
    // Check if it's a tar.lz4
    let pos = reader.stream_position()?;
    let mut header = [0u8; 512];
    {
        let mut decoder = Lz4Decoder::new(&mut reader)
            .map_err(|e| Error::Parse(format!("lz4 decoder: {}", e)))?;
        let n = decoder.read(&mut header)?;
        if n >= 268 && &header[257..265] == b"ustar\0" {
            reader.seek(std::io::SeekFrom::Start(pos))?;
            let manifest = list_tar_inner(reader, false)?; // lz4 handles decompression
            return Ok(manifest.entries);
        }
    }
    reader.seek(std::io::SeekFrom::Start(pos))?;

    // Plain lz4
    let mut decoder = Lz4Decoder::new(reader)
        .map_err(|e| Error::Parse(format!("lz4 decoder: {}", e)))?;
    let mut data = Vec::new();
    std::io::copy(&mut decoder, &mut data)
        .map_err(|e| Error::Parse(format!("lz4 decompress: {}", e)))?;

    let (_, result) = decoder.finish();
    result.map_err(|e| Error::Parse(format!("lz4 finish: {}", e)))?;

    let entry = InternalArchiveEntry::new(
        "archive-content".to_string(),
        data.len() as u64,
        0,
        false,
    );

    Ok(vec![entry])
}

#[cfg(not(feature = "lz4"))]
use std::io::{Read, Seek};

#[cfg(not(feature = "lz4"))]
pub fn list_lz4<R: Read + Seek + Send>(_reader: R) -> Result<Vec<InternalArchiveEntry>, Error> {
    Err(Error::Parse("lz4 support not enabled".into()))
}