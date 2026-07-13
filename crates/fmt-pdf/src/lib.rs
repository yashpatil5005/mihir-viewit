//! Phase 2.2 — PDF via pdfium-render.
//!
//! Per ADR 0002: we bundle our own `libpdfium.so` (arm64-v8a for Android;
//! x86_64 for desktop Linux). The `pdfium-render` crate does runtime
//! dynamic linking — it looks for `libpdfium.so` in the same directory
//! as the executable, falling back to the system library.
//!
//! We rasterize each page to a PNG bitmap, encode as base64 data URLs,
//! and return them in `Document::Pdf`. The frontend renders each page
//! as an `<img>` with lazy loading.

use viewit_core_types::{Document, Error, Format};
use pdfium_render::prelude::*;
use image::{ImageFormat, DynamicImage};

/// Maximum number of pages we eagerly rasterize. Large PDFs will have
/// the first N pages shipped; the rest are fetched via `pdf_page` command.
const MAX_EAGER_PAGES: usize = 5;

pub fn parse(bytes: &[u8], _format: Format, _name: &str) -> Result<Document, Error> {
    // Bind to pdfium at runtime. Per ADR 0002:
    // - Android: MainActivity.kt calls `System.loadLibrary("pdfium")` at class init,
    //   so by the time we get here the lib pdfium is already in the linker namespace.
    //   `bind_to_system_library()` will resolve `libpdfium.so` via `dlopen`.
    // - Desktop: try the bundled .so next to the binary (./libpdfium.so), then the
    //   system library as a fallback.
    let bindings = if cfg!(target_os = "android") {
        Pdfium::bind_to_system_library()
    } else {
        Pdfium::bind_to_library(Pdfium::pdfium_platform_library_name_at_path("./"))
            .or_else(|_| Pdfium::bind_to_system_library())
    }
    .map_err(|e| Error::Parse(format!("pdfium bind: {}", e)))?;
    let pdfium = Pdfium::new(bindings);

    let document = pdfium.load_pdf_from_byte_vec(bytes.to_vec(), None)
        .map_err(|e| Error::Parse(format!("pdfium load: {}", e)))?;

    let page_count = document.pages().len() as usize;
    let mut pages: Vec<String> = Vec::with_capacity(page_count.min(MAX_EAGER_PAGES));

    // Render config: target a reasonable width for mobile screens.
    let render_cfg = PdfRenderConfig::new()
        .set_target_width(1200);

    for (i, page) in document.pages().iter().enumerate() {
        if i >= MAX_EAGER_PAGES {
            break;
        }
        let bitmap = page
            .render_with_config(&render_cfg)
            .map_err(|e| Error::Parse(format!("pdfium render page {}: {}", i, e)))?;
        let img = bitmap
            .as_image()
            .map_err(|e| Error::Parse(format!("as_image page {}: {}", i, e)))?
            .into_rgb8();
        let mut buf = Vec::new();
        DynamicImage::ImageRgb8(img)
            .write_to(&mut std::io::Cursor::new(&mut buf), ImageFormat::Png)
            .map_err(|e| Error::Parse(format!("png encode page {}: {}", i, e)))?;
        use base64::Engine;
        let b64 = base64::engine::general_purpose::STANDARD.encode(&buf);
        pages.push(format!("data:image/png;base64,{}", b64));
    }

    Ok(Document::Pdf {
        page_count,
        pages,
        byte_len: bytes.len(),
    })
}
