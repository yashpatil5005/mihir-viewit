//! Font metadata extraction for TTF/OTF/WOFF/TTC via `ttf-parser`.
//!
//! Coverage — every format is actually parsed (no dangling entries):
//! - `.ttf`, `.otf`: sfnt (TrueType/PostScript-CFF outlines) parsed directly.
//! - `.ttc`: TrueType collection — the first face is parsed.
//! - `.woff`: Web Open Font Format — table blob compressed with zlib; we
//!   inflate and reconstruct an in-memory sfnt before handing it to ttf-parser.
//!
//! Formats we deliberately to *not* advertise (no parser is available and we
//! must not claim support): `.woff2` (needs glyf/loca transform reconstruction),
//! `.pfb` / `.cff` (PostScript Type 1 / standalone CFF), `.dfont` (macOS data
//! fork), `.sfd` (FontForge source), `.ps` (PostScript program). Those are routed
//! to the external/open-with path rather than a broken preview.

use base64::{engine::general_purpose, Engine as _};
use viewit_core_types::{Document, Error, Format};

pub fn parse(bytes: &[u8], format: Format, _name: &str) -> Result<Document, Error> {
    let sfnt: Vec<u8> = if bytes.len() >= 4 && &bytes[..4] == b"wOFF" {
        sfnt_from_woff(bytes)?
    } else {
        bytes.to_vec()
    };

    let face = ttf_parser::Face::parse(&sfnt, 0)
        .map_err(|e| Error::Parse(format!("font parse: {:?}", e)))?;

    // Get family name from name table (name ID 1 = Font Family).
    let mut family_name = String::new();
    for name in face.names().into_iter() {
        if name.name_id == ttf_parser::name_id::FAMILY {
            if name.is_unicode() {
                if let Some(s) = name.to_string() {
                    family_name = s;
                    break;
                }
            }
        }
    }
    if family_name.is_empty() {
        for name in face.names().into_iter() {
            if name.name_id == ttf_parser::name_id::FAMILY {
                if let Some(s) = name.to_string() {
                    family_name = s;
                    break;
                }
            }
        }
    }

    // Get weight from OS/2 table (default to 400 = normal).
    let weight = face.weight().to_number();

    // Check italic style.
    let is_italic = face.is_italic();

    // Encode font data as base64 for @font-face.
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

fn align4(n: usize) -> usize {
    (n + 3) & !3
}

/// Reconstruct an in-memory sfnt from a WOFF file (zlib-compressed tables).
/// WOFF stores the original sfnt flavor in bytes 4..8 and each table blob
/// deflated with zlib; when `compLength == origLength` the table is stored
/// verbatim (uncompressed). We rebuild a canonical sfnt table directory so
/// `ttf-parser` can read it.
fn sfnt_from_woff(woff: &[u8]) -> Result<Vec<u8>, Error> {
    if woff.len() < 44 {
        return Err(Error::Parse("woff too short".into()));
    }
    let flavor = woff[4..8].to_vec();
    let num_tables = u16::from_be_bytes([woff[12], woff[13]]) as usize;

    let mut entries: Vec<(Vec<u8>, Vec<u8>)> = Vec::with_capacity(num_tables);
    for i in 0..num_tables {
        let header_len = 44usize
            .checked_add(
                i.checked_mul(20)
                    .ok_or_else(|| Error::Parse("woff dir overflow".into()))?,
            )
            .ok_or_else(|| Error::Parse("woff dir overflow".into()))?;
        if header_len + 20 > woff.len() {
            return Err(Error::Parse("woff directory truncated".into()));
        }
        let rec = &woff[header_len..header_len + 20];
        let tag = rec[0..4].to_vec();
        let data_off = u32::from_be_bytes([rec[4], rec[5], rec[6], rec[7]]) as usize;
        let comp_len = u32::from_be_bytes([rec[8], rec[9], rec[10], rec[11]]) as usize;
        let orig_len = u32::from_be_bytes([rec[12], rec[13], rec[14], rec[15]]) as usize;

        let raw = woff
            .get(data_off..data_off.saturating_add(comp_len))
            .ok_or_else(|| Error::Parse("woff table data out of range".into()))?;

        let out: Vec<u8> = if comp_len == 0 || comp_len == orig_len {
            raw.to_vec()
        } else {
            use std::io::Read;
            let mut inf = flate2::read::ZlibDecoder::new(raw);
            let mut o = Vec::with_capacity(orig_len);
            inf.read_to_end(&mut o)
                .map_err(|e| Error::Parse(format!("woff zlib: {e}")))?;
            o
        };
        entries.push((tag, out));
    }

    entries.sort_by_key(|(t, _)| t.clone());

    let n = entries.len() as u16;
    let max_pow2 = 1u16 << (15 - (n.leading_zeros() as u16));
    let entry_selector = (f64::from(n)).log2().floor() as u16;
    let search_range = max_pow2 * 16;
    let range_shift = n * 16 - search_range;

    let dir_end = 12usize
        .checked_add(
            entries
                .len()
                .checked_mul(16)
                .ok_or_else(|| Error::Parse("sfnt overflow".into()))?,
        )
        .ok_or_else(|| Error::Parse("sfnt overflow".into()))?;
    let mut offset = dir_end;
    let mut sfnt = Vec::new();
    sfnt.extend_from_slice(&flavor);
    sfnt.extend_from_slice(&u16::to_be_bytes(n));
    sfnt.extend_from_slice(&u16::to_be_bytes(search_range));
    sfnt.extend_from_slice(&u16::to_be_bytes(entry_selector));
    sfnt.extend_from_slice(&u16::to_be_bytes(range_shift));
    for (tag, data) in &entries {
        sfnt.extend_from_slice(tag);
        sfnt.extend_from_slice(&[0u8; 4]);
        sfnt.extend_from_slice(&(offset as u32).to_be_bytes());
        sfnt.extend_from_slice(&(data.len() as u32).to_be_bytes());
        offset = align4(offset.saturating_add(data.len()));
    }
    for (_, data) in &entries {
        sfnt.extend_from_slice(data);
        let pad = align4(data.len()).saturating_sub(data.len());
        for _ in 0..pad {
            sfnt.push(0);
        }
    }
    Ok(sfnt)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_woff2_honestly() {
        // A fake WOFF2 wrapper must not be silently treated as a supported font.
        let woff2 = b"wOF2";
        assert!(parse(woff2, Format::Font, "x.woff2").is_err());
        // And raw PostScript/dfont/sfd data yields a parse error, never a false OK.
        let pfb = b"%!PS-AdobeFont";
        assert!(parse(pfb, Format::Font, "x.pfb").is_err());
        let sfd = b"SplineFontDB: 3.0";
        assert!(parse(sfd, Format::Font, "x.sfd").is_err());
    }
}
