//! Image crate — decode unsupported formats (TIFF, DNG, NEF) to PNG for
//! WebView rendering, pass through formats the WebView handles natively.

use viewit_core_types::{Document, Error, Format};

#[allow(dead_code)]
pub fn parse(bytes: &[u8], format: Format, name: &str) -> Result<Document, Error> {
    Ok(Document::Image {
        format,
        byte_len: bytes.len(),
        name: name.to_string(),
        asset_path: String::new(),
        stream_url: None,
    })
}

/// Returns true if the WebView cannot natively decode this image format.
#[allow(dead_code)]
pub fn needs_conversion(format: Format) -> bool {
    matches!(
        format,
        Format::ImageTiff | Format::ImageRaw | Format::ImagePsd
    )
}

/// Decode image bytes to PNG. Used for formats WebView cannot render natively
/// (TIFF, DNG/NEF/CR2 camera RAW, PSD).
#[allow(dead_code)]
pub fn decode_to_png(bytes: &[u8], format: Format) -> Result<Vec<u8>, Error> {
    // Try standard path first
    if let Ok(img) = image::load_from_memory(bytes) {
        return encode_to_png(&img);
    }

    // Fallback: try TIFF-specific decoding with exotic color types
    // DNG/NEF/CR2 are TIFF-based, so try TIFF decode for ImageRaw too
    if matches!(format, Format::ImageTiff | Format::ImageRaw) {
        match decode_tiff_exotic(bytes) {
            Ok(img) => return encode_to_png(&img),
            Err(e) => {
                eprintln!("[viewit] tiff exotic decode failed: {}", e);
            }
        }
    }

    Err(Error::Parse(format!(
        "failed to decode {:?} to PNG",
        format_short_name(format)
    )))
}

