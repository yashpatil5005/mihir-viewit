#[cfg(feature = "zstd")]
use std::io::{Read, Seek};
#[cfg(feature = "zstd")]
use zstd::stream::read::Decoder as ZstdDecoder;
use crate::{InternalArchiveEntry, Error};
#[cfg(feature = "zstd")]
use crate::tar::list_tar as list_tar_inner;

#[cfg(feature = "zstd")]
pub fn list_zstd<R: Read + Seek + Send>(mut reader: R) -> Result<Vec<InternalArchiveEntry>, Error> {
    // Check if it's a tar.zst
    let pos = reader.stream_position()?;
    let mut header = [0u8; 512];
    {
        let mut decoder = ZstdDecoder::new(&mut reader)
            .map_err(|e| Error::Parse(format!("zstd decoder: {}", e)))?;
        let n = decoder.read(&mut header)?;
        if n >= 268 && &header[257..265] == b"ustar\0" {
            reader.seek(std::io::SeekFrom::Start(pos))?;
            let manifest = list_tar_inner(reader, false)?; // zstd handles decompression
            return Ok(manifest.entries);
        }
    }
    reader.seek(std::io::SeekFrom::Start(pos))?;

    // Plain zst
    let mut decoder = ZstdDecoder::new(reader)
        .map_err(|e| Error::Parse(format!("zstd decoder: {}", e)))?;
    let mut data = Vec::new();
    decoder.read_to_end(&mut data)
        .map_err(|e| Error::Parse(format!("zstd decompress: {}", e)))?;

    let entry = InternalArchiveEntry::new(
        "archive-content".to_string(),
        data.len() as u64,
        0,
        false,
    );

    Ok(vec![entry])
}

#[cfg(not(feature = "zstd"))]
use std::io::{Read, Seek};

#[cfg(not(feature = "zstd"))]
pub fn list_zstd<R: Read + Seek + Send>(_reader: R) -> Result<Vec<InternalArchiveEntry>, Error> {
    Err(Error::Parse("zstd support not enabled".into()))
}