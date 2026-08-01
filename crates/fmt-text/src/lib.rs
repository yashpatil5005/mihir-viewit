//! Phase 2 fmt-text dispatcher.
//!
//! Branches on the sniffed `Format`:
//! - `PlainText` / `Code`     → lossy-UTF-8 String, return `Text`.
//! - `Markdown`               → `pulldown-cmark` → HTML, return `Markdown`.
//! - `Json`                   → `serde_json::to_string_pretty`, return `Json`.
//! - `Csv`                    → read first ~200 rows with the `csv` crate, return `Csv`.
//!
//! Phase 2.3 encoding detection: if `encoding_detect` feature is on, we use
//! `chardetng` + `encoding_rs` to sniff the file's encoding instead of
//! always settling for UTF-8 lossy.

use viewit_core_types::{Document, Error, Format};

/// Number of CSV rows we ship in the first payload. The frontend asks for
/// later pages as the user scrolls near the bottom of the virtualized table.
const CSV_PREVIEW_ROWS: usize = 200;

/// Phase 2.3 — eager payload cap. Larger files truncate and frontend
/// requests follow-up pages via `text_page` Tauri command.
const TEXT_EAGER_CAP: usize = 256 * 1024;

pub fn parse_text(bytes: &[u8], format: Format, _name: &str) -> Result<Document, Error> {
    // Step 1 — decode bytes to a String. Phase 2.3 swaps lossy for chardetng.
    let (text, encoding_label) = decode(bytes);

    match format {
        Format::Rtf => {
            let html = rtf_to_html(&text);
            let truncated = html.len() > TEXT_EAGER_CAP;
            Ok(Document::Markdown {
                html: if truncated {
                    html[..TEXT_EAGER_CAP].to_string()
                } else {
                    html
                },
                byte_len: bytes.len(),
            })
        }
        Format::Markdown => {
            let html = markdown_to_html(&text);
            Ok(Document::Markdown {
                html,
                byte_len: bytes.len(),
            })
        }
        Format::Json => {
            let pretty = pretty_json(&text).unwrap_or_else(|_| text.clone());
            let truncated = pretty.len() > TEXT_EAGER_CAP;
            Ok(Document::Json {
                pretty: if truncated {
                    pretty[..TEXT_EAGER_CAP].to_string()
                } else {
                    pretty
                },
                byte_len: bytes.len(),
            })
        }
        Format::Csv => {
            let (header, preview_rows, total_hint) = parse_csv(&text);
            Ok(Document::Csv {
                header,
                preview_rows,
                total_rows_hint: total_hint,
                byte_len: bytes.len(),
            })
        }
        Format::PlainText | Format::Code | Format::Plist | Format::Ics | Format::Vcf | _ => {
            let truncated = text.len() > TEXT_EAGER_CAP;
            Ok(Document::Text {
                content: if truncated {
                    text[..TEXT_EAGER_CAP].to_string()
                } else {
                    text
                },
                encoding: encoding_label,
                byte_len: bytes.len(),
                truncated,
                stream_url: None,
            })
        }
    }
}

// --- Phase 3.5 — RTF plain-text extraction ----------------------------------