/// Decode TIFF with exotic color types (YCbCr, CMYK, Multiband).
fn decode_tiff_exotic(bytes: &[u8]) -> Result<image::DynamicImage, String> {
    // Try standard image crate path first
    if let Ok(img) = image::load_from_memory(bytes) {
        return Ok(img);
    }

    // Parse header to get color type via tiff crate
    use std::io::Cursor;
    let mut decoder = tiff::decoder::Decoder::new(Cursor::new(bytes))
        .map_err(|e| format!("tiff decoder init: {}", e))?;
    let (width, height) = decoder
        .dimensions()
        .map_err(|e| format!("tiff dimensions: {}", e))?;
    let color_type = decoder
        .colortype()
        .map_err(|e| format!("tiff colortype: {}", e))?;
    eprintln!("[viewit] tiff color_type={:?} {}x{}", color_type, width, height);

    // For YCbCr: read strips manually since tiff crate fails on ChromaSubsampling
    if matches!(color_type, tiff::ColorType::YCbCr(_)) {
        match decode_tiff_ycbcr_manual(bytes, width, height) {
            Ok(img) => return Ok(img),
            Err(e) => {
                eprintln!("[viewit] tiff ycbcr manual failed ({}), trying tiff crate read_image", e);
            }
        }
        // Fallback: try tiff crate read_image (may work for non-subsampled or JPEG-compressed)
        let buf = decoder.read_image().map_err(|e| format!("tiff read: {:?}", e))?;
        if let tiff::decoder::DecodingResult::U8(pixels) = buf {
            // For YCbCr without subsampling, pixel count should equal w*h*3
            if pixels.len() == (width * height * 3) as usize {
                let rgb = ycbcr_to_rgb(&pixels, width as usize, height as usize);
                return make_rgb_image(width, height, rgb, "YCbCr-crate");
            }
            // Maybe it decoded to RGB directly
            if pixels.len() == (width * height * 3) as usize {
                return make_rgb_image(width, height, pixels, "YCbCr-crate-rgb");
            }
            return Err(format!("YCbCr crate decode: unexpected pixel count {}", pixels.len()));
        }
        return Err("YCbCr crate decode: unexpected result type".into());
    }

    let buf = decoder
        .read_image()
        .map_err(|e| {
            eprintln!("[viewit] tiff read_image error: {:?}", e);
            format!("tiff read: {:?}", e)
        })?;

    match color_type {
        tiff::ColorType::CMYK(8) => {
            if let tiff::decoder::DecodingResult::U8(pixels) = buf {
                let rgb = cmyk_to_rgb(&pixels, width as usize, height as usize);
                return make_rgb_image(width, height, rgb, "CMYK");
            }
            Err("unsupported CMYK decoding result variant".into())
        }
        tiff::ColorType::Multiband { bit_depth, .. } => {
            if let tiff::decoder::DecodingResult::U8(pixels) = buf {
                let w = width as usize;
                let h = height as usize;
                let expected_rgb = w * h * 3;
                let expected_rgba = w * h * 4;
                if pixels.len() == expected_rgb {
                    return make_rgb_image(width, height, pixels, "Multiband-RGB");
                } else if pixels.len() == expected_rgba {
                    let rgb: Vec<u8> = pixels
                        .chunks_exact(4)
                        .flat_map(|c| [c[0], c[1], c[2]])
                        .collect();
                    return make_rgb_image(width, height, rgb, "Multiband-RGBA");
                } else if bit_depth == 8 {
                    let rgb: Vec<u8> = pixels
                        .chunks_exact(3)
                        .take(w * h)
                        .flat_map(|c| [c[0], c[1], c[2]])
                        .collect();
                    if rgb.len() == expected_rgb {
                        return make_rgb_image(width, height, rgb, "Multiband-partial");
                    }
                }
            }
            Err("unsupported Multiband layout".into())
        }
        tiff::ColorType::RGB(8) => {
            if let tiff::decoder::DecodingResult::U8(pixels) = buf {
                return make_rgb_image(width, height, pixels, "RGB");
            }
            Err("unexpected decoding result for RGB8".into())
        }
        tiff::ColorType::Gray(8) => {
            if let tiff::decoder::DecodingResult::U8(pixels) = buf {
                let img = image::DynamicImage::ImageLuma8(
                    image::ImageBuffer::from_raw(width, height, pixels)
                        .ok_or("failed to create gray image buffer")?,
                );
                return Ok(img);
            }
            Err("unexpected decoding result for Gray8".into())
        }
        other => Err(format!("unsupported TIFF color type: {:?}", other)),
    }
}

