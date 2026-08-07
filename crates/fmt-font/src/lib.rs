//! Font metadata extraction for TTF/OTF/WOFF via `ttf-parser`.

use base64::{engine::general_purpose, Engine as _};
use viewit_core_types::{Document, Error, Format};

pub fn parse(bytes: &[u8], format: Format, _name: &str) -> Result<Document, Error> {
    let face = ttf_parser::Face::parse(bytes, 0)
        .map_err(|e| Error::Parse(format!("font parse: {:?}", e)))?;

    // Get family name from name table (name ID 1 = Font Family)
    let mut family_name = String::new();
    for name in face.names().into_iter() {
        if name.name_id == ttf_parser::name_id::FAMILY {
            if let Some(s) = name.to_string() {
                family_name = s;
                break;
            }
        }
    }

    // Get weight from OS/2 table (default to 400 = normal)
    let weight = face.weight().to_number();

    // Check italic style
    let is_italic = face.is_italic();

    // Encode font data as base64 for @font-face
    let font_data = general_purpose::STANDARD.encode(bytes);

    Ok(Document::Font {
        family_name,
        weight,
        is_italic,
        format,
        byte_len: bytes.len(),
        font_data: Some(font_data),
    })
}
