use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArchiveEntry {
    pub name: String,
    pub size: u64,
    pub compressed_size: u64,
    pub modified: Option<i64>,
    pub crc32: Option<u32>,
    pub method: Option<String>,
    pub is_dir: bool,
    pub nested: bool,
}

impl ArchiveEntry {
    pub fn new(
        name: String,
        size: u64,
        compressed_size: u64,
        is_dir: bool,
    ) -> Self {
        Self {
            name,
            size,
            compressed_size,
            modified: None,
            crc32: None,
            method: None,
            is_dir,
            nested: false,
        }
    }

    pub fn with_metadata(
        name: String,
        size: u64,
        compressed_size: u64,
        is_dir: bool,
        modified: Option<i64>,
        crc32: Option<u32>,
        method: Option<String>,
    ) -> Self {
        Self {
            name,
            size,
            compressed_size,
            modified,
            crc32,
            method,
            is_dir,
            nested: false,
        }
    }

    pub fn set_nested(&mut self, nested: bool) {
        self.nested = nested;
    }
}