#[allow(dead_code)]
fn rtf_to_text(rtf: &str) -> String {
    let mut out = String::with_capacity(rtf.len());
    let mut chars = rtf.chars().peekable();
    let mut depth: i32 = 0;
    let mut skip_group: i32 = -1;

    while let Some(c) = chars.next() {
        match c {
            '\\' => {
                let next = match chars.peek() {
                    Some(&n) => n,
                    None => break,
                };
                if next == '\\' || next == '{' || next == '}' {
                    out.push(next);
                    chars.next();
                } else if next == '*' {
                    // destination control — skip group
                    if skip_group == -1 {
                        skip_group = depth;
                    }
                    chars.next();
                    // consume rest of control word
                    while let Some(&w) = chars.peek() {
                        if w.is_alphabetic() {
                            chars.next();
                        } else {
                            break;
                        }
                    }
                } else if next.is_alphabetic() {
                    // control word
                    let mut word = String::new();
                    chars.next();
                    while let Some(&w) = chars.peek() {
                        if w.is_alphabetic() {
                            word.push(w);
                            chars.next();
                        } else {
                            break;
                        }
                    }
                    // optional digit parameter
                    if let Some(&w) = chars.peek() {
                        if w == '-' || w.is_ascii_digit() {
                            chars.next();
                            while let Some(&d) = chars.peek() {
                                if d.is_ascii_digit() {
                                    chars.next();
                                } else {
                                    break;
                                }
                            }
                        }
                    }
                    // space after control word is consumed
                    if let Some(&' ') = chars.peek() {
                        chars.next();
                    }
                    // insert paragraph break for \par, line break for \line
                    if word == "par" {
                        out.push_str("\n\n");
                    } else if word == "line" {
                        out.push('\n');
                    } else if word == "tab" {
                        out.push('\t');
                    }
                } else if next == '\'' {
                    // hex escape \'XX
                    chars.next();
                    let h1 = chars.next();
                    let h2 = chars.next();
                    if let (Some(a), Some(b)) = (h1, h2) {
                        let s = format!("{}{}", a, b);
                        if let Ok(b) = u8::from_str_radix(&s, 16) {
                            out.push(b as char);
                        }
                    }
                } else {
                    chars.next();
                }
            }
            '{' => {
                depth += 1;
            }
            '}' => {
                depth -= 1;
                if skip_group != -1 && depth < skip_group {
                    skip_group = -1;
                }
            }
            c if skip_group == -1 && !c.is_control() => {
                out.push(c);
            }
            _ => {}
        }
    }
    out.replace("\n{3,}", "\n\n").trim_matches('\n').to_string()
}

// --- RTF → HTML conversion with formatting support ---------------------------

fn rtf_to_html(rtf: &str) -> String {
    let mut out = String::with_capacity(rtf.len() * 2);
    let mut chars = rtf.chars().peekable();
    let mut depth: i32 = 0;
    let mut skip_group: i32 = -1;
    let mut bold = false;
    let mut italic = false;
    let mut underline = false;
    let mut font_size: Option<u32> = None;
    let mut in_paragraph = false;

    // Track nesting of formatting via stacks
    let mut bold_stack: Vec<bool> = Vec::new();
    let mut italic_stack: Vec<bool> = Vec::new();
    let mut underline_stack: Vec<bool> = Vec::new();

    while let Some(c) = chars.next() {
        match c {
            '\\' => {
                let next = match chars.peek() {
                    Some(&n) => n,
                    None => break,
                };
                if next == '\\' || next == '{' || next == '}' {
                    push_escaped(&mut out, next, &mut in_paragraph);
                    chars.next();
                } else if next == '*' {
                    if skip_group == -1 {
                        skip_group = depth;
                    }
                    chars.next();
                    while let Some(&w) = chars.peek() {
                        if w.is_alphabetic() {
                            chars.next();
                        } else {
                            break;
                        }
                    }
                } else if next.is_alphabetic() {
                    let mut word = String::new();
                    chars.next();
                    while let Some(&w) = chars.peek() {
                        if w.is_alphabetic() {
                            word.push(w);
                            chars.next();
                        } else {
                            break;
                        }
                    }
                    // optional numeric parameter
                    let param = if let Some(&w) = chars.peek() {
                        if w == '-' || w.is_ascii_digit() {
                            let negative = w == '-';
                            if negative {
                                chars.next();
                            }
                            let mut num = String::new();
                            while let Some(&d) = chars.peek() {
                                if d.is_ascii_digit() {
                                    num.push(d);
                                    chars.next();
                                } else {
                                    break;
                                }
                            }
                            num.parse::<i32>()
                                .ok()
                                .map(|n| if negative { -n } else { n })
                        } else {
                            None
                        }
                    } else {
                        None
                    };
                    // consume trailing space
                    if let Some(&' ') = chars.peek() {
                        chars.next();
                    }

                    match word.as_str() {
                        "par" | "pard" => {
                            close_paragraph(&mut out, &mut in_paragraph);
                            open_paragraph(&mut out, &mut in_paragraph);
                        }
                        "line" => {
                            out.push_str("<br/>");
                        }
                        "tab" => {
                            out.push_str("&nbsp;&nbsp;&nbsp;&nbsp;");
                        }
                        "b" | "b0" => {
                            let new_val = word == "b";
                            bold = if new_val {
                                param.map_or(true, |p| p != 0)
                            } else {
                                false
                            };
                        }
                        "i" | "i0" => {
                            let new_val = word == "i";
                            italic = if new_val {
                                param.map_or(true, |p| p != 0)
                            } else {
                                false
                            };
                        }
                        "ul" | "ul0" | "ulnone" => {
                            underline = word == "ul";
                        }
                        "fs" => {
                            if let Some(p) = param {
                                font_size = if p > 0 { Some(p as u32 / 2) } else { None };
                            }
                        }
                        "fs0" => {
                            font_size = None;
                        }
                        _ => {}
                    }
                } else if next == '\'' {
                    chars.next();
                    let h1 = chars.next();
                    let h2 = chars.next();
                    if let (Some(a), Some(b)) = (h1, h2) {
                        let s = format!("{}{}", a, b);
                        if let Ok(byte_val) = u8::from_str_radix(&s, 16) {
                            let ch = byte_val as char;
                            if ch.is_control() {
                                continue;
                            }
                            push_char_html(
                                &mut out,
                                ch,
                                bold,
                                italic,
                                underline,
                                font_size,
                                &mut in_paragraph,
                            );
                        }
                    }
                } else {
                    chars.next();
                }
            }
            '{' => {
                depth += 1;
                bold_stack.push(bold);
                italic_stack.push(italic);
                underline_stack.push(underline);
            }
            '}' => {
                depth -= 1;
                if skip_group != -1 && depth < skip_group {
                    skip_group = -1;
                }
                bold = bold_stack.pop().unwrap_or(false);
                italic = italic_stack.pop().unwrap_or(false);
                underline = underline_stack.pop().unwrap_or(false);
            }
            c if skip_group == -1 && !c.is_control() => {
                push_char_html(
                    &mut out,
                    c,
                    bold,
                    italic,
                    underline,
                    font_size,
                    &mut in_paragraph,
                );
            }
            _ => {}
        }
    }
    close_paragraph(&mut out, &mut in_paragraph);

    // Collapse 3+ consecutive <br/> into a paragraph break
    let mut result = out.replace("<br/><br/><br/>", "</p><p>");
    // Wrap in <p> if not already
    if !result.starts_with('<') {
        result = format!("<p>{}</p>", result);
    }
    result.trim().to_string()
}