/// Manual TIFF strip reader for YCbCr images.
/// Bypasses the tiff crate's broken ChromaSubsampling handling.
fn decode_tiff_ycbcr_manual(bytes: &[u8], width: u32, height: u32) -> Result<image::DynamicImage, String> {
    if bytes.len() < 8 {
        return Err("tiff too short".into());
    }
    let le = bytes[0] == b'I' && bytes[1] == b'I';
    let u16r = |o: usize| -> u16 {
        if le { u16::from_le_bytes([bytes[o], bytes[o+1]]) } else { u16::from_be_bytes([bytes[o], bytes[o+1]]) }
    };
    let u32r = |o: usize| -> u32 {
        if le { u32::from_le_bytes([bytes[o], bytes[o+1], bytes[o+2], bytes[o+3]]) }
        else { u32::from_be_bytes([bytes[o], bytes[o+1], bytes[o+2], bytes[o+3]]) }
    };

    if u16r(2) != 42 {
        return Err(format!("invalid TIFF magic: {}", u16r(2)));
    }
    let ifd_off = u32r(4) as usize;
    let num_entries = u16r(ifd_off);

    let mut strip_offsets: Vec<u32> = Vec::new();
    let mut strip_byte_counts: Vec<u32> = Vec::new();
    let mut compression: u16 = 1;
    let mut rows_per_strip: u32 = height;

    let mut pos = ifd_off + 2;
    for _ in 0..num_entries {
        if pos + 12 > bytes.len() {
            break;
        }
        let tag = u16r(pos);
        let typ = u16r(pos + 2);
        let count = u32r(pos + 4);

        match tag {
            259 => {
                compression = if typ == 3 && count == 1 { u16r(pos + 8) } else { 1 };
            }
            278 => {
                rows_per_strip = if typ == 4 && count == 1 {
                    u32r(pos + 8)
                } else if typ == 3 && count == 1 {
                    u16r(pos + 8) as u32
                } else {
                    height
                };
            }
            273 => {
                if count == 1 {
                    strip_offsets = vec![u32r(pos + 8)];
                } else {
                    let off = u32r(pos + 8) as usize;
                    strip_offsets = (0..count as usize)
                        .map(|i| u32r(off + i * 4))
                        .collect();
                }
            }
            279 => {
                if count == 1 {
                    strip_byte_counts = vec![u32r(pos + 8)];
                } else {
                    let off = u32r(pos + 8) as usize;
                    strip_byte_counts = (0..count as usize)
                        .map(|i| u32r(off + i * 4))
                        .collect();
                }
            }
            _ => {}
        }
        pos += 12;
    }

    eprintln!(
        "[viewit] tiff strips: comp={}, rps={}, #off={}, #cnt={}",
        compression,
        rows_per_strip,
        strip_offsets.len(),
        strip_byte_counts.len()
    );

    let mut all_yuv = Vec::with_capacity((width * height * 3) as usize);

    for (&offset, &byte_count) in strip_offsets.iter().zip(strip_byte_counts.iter()) {
        let start = offset as usize;
        let end = (offset + byte_count) as usize;
        if end > bytes.len() {
            return Err("strip overflows input".into());
        }
        let strip_data = &bytes[start..end];
        let decompressed = match compression {
            1 => strip_data.to_vec(),
            32773 => packbits_decompress(strip_data)?,
            _ => return Err(format!("unsupported TIFF compression: {}", compression)),
        };
        all_yuv.extend_from_slice(&decompressed);
    }

    let expected = (width * height * 3) as usize;
    if all_yuv.len() > expected {
        all_yuv.truncate(expected);
    }

    eprintln!(
        "[viewit] tiff ycbcr raw pixels: {} (expected {})",
        all_yuv.len(),
        expected
    );

    // Assemble YCbCr from per-strip planar data.
    // Each strip contains: Y(rows*320), Cb(rows/2*160), Cr(rows/2*160)
    // We need to extract per-strip planes and copy into full-image planes.
    let mut y_plane_full = vec![0u8; (width * height) as usize];
    let cb_w = ((width + 1) / 2) as usize;
    let cb_h = ((height + 1) / 2) as usize;
    let mut cb_plane_full = vec![0u8; cb_w * cb_h];
    let mut cr_plane_full = vec![0u8; cb_w * cb_h];

    let mut src_offset = 0usize;
    let mut y_dst = 0usize;
    let mut cb_dst = 0usize;
    let mut cr_dst = 0usize;

    for (i, &_byte_count) in strip_byte_counts.iter().enumerate() {
        let strip_rows = if i < strip_byte_counts.len() - 1 {
            rows_per_strip as usize
        } else {
            // Last strip may be shorter
            let remaining = height as usize - y_dst / width as usize;
            remaining.min(rows_per_strip as usize)
        };
        let strip_y_size = width as usize * strip_rows;
        let strip_cb_size = cb_w * ((strip_rows + 1) / 2);
        let strip_cr_size = strip_cb_size;

        // Copy Y
        let y_end = (src_offset + strip_y_size).min(all_yuv.len());
        let copy_len = (y_end - src_offset).min(y_plane_full.len() - y_dst);
        y_plane_full[y_dst..y_dst + copy_len].copy_from_slice(&all_yuv[src_offset..src_offset + copy_len]);
        src_offset += strip_y_size;
        y_dst += copy_len;

        // Copy Cb
        let cb_end = (src_offset + strip_cb_size).min(all_yuv.len());
        let copy_len = (cb_end - src_offset).min(cb_plane_full.len() - cb_dst);
        cb_plane_full[cb_dst..cb_dst + copy_len].copy_from_slice(&all_yuv[src_offset..src_offset + copy_len]);
        src_offset += strip_cb_size;
        cb_dst += copy_len;

        // Copy Cr
        let cr_end = (src_offset + strip_cr_size).min(all_yuv.len());
        let copy_len = (cr_end - src_offset).min(cr_plane_full.len() - cr_dst);
        cr_plane_full[cr_dst..cr_dst + copy_len].copy_from_slice(&all_yuv[src_offset..src_offset + copy_len]);
        src_offset += strip_cr_size;
        cr_dst += copy_len;
    }

    eprintln!(
        "[viewit] tiff planes: y={} cb={} cr={}",
        y_plane_full.len(),
        cb_plane_full.len(),
        cr_plane_full.len()
    );

    // Check if data is chroma subsampled (4:2:0 = 1.5 bytes/pixel)
    let bytes_per_pixel = all_yuv.len() as f64 / (width as f64 * height as f64);
    let rgb = if (bytes_per_pixel - 1.5).abs() < 0.01 {
        // 4:2:0 subsampling with per-strip planar layout
        ycbcr_420_from_planes(&y_plane_full, &cb_plane_full, &cr_plane_full, width as usize, height as usize, cb_w)
    } else if (bytes_per_pixel - 2.0).abs() < 0.01 {
        ycbcr_422_from_planes(&y_plane_full, &cb_plane_full, &cr_plane_full, width as usize, height as usize, cb_w)
    } else {
        ycbcr_to_rgb(&y_plane_full, width as usize, height as usize)
    };
    make_rgb_image(width, height, rgb, "YCbCr-strips")
}

