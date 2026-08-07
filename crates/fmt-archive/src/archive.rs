use crate::entry::ArchiveEntry;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArchiveManifest {
    pub entries: Vec<ArchiveEntry>,
    pub format: String,
    pub total_size: u64,
    pub total_compressed_size: u64,
}

impl ArchiveManifest {
    pub fn new(entries: Vec<ArchiveEntry>, format: String) -> Self {
        let total_size = entries.iter().map(|e| e.size).sum();
        let total_compressed_size = entries.iter().map(|e| e.compressed_size).sum();
        Self {
            entries,
            format,
            total_size,
            total_compressed_size,
        }
    }
}
