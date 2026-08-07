//! Legacy binary Office (.doc / .ppt) parser.
//!
//! Best-effort text extraction via CFB compound-storage walk.
//! Marked partial-preview — layout/formatting not preserved.

use std::io::Cursor;
use viewit_core_types::{Document, Error, Format, PptxSlide};

pub fn parse_legacy_binary(bytes: &[u8], format: Format) -> Result<Document, Error> {
    let cursor = Cursor::new(bytes.to_vec());
    let mut cfb =
        cfb::CompoundFile::open(cursor).map_err(|e| Error::Parse(format!("cfb open: {}", e)))?;

    let streams: Vec<&str> = match format {
        Format::Doc => vec!["WordDocument", "1Table", "0Table"],
        Format::Ppt => vec!["PowerPoint Document", "Current User"],
        _ => return Err(Error::UnsupportedFormat(format)),
    };

    let mut text = String::new();
    for stream_name in &streams {
        if cfb.is_stream(stream_name) {
            use std::io::Read;
            if let Ok(mut stream) = cfb.open_stream(stream_name) {
                let mut buf = Vec::new();
                if stream.read_to_end(&mut buf).is_ok() {
                    let extracted = extract_utf16_le(&buf);
                    if !extracted.trim().is_empty() {
                        text.push_str(&extracted);
                        text.push('\n');
                    }
                    let ascii = extract_ascii(&buf);
                    if !ascii.trim().is_empty() {
                        text.push_str(&ascii);
                        text.push('\n');
                    }
                }
            }
        }
    }

    // Fallback: walk all streams
    if text.trim().is_empty() {
        let paths: Vec<_> = cfb
            .read_root_storage()
            .filter(|e| e.is_stream())
            .map(|e| e.path().to_path_buf())
            .collect();
        for path in paths {
            if let Ok(mut s) = cfb.open_stream(&path) {
                use std::io::Read;
                let mut b = Vec::new();
                if s.read_to_end(&mut b).is_ok() {
                    text.push_str(&extract_ascii(&b));
                }
            }
        }
    }

    if format == Format::Ppt {
        let slides = legacy_ppt_slides(&text);
        return Ok(Document::Pptx {
            slide_count: slides.len(),
            slides,
            byte_len: bytes.len(),
            asset_path: String::new(),
            stream_url: None,
        });
    }

    let label = match format {
        Format::Doc => "Word .doc",
        _ => "legacy",
    };
    Ok(Document::Text {
        content: format!(
            "[Partial preview — {} legacy binary, layout/formatting not preserved]\n\n{}",
            label,
            text.trim()
        ),
        encoding: "utf-8".into(),
        byte_len: bytes.len(),
        truncated: text.len() > 256 * 1024,
        stream_url: None,
    })
}

fn legacy_ppt_slides(text: &str) -> Vec<PptxSlide> {
    let cleaned: Vec<String> = text
        .lines()
        .map(str::trim)
        .filter(|line| line.len() >= 3)
        .filter(|line| {
            !line
                .chars()
                .all(|c| c.is_ascii_punctuation() || c.is_ascii_digit())
        })
        .map(str::to_string)
        .collect();

    if cleaned.is_empty() {
        return vec![PptxSlide {
            title: "Legacy PowerPoint preview".into(),
            body: "No extractable slide text found. Legacy binary PowerPoint layout is not decoded in the lightweight viewer.".into(),
            elements: Vec::new(),
        }];
    }

    let mut slides = Vec::new();
    let chunk_size = 12;
    for (idx, chunk) in cleaned.chunks(chunk_size).enumerate() {
        let title = chunk
            .first()
            .cloned()
            .filter(|s| s.len() <= 120)
            .unwrap_or_else(|| format!("Legacy PowerPoint slide {}", idx + 1));
        let body = chunk.iter().skip(1).cloned().collect::<Vec<_>>().join("\n");
        slides.push(PptxSlide {
            title,
            body,
            elements: Vec::new(),
        });
    }
    slides
}

fn extract_utf16_le(bytes: &[u8]) -> String {
    let mut out = String::new();
    let mut cur = String::new();
    let mut i = 0;
    while i + 1 < bytes.len() {
        let lo = bytes[i] as u16;
        let hi = bytes[i + 1] as u16;
        let cp = lo | (hi << 8);
        if (0x20..=0x7E).contains(&cp) {
            cur.push(cp as u8 as char);
        } else if cp == 0x0A || cp == 0x0D {
            if !cur.is_empty() {
                cur.push('\n');
                out.push_str(&cur);
                cur.clear();
            }
        } else if !cur.is_empty() {
            if cur.len() >= 4 {
                out.push_str(&cur);
                out.push('\n');
            }
            cur.clear();
        }
        i += 2;
    }
    if cur.len() >= 4 {
        out.push_str(&cur);
    }
    out
}

fn extract_ascii(bytes: &[u8]) -> String {
    let mut out = String::new();
    let mut cur: Vec<u8> = Vec::new();
    for &b in bytes {
        if (0x20..=0x7E).contains(&b) {
            cur.push(b);
        } else if b == 0x0A || b == 0x0D {
            if cur.len() >= 4 {
                out.push_str(&String::from_utf8_lossy(&cur));
                out.push('\n');
            }
            cur.clear();
        } else if !cur.is_empty() {
            if cur.len() >= 4 {
                out.push_str(&String::from_utf8_lossy(&cur));
                out.push(' ');
            }
            cur.clear();
        }
    }
    if cur.len() >= 4 {
        out.push_str(&String::from_utf8_lossy(&cur));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_utf16_le_basic() {
        let input = b"H\x00e\x00l\x00l\x00o\x00";
        let result = extract_utf16_le(input);
        assert_eq!(result, "Hello");
    }

    #[test]
    fn extract_ascii_basic() {
        let input = b"Hello\x00World";
        let result = extract_ascii(input);
        assert!(result.contains("Hello"));
    }
}