/// PackBits decompression for TIFF strips.
fn packbits_decompress(input: &[u8]) -> Result<Vec<u8>, String> {
    let mut output = Vec::with_capacity(input.len() * 2);
    let mut i = 0;
    while i < input.len() {
        let header = input[i] as i8;
        i += 1;
        if header >= 0 {
            let count = (header as usize) + 1;
            if i + count > input.len() {
                return Err("packbits: overread during literal copy".into());
            }
            output.extend_from_slice(&input[i..i + count]);
            i += count;
        } else if header == -128 {
            // No-op
        } else {
            let count = (1 - header as i32) as usize;
            if i >= input.len() {
                return Err("packbits: overread during repeat".into());
            }
            let byte = input[i];
            i += 1;
            output.extend(std::iter::repeat(byte).take(count));
        }
    }
    Ok(output)
}

fn make_rgb_image(
    w: u32,
    h: u32,
    rgb: Vec<u8>,
    source: &str,
) -> Result<image::DynamicImage, String> {
    let expected = w as usize * h as usize * 3;
    if rgb.len() < expected {
        return Err(format!(
            "{}: pixel data too short ({} < {})",
            source,
            rgb.len(),
            expected
        ));
    }
    let img = image::DynamicImage::ImageRgb8(
        image::ImageBuffer::from_raw(w, h, rgb)
            .ok_or_else(|| format!("failed to create RGB image buffer from {}", source))?,
    );
    Ok(img)
}

/// YCbCr (BT.601) 4:4:4 → RGB (no chroma subsampling).
fn ycbcr_to_rgb(pixels: &[u8], w: usize, h: usize) -> Vec<u8> {
    let mut rgb = Vec::with_capacity(w * h * 3);
    for chunk in pixels.chunks_exact(3) {
        let y = chunk[0] as f32;
        let cb = chunk[1] as f32 - 128.0;
        let cr = chunk[2] as f32 - 128.0;
        let r = (y + 1.402 * cr).clamp(0.0, 255.0) as u8;
        let g = (y - 0.344136 * cb - 0.714136 * cr).clamp(0.0, 255.0) as u8;
        let b = (y + 1.772 * cb).clamp(0.0, 255.0) as u8;
        rgb.extend_from_slice(&[r, g, b]);
    }
    rgb
}

