use crate::tar::list_tar as list_tar_inner;
use crate::{Error, InternalArchiveEntry};
use flate2::read::GzDecoder as Flate2GzDecoder;
use std::io::{Read, Seek};

pub fn list_gz<R: Read + Seek + Send>(mut reader: R) -> Result<Vec<InternalArchiveEntry>, Error> {
    // First check if it's a tar.gz or plain gz
    let pos = reader.stream_position()?;
    let mut header = [0u8; 512];
    {
        let mut decoder = Flate2GzDecoder::new(&mut reader);
        let n = decoder.read(&mut header)?;
        if n >= 268 && &header[257..265] == b"ustar\0" {
            // It's a tar.gz - delegate to tar module
            reader.seek(std::io::SeekFrom::Start(pos))?;
            let manifest = list_tar_inner(reader, true)?;
            return Ok(manifest.entries);
        }
    }
    reader.seek(std::io::SeekFrom::Start(pos))?;

    // Plain gz - single compressed file
    let mut decoder = Flate2GzDecoder::new(reader);
    let mut data = Vec::new();
    decoder
        .read_to_end(&mut data)
        .map_err(|e| Error::Parse(format!("gz decompress: {}", e)))?;

    // We can't easily get the original filename from gz, so use a default
    let entry = InternalArchiveEntry::new(
        "archive-content".to_string(),
        data.len() as u64,
        0, // compressed size unknown without reading full stream
        false,
    );

    Ok(vec![entry])
}

pub struct GzDecoderWrapper<R: Read> {
    inner: Flate2GzDecoder<R>,
}

impl<R: Read> GzDecoderWrapper<R> {
    pub fn new(reader: R) -> Self {
        Self {
            inner: Flate2GzDecoder::new(reader),
        }
    }
}

impl<R: Read> Read for GzDecoderWrapper<R> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.inner.read(buf)
    }
}

impl<R: Read + Seek> Seek for GzDecoderWrapper<R> {
    fn seek(&mut self, pos: std::io::SeekFrom) -> std::io::Result<u64> {
        self.inner.get_mut().seek(pos)
    }
}
