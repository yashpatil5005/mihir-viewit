//! Phase 2.2 — PDF via pdfium-render (ADR 0002 bundled libpdfium on Android).

use image::{DynamicImage, ImageFormat};
use pdfium_render::prelude::*;
use viewit_core_types::{Document, Error, Format};

#[cfg(not(target_os = "android"))]
const MAX_EAGER_PAGES: usize = 5;
#[cfg(target_os = "android")]
const MAX_EAGER_PAGES: usize = 2;

fn bind_pdfium() -> Result<Pdfium, Error> {
    let bindings = if cfg!(target_os = "android") {
        Pdfium::bind_to_system_library()
    } else {
        Pdfium::bind_to_library(Pdfium::pdfium_platform_library_name_at_path("./"))
            .or_else(|_| Pdfium::bind_to_system_library())
    }
    .map_err(|e| Error::Parse(format!("pdfium bind: {}", e)))?;
    Ok(Pdfium::new(bindings))
}

fn render_page_png_data_url(
    pdfium: &Pdfium,
    bytes: &[u8],
    page_index: usize,
) -> Result<String, Error> {
    let document = pdfium
        .load_pdf_from_byte_vec(bytes.to_vec(), None)
        .map_err(|e| Error::Parse(format!("pdfium load: {}", e)))?;
    let render_cfg = PdfRenderConfig::new().set_target_width(900);
    let page = document
        .pages()
        .iter()
        .nth(page_index)
        .ok_or_else(|| Error::Parse(format!("pdfium page {}: out of range", page_index)))?;
    let bitmap = page
        .render_with_config(&render_cfg)
        .map_err(|e| Error::Parse(format!("pdfium render page {}: {}", page_index, e)))?;
    let img = bitmap
        .as_image()
        .map_err(|e| Error::Parse(format!("as_image page {}: {}", page_index, e)))?
        .into_rgb8();
    let mut buf = Vec::new();
    DynamicImage::ImageRgb8(img)
        .write_to(&mut std::io::Cursor::new(&mut buf), ImageFormat::Png)
        .map_err(|e| Error::Parse(format!("png encode page {}: {}", page_index, e)))?;
    use base64::Engine;
    let b64 = base64::engine::general_purpose::STANDARD.encode(&buf);
    Ok(format!("data:image/png;base64,{}", b64))
}

pub fn render_page(bytes: &[u8], page_index: usize) -> Result<String, Error> {
    let pdfium = bind_pdfium()?;
    render_page_png_data_url(&pdfium, bytes, page_index)
}

pub fn parse(bytes: &[u8], _format: Format, _name: &str) -> Result<Document, Error> {
    let pdfium = bind_pdfium()?;
    let document = pdfium
        .load_pdf_from_byte_vec(bytes.to_vec(), None)
        .map_err(|e| Error::Parse(format!("pdfium load: {}", e)))?;
    let page_count = document.pages().len() as usize;
    let mut pages = Vec::with_capacity(page_count.min(MAX_EAGER_PAGES));
    for i in 0..page_count.min(MAX_EAGER_PAGES) {
        pages.push(render_page_png_data_url(&pdfium, bytes, i)?);
    }
    Ok(Document::Pdf {
        page_count,
        pages,
        byte_len: bytes.len(),
        native: false,
        name: _name.to_string(),
        stream_url: None,
    })
}