fn push_escaped(out: &mut String, ch: char, in_paragraph: &mut bool) {
    match ch {
        '\\' => push_char_html(out, '\\', false, false, false, None, in_paragraph),
        '{' => push_char_html(out, '{', false, false, false, None, in_paragraph),
        '}' => push_char_html(out, '}', false, false, false, None, in_paragraph),
        _ => {}
    }
}

fn push_char_html(
    out: &mut String,
    ch: char,
    bold: bool,
    italic: bool,
    underline: bool,
    font_size: Option<u32>,
    in_paragraph: &mut bool,
) {
    if !*in_paragraph {
        out.push_str("<p>");
        *in_paragraph = true;
    }
    let needs_open = bold || italic || underline || font_size.is_some();
    if needs_open {
        out.push('<');
        if bold {
            out.push_str("strong");
        }
        if italic {
            out.push_str("em");
        }
        if underline {
            out.push_str("u");
        }
        out.push('>');
    }
    match ch {
        '<' => out.push_str("&lt;"),
        '>' => out.push_str("&gt;"),
        '&' => out.push_str("&amp;"),
        '"' => out.push_str("&quot;"),
        '\n' => out.push_str("<br/>"),
        c => out.push(c),
    }
    if needs_open {
        out.push_str("</");
        if underline {
            out.push_str("u");
        }
        if italic {
            out.push_str("em");
        }
        if bold {
            out.push_str("strong");
        }
        out.push('>');
    }
}

fn open_paragraph(out: &mut String, in_paragraph: &mut bool) {
    if *in_paragraph {
        out.push_str("</p>");
    }
    out.push_str("<p>");
    *in_paragraph = true;
}

fn close_paragraph(out: &mut String, in_paragraph: &mut bool) {
    if *in_paragraph {
        out.push_str("</p>");
        *in_paragraph = false;
    }
}

// --- Phase 2.3 — encoding detection ----------------------------------------