/// YCbCr 4:2:0 from separate Y, Cb, Cr planes → RGB.
fn ycbcr_420_from_planes(y: &[u8], cb: &[u8], cr: &[u8], w: usize, h: usize, cb_w: usize) -> Vec<u8> {
    let mut rgb = vec![0u8; w * h * 3];
    for row in 0..h {
        for col in 0..w {
            let yi = row * w + col;
            let y_val = if yi < y.len() { y[yi] as f32 } else { 0.0 };
            let ci = (row / 2) * cb_w + col / 2;
            let cb_val = if ci < cb.len() { cb[ci] as f32 - 128.0 } else { 0.0 };
            let cr_val = if ci < cr.len() { cr[ci] as f32 - 128.0 } else { 0.0 };
            let r = (y_val + 1.402 * cr_val).clamp(0.0, 255.0) as u8;
            let g = (y_val - 0.344136 * cb_val - 0.714136 * cr_val).clamp(0.0, 255.0) as u8;
            let b = (y_val + 1.772 * cb_val).clamp(0.0, 255.0) as u8;
            let off = (row * w + col) * 3;
            rgb[off] = r;
            rgb[off + 1] = g;
            rgb[off + 2] = b;
        }
    }
    rgb
}

/// YCbCr 4:2:2 from separate Y, Cb, Cr planes → RGB.
fn ycbcr_422_from_planes(y: &[u8], cb: &[u8], cr: &[u8], w: usize, h: usize, cb_w: usize) -> Vec<u8> {
    let mut rgb = vec![0u8; w * h * 3];
    for row in 0..h {
        for col in 0..w {
            let yi = row * w + col;
            let y_val = if yi < y.len() { y[yi] as f32 } else { 0.0 };
            let ci = row * cb_w + col / 2;
            let cb_val = if ci < cb.len() { cb[ci] as f32 - 128.0 } else { 0.0 };
            let cr_val = if ci < cr.len() { cr[ci] as f32 - 128.0 } else { 0.0 };
            let r = (y_val + 1.402 * cr_val).clamp(0.0, 255.0) as u8;
            let g = (y_val - 0.344136 * cb_val - 0.714136 * cr_val).clamp(0.0, 255.0) as u8;
            let b = (y_val + 1.772 * cb_val).clamp(0.0, 255.0) as u8;
            let off = (row * w + col) * 3;
            rgb[off] = r;
            rgb[off + 1] = g;
            rgb[off + 2] = b;
        }
    }
    rgb
}

/// CMYK (4 bytes per pixel, interleaved) → RGB (3 bytes per pixel).
fn cmyk_to_rgb(pixels: &[u8], w: usize, h: usize) -> Vec<u8> {
    let mut rgb = Vec::with_capacity(w * h * 3);
    for chunk in pixels.chunks_exact(4) {
        let c = chunk[0] as f32 / 255.0;
        let m = chunk[1] as f32 / 255.0;
        let y = chunk[2] as f32 / 255.0;
        let k = chunk[3] as f32 / 255.0;
        let r = ((1.0 - c) * (1.0 - k) * 255.0) as u8;
        let g = ((1.0 - m) * (1.0 - k) * 255.0) as u8;
        let b = ((1.0 - y) * (1.0 - k) * 255.0) as u8;
        rgb.extend_from_slice(&[r, g, b]);
    }
    rgb
}

fn encode_to_png(img: &image::DynamicImage) -> Result<Vec<u8>, Error> {
    let rgb = img.to_rgb8();
    let mut png_bytes: Vec<u8> = Vec::new();
    let encoder = image::codecs::png::PngEncoder::new(&mut png_bytes);
    image::ImageEncoder::write_image(
        encoder,
        rgb.as_raw(),
        rgb.width(),
        rgb.height(),
        image::ExtendedColorType::Rgb8,
    )
    .map_err(|e| Error::Parse(format!("PNG encode failed: {}", e)))?;
    Ok(png_bytes)
}

fn format_short_name(f: Format) -> &'static str {
    match f {
        Format::ImageTiff => "TIFF",
        Format::ImageRaw => "RAW",
        Format::ImagePsd => "PSD",
        _ => "image",
    }
}
