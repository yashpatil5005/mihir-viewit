use crate::{ArchiveManifest, Error};
use std::io::{Read, Seek};

#[cfg(feature = "rar")]
pub fn list_rar<R: Read + Seek>(mut reader: R) -> Result<ArchiveManifest, Error> {
    let pos = reader.stream_position()?;
    let len = reader.seek(std::io::SeekFrom::End(0))?;
    reader.seek(std::io::SeekFrom::Start(pos))?;

    // unrar requires a file path, so we need to read into memory for now
    // TODO: implement streaming RAR parsing when unrar supports Read+Seek
    let mut data = Vec::new();
    std::io::copy(&mut reader, &mut data).map_err(|e| Error::Parse(format!("rar read: {}", e)))?;

    // For WASM, we can't use the native unrar library easily
    // This is a placeholder - RAR support in WASM would need unrar.js fallback
    return Err(Error::Parse(
        "RAR listing not yet implemented for streaming".into(),
    ));
}

#[cfg(not(feature = "rar"))]
pub fn list_rar<R: Read + Seek>(_reader: R) -> Result<ArchiveManifest, Error> {
    Err(Error::Parse("RAR support not enabled".into()))
}

#[cfg(feature = "rar")]
pub fn extract_entry<R: Read + Seek>(mut reader: R, entry_name: &str) -> Result<Vec<u8>, Error> {
    let mut data = Vec::new();
    std::io::copy(&mut reader, &mut data).map_err(|e| Error::Parse(format!("rar read: {}", e)))?;

    Err(Error::Parse(
        "RAR extraction not yet implemented for streaming".into(),
    ))
}

#[cfg(not(feature = "rar"))]
pub fn extract_entry<R: Read + Seek>(_reader: R, _entry_name: &str) -> Result<Vec<u8>, Error> {
    Err(Error::Parse("RAR support not enabled".into()))
}

#[cfg(feature = "rar")]
pub fn extract_all<R: Read + Seek>(mut reader: R) -> Result<Vec<(String, Vec<u8>)>, Error> {
    let mut data = Vec::new();
    std::io::copy(&mut reader, &mut data).map_err(|e| Error::Parse(format!("rar read: {}", e)))?;

    Err(Error::Parse(
        "RAR extraction not yet implemented for streaming".into(),
    ))
}

#[cfg(not(feature = "rar"))]
pub fn extract_all<R: Read + Seek>(_reader: R) -> Result<Vec<(String, Vec<u8>)>, Error> {
    Err(Error::Parse("RAR support not enabled".into()))
}