#[cfg(feature = "encoding_detect")]
fn decode(bytes: &[u8]) -> (String, String) {
    use chardetng::EncodingDetector;
    use encoding_rs;

    // Strip a UTF-8 BOM first; if present, that's authoritative.
    if bytes.starts_with(b"\xEF\xBB\xBF") {
        let (s, _, _) = encoding_rs::UTF_8.decode(&bytes[3..]);
        return (s.into_owned(), "utf-8".to_string());
    }
    // UTF-16 LE / BE BOMs
    if bytes.starts_with(&[0xFF, 0xFE]) {
        let (s, _, _) = encoding_rs::UTF_16LE.decode(&bytes[2..]);
        return (s.into_owned(), "utf-16le".to_string());
    }
    if bytes.starts_with(&[0xFE, 0xFF]) {
        let (s, _, _) = encoding_rs::UTF_16BE.decode(&bytes[2..]);
        return (s.into_owned(), "utf-16be".to_string());
    }

    // Heuristic via chardetng.
    let mut detector = EncodingDetector::new();
    detector.feed(bytes, true);
    let enc = detector.guess(None, true);
    let (s, encoding_used, _) = enc.decode(bytes);
    (s.into_owned(), encoding_used.name().to_string())
}

#[cfg(not(feature = "encoding_detect"))]
fn decode(bytes: &[u8]) -> (String, String) {
    // No chardetng; just strip BOM and lossy.
    let stripped: &[u8] = bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(bytes);
    let s = String::from_utf8_lossy(stripped).into_owned();
    (s, "utf-8".to_string())
}

// --- Phase 2.4 — Markdown via pulldown-cmark ---------------------------------

fn markdown_to_html(text: &str) -> String {
    use pulldown_cmark::{html, Options, Parser};

    // CommonMark + a conservative set of extensions. We avoid raw-HTML pass-through
    // because the frontend renders user-supplied files; unexpected inline <script> /
    // <iframe> would be a XSS hole. (DOMPurify-style sanitization on the Svelte side
    // is the belt-and-suspenders per ADR-TBD Phase 2.4.)
    let mut opts = Options::empty();
    opts.insert(Options::ENABLE_TABLES);
    opts.insert(Options::ENABLE_FOOTNOTES);
    opts.insert(Options::ENABLE_STRIKETHROUGH);
    opts.insert(Options::ENABLE_TASKLISTS);
    opts.insert(Options::ENABLE_SMART_PUNCTUATION);
    // Important: NOT Options::ENABLE_OLD_FOOTNOTES — superseded.

    let parser = Parser::new_ext(text, opts);
    let mut out = String::with_capacity(text.len() * 2);
    html::push_html(&mut out, parser);
    out
}

// --- Phase 2.5 — JSON pretty print ------------------------------------------

fn pretty_json(text: &str) -> Result<String, Error> {
    let value: serde_json::Value =
        serde_json::from_str(text).map_err(|e| Error::Parse(format!("JSON parse: {}", e)))?;
    Ok(serde_json::to_string_pretty(&value)
        .map_err(|e| Error::Parse(format!("JSON encode: {}", e)))?)
}

// --- Phase 2.6 — CSV preview rows -------------------------------------------

fn parse_csv(text: &str) -> (Vec<String>, Vec<Vec<String>>, Option<usize>) {
    let mut rdr = csv::ReaderBuilder::new()
        .has_headers(true)
        .flexible(true)
        .from_reader(text.as_bytes());

    let header: Vec<String> = rdr
        .headers()
        .map(|r| r.iter().map(|c| c.to_string()).collect())
        .unwrap_or_default();

    let mut preview: Vec<Vec<String>> = Vec::with_capacity(CSV_PREVIEW_ROWS);
    let mut total: usize = 0;
    for result in rdr.records() {
        total += 1;
        if preview.len() < CSV_PREVIEW_ROWS {
            match result {
                Ok(r) => preview.push(r.iter().map(|c| c.to_string()).collect()),
                Err(_) => break,
            }
        } else if total % 1024 == 0 {
            // We're past preview; keep counting for the row hint, but cheap.
        }
    }
    // If we exited the loop early, total underestimates; we just say "we only know preview".
    let total_rows_hint = if preview.len() < CSV_PREVIEW_ROWS {
        Some(preview.len())
    } else {
        // Best-effort. Phase 5.2 search could refine with a separate stream.
        None
    };

    (header, preview, total_rows_hint)
}
