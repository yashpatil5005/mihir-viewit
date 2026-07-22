# ViewIt Supported File Formats

**Last Updated:** July 22, 2026

ViewIt is a universal file viewer supporting 250+ file formats across text, code, images, documents, eBooks, archives, and media files.

## Platform Compatibility Matrix

| Symbol | Meaning |
|--------|---------|
| ✅ | Fully supported and tested |
| ⚠️ | Supported but with limitations (see notes) |
| 🔄 | Implemented but not fully tested |
| 📦 | Available in release builds only |
| ❌ | Not supported |
| 🚧 | Planned for future release |

### Platform Support Summary

- **Android**: Primary target platform (API 34+, WebView Chrome 113+)
- **iOS**: Planned (not yet tested)
- **Desktop** (Windows/macOS/Linux): Planned (not yet tested)
- **Web**: Future expansion planned

---

## 1. Text & Plain Text Formats

### 1.1 Plain Text

| Format | Extensions | Android | iOS | Desktop | Web | Notes |
|--------|-----------|---------|-----|---------|-----|-------|
| Plain Text | `.txt`, `.text`, `.log` | ✅ | 🚧 | 🚧 | 🚧 | UTF-8, Latin-1, auto-detected |
| README | `.readme` | ✅ | 🚧 | 🚧 | 🚧 | Plain text format |
| Git Config | `.gitignore`, `.gitattributes` | ✅ | 🚧 | 🚧 | 🚧 | |
| Editor Config | `.editorconfig` | ✅ | 🚧 | 🚧 | 🚧 | |
| Docker Config | `.dockerignore` | ✅ | 🚧 | 🚧 | 🚧 | |
| NPM Config | `.npmignore` | ✅ | 🚧 | 🚧 | 🚧 | |
| NFO Files | `.nfo` | ✅ | 🚧 | 🚧 | 🚧 | Legacy text info files |
| Strings | `.strings` | ✅ | 🚧 | 🚧 | 🚧 | iOS/macOS localization |

### 1.2 Subtitles & Captions

| Format | Extensions | Android | iOS | Desktop | Web | Notes |
|--------|-----------|---------|-----|---------|-----|-------|
| SubRip | `.srt` | ✅ | 🚧 | 🚧 | 🚧 | Standard subtitle format |
| WebVTT | `.vtt` | ✅ | 🚧 | 🚧 | 🚧 | Web video captions |
| SUB | `.sub` | ✅ | 🚧 | 🚧 | 🚧 | MicroDVD subtitles |

### 1.3 Documentation Formats

| Format | Extensions | Android | iOS | Desktop | Web | Notes |
|--------|-----------|---------|-----|---------|-----|-------|
| Markdown | `.md`, `.markdown`, `.mdown`, `.mkdn`, `.mdx` | ✅ | 🚧 | 🚧 | 🚧 | Rendered HTML with syntax highlighting |
| reStructuredText | `.rst` | ✅ | 🚧 | 🚧 | 🚧 | Python documentation format |
| AsciiDoc | `.adoc`, `.asc` | ✅ | 🚧 | 🚧 | 🚧 | Advanced documentation format |

### 1.4 Structured Data Formats

| Format | Extensions | Android | iOS | Desktop | Web | Notes |
|--------|-----------|---------|-----|---------|-----|-------|
| JSON | `.json`, `.jsonc`, `.json5` | ✅ | 🚧 | 🚧 | 🚧 | Pretty-printed with syntax highlighting |
| JSON Lines | `.jsonl`, `.ndjson` | ✅ | 🚧 | 🚧 | 🚧 | Newline-delimited JSON |
| Lock Files | `.lock` | ✅ | 🚧 | 🚧 | 🚧 | package-lock.json, yarn.lock, etc. |
| Jupyter Notebook | `.ipynb` | ✅ | 🚧 | 🚧 | 🚧 | JSON format, code cells extracted |
| CSV | `.csv` | ✅ | 🚧 | 🚧 | 🚧 | Virtualized table view |
| TSV | `.tsv` | ✅ | 🚧 | 🚧 | 🚧 | Tab-separated values |
| YAML | `.yaml`, `.yml` | ✅ | 🚧 | 🚧 | 🚧 | Code syntax highlighting |
| TOML | `.toml` | ✅ | 🚧 | 🚧 | 🚧 | Config file format |
| INI | `.ini`, `.cfg`, `.conf`, `.config` | ✅ | 🚧 | 🚧 | 🚧 | Configuration files |
| ENV | `.env` | ✅ | 🚧 | 🚧 | 🚧 | Environment variables |
| XML | `.xml`, `.xsd`, `.xsl`, `.xslt` | ✅ | 🚧 | 🚧 | 🚧 | Code syntax highlighting |
| Property List | `.plist` | 🔄 | 🚧 | 🚧 | 🚧 | Apple configuration format |

### 1.5 Web Development

| Format | Extensions | Android | iOS | Desktop | Web | Notes |
|--------|-----------|---------|-----|---------|-----|-------|
| HTML | `.html`, `.htm`, `.xhtml` | ✅ | 🚧 | 🚧 | 🚧 | Code view with syntax highlighting |
| CSS | `.css` | ✅ | 🚧 | 🚧 | 🚧 | Stylesheets |
| SCSS/Sass | `.scss`, `.sass` | ✅ | 🚧 | 🚧 | 🚧 | CSS preprocessors |
| Less | `.less` | ✅ | 🚧 | 🚧 | 🚧 | CSS preprocessor |
| JavaScript | `.js`, `.mjs`, `.cjs` | ✅ | 🚧 | 🚧 | 🚧 | ES modules supported |
| TypeScript | `.ts`, `.tsx` | ✅ | 🚧 | 🚧 | 🚧 | JSX/TSX syntax highlighting |
| JSX | `.jsx` | ✅ | 🚧 | 🚧 | 🚧 | React components |
| Vue | `.vue` | ✅ | 🚧 | 🚧 | 🚧 | Single-file components |
| Svelte | `.svelte` | ✅ | 🚧 | 🚧 | 🚧 | Svelte components |
| Astro | `.astro` | ✅ | 🚧 | 🚧 | 🚧 | Astro framework files |

### 1.6 Programming Languages

| Format | Extensions | Android | iOS | Desktop | Web | Notes |
|--------|-----------|---------|-----|---------|-----|-------|
| Rust | `.rs` | ✅ | 🚧 | 🚧 | 🚧 | |
| Python | `.py`, `.pyw` | ✅ | 🚧 | 🚧 | 🚧 | |
| Go | `.go` | ✅ | 🚧 | 🚧 | 🚧 | |
| C/C++ | `.c`, `.cc`, `.cpp`, `.cxx`, `.h`, `.hpp`, `.hh` | ✅ | 🚧 | 🚧 | 🚧 | |
| Java | `.java` | ✅ | 🚧 | 🚧 | 🚧 | |
| Kotlin | `.kt`, `.kts` | ✅ | 🚧 | 🚧 | 🚧 | |
| Swift | `.swift` | ✅ | 🚧 | 🚧 | 🚧 | |
| C# | `.cs` | ✅ | 🚧 | 🚧 | 🚧 | |
| F# | `.fs`, `.fsx` | ✅ | 🚧 | 🚧 | 🚧 | |
| Ruby | `.rb` | ✅ | 🚧 | 🚧 | 🚧 | |
| PHP | `.php` | ✅ | 🚧 | 🚧 | 🚧 | |
| Elixir | `.ex`, `.exs` | ✅ | 🚧 | 🚧 | 🚧 | |
| Erlang | `.erl`, `.hrl` | ✅ | 🚧 | 🚧 | 🚧 | |
| Haskell | `.hs` | ✅ | 🚧 | 🚧 | 🚧 | |
| OCaml | `.ml`, `.mli` | ✅ | 🚧 | 🚧 | 🚧 | |
| Clojure | `.clj`, `.cljs` | ✅ | 🚧 | 🚧 | 🚧 | |
| Scala | `.scala`, `.sc` | ✅ | 🚧 | 🚧 | 🚧 | |
| R | `.r` | ✅ | 🚧 | 🚧 | 🚧 | |
| Julia | `.jl` | ✅ | 🚧 | 🚧 | 🚧 | |
| Dart | `.dart` | ✅ | 🚧 | 🚧 | 🚧 | |
| Zig | `.zig` | ✅ | 🚧 | 🚧 | 🚧 | |
| Nim | `.nim` | ✅ | 🚧 | 🚧 | 🚧 | |
| V | `.v` | ✅ | 🚧 | 🚧 | 🚧 | |
| Lua | `.lua` | ✅ | 🚧 | 🚧 | 🚧 | |

### 1.7 Shell Scripts

| Format | Extensions | Android | iOS | Desktop | Web | Notes |
|--------|-----------|---------|-----|---------|-----|-------|
| Bash | `.sh`, `.bash` | ✅ | 🚧 | 🚧 | 🚧 | |
| Zsh | `.zsh` | ✅ | 🚧 | 🚧 | 🚧 | |
| Fish | `.fish` | ✅ | 🚧 | 🚧 | 🚧 | |
| PowerShell | `.ps1` | ✅ | 🚧 | 🚧 | 🚧 | |
| Batch | `.bat`, `.cmd` | ✅ | 🚧 | 🚧 | 🚧 | Windows batch files |

### 1.8 Database & Query Languages

| Format | Extensions | Android | iOS | Desktop | Web | Notes |
|--------|-----------|---------|-----|---------|-----|-------|
| SQL | `.sql` | ✅ | 🚧 | 🚧 | 🚧 | All SQL dialects |
| GraphQL | `.graphql`, `.gql` | ✅ | 🚧 | 🚧 | 🚧 | Schema and queries |

### 1.9 Build & Configuration

| Format | Extensions | Android | iOS | Desktop | Web | Notes |
|--------|-----------|---------|-----|---------|-----|-------|
| Gradle | `.gradle` | ✅ | 🚧 | 🚧 | 🚧 | Build scripts |
| CMake | `.cmake` | ✅ | 🚧 | 🚧 | 🚧 | Build configuration |
| Makefile | `.make`, `.mk`, `Makefile` | ✅ | 🚧 | 🚧 | 🚧 | |
| Dockerfile | `.dockerfile`, `Dockerfile` | ✅ | 🚧 | 🚧 | 🚧 | Container definitions |
| Protocol Buffers | `.proto` | ✅ | 🚧 | 🚧 | 🚧 | gRPC definitions |

### 1.10 Hardware Description & Low-Level

| Format | Extensions | Android | iOS | Desktop | Web | Notes |
|--------|-----------|---------|-----|---------|-----|-------|
| SystemVerilog | `.sv` | ✅ | 🚧 | 🚧 | 🚧 | HDL |
| VHDL | `.vhd`, `.vhdl` | ✅ | 🚧 | 🚧 | 🚧 | Hardware description |
| Assembly | `.asm`, `.s` | ✅ | 🚧 | 🚧 | 🚧 | Various architectures |
| WebAssembly | `.wasm`, `.wat` | ✅ | 🚧 | 🚧 | 🚧 | Binary and text format |

### 1.11 Other Code Formats

| Format | Extensions | Android | iOS | Desktop | Web | Notes |
|--------|-----------|---------|-----|---------|-----|-------|
| Vim Script | `.vim` | ✅ | 🚧 | 🚧 | 🚧 | Editor configuration |
| Java Bytecode | `.class` | ✅ | 🚧 | 🚧 | 🚧 | Treated as text |
| Patch/Diff | `.patch`, `.diff` | ✅ | 🚧 | 🚧 | 🚧 | Version control |
| Apache Config | `.htaccess` | ✅ | 🚧 | 🚧 | 🚧 | Web server config |
| KML/KMZ | `.kml`, `.kmz` | ✅ | 🚧 | 🚧 | 🚧 | Google Earth markup |
| Desktop Entry | `.desktop` | ✅ | 🚧 | 🚧 | 🚧 | Linux application launchers |

### 1.12 Calendar & Contacts

| Format | Extensions | Android | iOS | Desktop | Web | Notes |
|--------|-----------|---------|-----|---------|-----|-------|
| iCalendar | `.ics` | 🔄 | 🚧 | 🚧 | 🚧 | Calendar events |
| vCard | `.vcf` | 🔄 | 🚧 | 🚧 | 🚧 | Contact cards |

---

## 2. Image Formats

### 2.1 Common Raster Formats

| Format | Extensions | Android | iOS | Desktop | Web | Notes |
|--------|-----------|---------|-----|---------|-----|-------|
| PNG | `.png` | ✅ | 🚧 | 🚧 | 🚧 | Native WebView rendering |
| JPEG | `.jpg`, `.jpeg`, `.jfif` | ✅ | 🚧 | 🚧 | 🚧 | Native WebView rendering |
| GIF | `.gif` | ✅ | 🚧 | 🚧 | 🚧 | Animated GIF supported |
| WebP | `.webp` | ✅ | 🚧 | 🚧 | 🚧 | Modern web format |
| BMP | `.bmp` | ✅ | 🚧 | 🚧 | 🚧 | Windows bitmap |
| TIFF | `.tif`, `.tiff` | ✅ | 🚧 | 🚧 | 🚧 | Multi-page support |
| ICO | `.ico` | ✅ | 🚧 | 🚧 | 🚧 | Windows icons |
| ICNS | `.icns` | ✅ | 🚧 | 🚧 | 🚧 | macOS icons |

### 2.2 Vector Formats

| Format | Extensions | Android | iOS | Desktop | Web | Notes |
|--------|-----------|---------|-----|---------|-----|-------|
| SVG | `.svg`, `.svgz` | ✅ | 🚧 | 🚧 | 🚧 | Scalable vector graphics |

### 2.3 Modern/HEIF Formats

| Format | Extensions | Android | iOS | Desktop | Web | Notes |
|--------|-----------|---------|-----|---------|-----|-------|
| HEIC/HEIF | `.heic`, `.heif` | ✅ | 🚧 | 🚧 | 🚧 | Apple photos format |
| AVIF | `.avif` | ✅ | 🚧 | 🚧 | 🚧 | AV1 Image File Format |

### 2.4 Professional/RAW Formats

| Format | Extensions | Android | iOS | Desktop | Web | Notes |
|--------|-----------|---------|-----|---------|-----|-------|
| PSD | `.psd` | ✅ | 🚧 | 🚧 | 🚧 | Adobe Photoshop |
| Adobe DNG | `.dng` | ✅ | 🚧 | 🚧 | 🚧 | Digital Negative |
| Canon RAW | `.cr2`, `.cr3` | ✅ | 🚧 | 🚧 | 🚧 | Canon cameras |
| Nikon RAW | `.nef` | ✅ | 🚧 | 🚧 | 🚧 | Nikon cameras |
| Sony RAW | `.arw` | ✅ | 🚧 | 🚧 | 🚧 | Sony cameras |
| Olympus RAW | `.orf` | ✅ | 🚧 | 🚧 | 🚧 | Olympus cameras |
| Panasonic RAW | `.rw2` | ✅ | 🚧 | 🚧 | 🚧 | Panasonic cameras |
| Fuji RAW | `.raf` | ✅ | 🚧 | 🚧 | 🚧 | Fujifilm cameras |
| Samsung RAW | `.srw` | ✅ | 🚧 | 🚧 | 🚧 | Samsung cameras |
| Pentax RAW | `.pef` | ✅ | 🚧 | 🚧 | 🚧 | Pentax cameras |

### 2.5 Advanced/Specialized Formats

| Format | Extensions | Android | iOS | Desktop | Web | Notes |
|--------|-----------|---------|-----|---------|-----|-------|
| JPEG 2000 | `.jp2` | 🔄 | 🚧 | 🚧 | 🚧 | High-quality compression |
| JPEG Stereo | `.jps` | 🔄 | 🚧 | 🚧 | 🚧 | 3D stereoscopic images |
| OpenEXR | `.exr` | 🔄 | 🚧 | 🚧 | 🚧 | HDR image format |
| Radiance HDR | `.hdr` | 🔄 | 🚧 | 🚧 | 🚧 | High dynamic range |
| FITS | `.fts` | 🔄 | 🚧 | 🚧 | 🚧 | Astronomy images |
| DDS | `.dds` | 🔄 | 🚧 | 🚧 | 🚧 | DirectDraw Surface |
| Kodak RAW | `.erf` | 🔄 | 🚧 | 🚧 | 🚧 | Kodak cameras |
| Sigma RAW | `.x3f` | 🔄 | 🚧 | 🚧 | 🚧 | Sigma cameras |
| Sony RAW 2 | `.sfw` | 🔄 | 🚧 | 🚧 | 🚧 | Sony alternative format |

### 2.6 Legacy/Uncommon Formats

| Format | Extensions | Android | iOS | Desktop | Web | Notes |
|--------|-----------|---------|-----|---------|-----|-------|
| TGA | `.tga` | 🔄 | 🚧 | 🚧 | 🚧 | Truevision Targa |
| PCX | `.pcx` | 🔄 | 🚧 | 🚧 | 🚧 | PC Paintbrush |
| SGI | `.sgi` | 🔄 | 🚧 | 🚧 | 🚧 | Silicon Graphics |
| Sun Raster | `.ras` | 🔄 | 🚧 | 🚧 | 🚧 | Sun Microsystems |
| PICT | `.pict`, `.picon` | 🔄 | 🚧 | 🚧 | 🚧 | Classic Mac format |
| XPM | `.xpm` | 🔄 | 🚧 | 🚧 | 🚧 | X11 Pixmap |
| XBM | `.xbm` | 🔄 | 🚧 | 🚧 | 🚧 | X11 Bitmap |
| XCF | `.xcf` | 🔄 | 🚧 | 🚧 | 🚧 | GIMP native format |
| XWD | `.xwd` | 🔄 | 🚧 | 🚧 | 🚧 | X Window Dump |
| PBM/PGM/PPM | `.pbm`, `.pgm`, `.ppm`, `.pnm` | 🔄 | 🚧 | 🚧 | 🚧 | Netpbm formats |
| PAM | `.pam` | 🔄 | 🚧 | 🚧 | 🚧 | Portable Arbitrary Map |
| PFM | `.pfm` | 🔄 | 🚧 | 🚧 | 🚧 | Portable Float Map |
| Photo CD | `.pcd` | 🔄 | 🚧 | 🚧 | 🚧 | Kodak Photo CD |
| WBMP | `.wbmp` | 🔄 | 🚧 | 🚧 | 🚧 | Wireless Bitmap |
| WPG | `.wpg` | 🔄 | 🚧 | 🚧 | 🚧 | WordPerfect Graphics |
| MNG | `.mng` | 🔄 | 🚧 | 🚧 | 🚧 | Multiple-image Network Graphics |
| Cursor | `.cur` | 🔄 | 🚧 | 🚧 | 🚧 | Windows cursors |
| PES | `.pes` | 🔄 | 🚧 | 🚧 | 🚧 | Embroidery format |
| Windows Meta | `.wmf` | 🔄 | 🚧 | 🚧 | 🚧 | Windows Metafile |
| DjVu | `.djvu`, `.djv` | 🔄 | 🚧 | 🚧 | 🚧 | Scanned documents |

---

## 3. Document & Office Formats

### 3.1 PDF

| Format | Extensions | Android | iOS | Desktop | Web | Notes |
|--------|-----------|---------|-----|---------|-----|-------|
| PDF | `.pdf` | ✅ | 🚧 | 🚧 | 🚧 | Native rendering via PDF.js on Android |

### 3.2 Microsoft Office (Modern)

| Format | Extensions | Android | iOS | Desktop | Web | Notes |
|--------|-----------|---------|-----|---------|-----|-------|
| Word (OOXML) | `.docx`, `.docm`, `.dotx`, `.dotm` | ✅ | 🚧 | 🚧 | 🚧 | Full document structure |
| Excel (OOXML) | `.xlsx`, `.xlsm`, `.xlsb` | ✅ | 🚧 | 🚧 | 🚧 | Multi-sheet virtualized grid |
| PowerPoint (OOXML) | `.pptx`, `.pptm`, `.potx` | ✅ | 🚧 | 🚧 | 🚧 | Slide-by-slide with text extraction |

### 3.3 Microsoft Office (Legacy)

| Format | Extensions | Android | iOS | Desktop | Web | Notes |
|--------|-----------|---------|-----|---------|-----|-------|
| Word 97-2003 | `.doc` | ⚠️ | 🚧 | 🚧 | 🚧 | Binary format, basic text extraction |
| Excel 97-2003 | `.xls` | ✅ | 🚧 | 🚧 | 🚧 | CFB-based spreadsheet |
| PowerPoint 97-2003 | `.ppt` | ✅ | 🚧 | 🚧 | 🚧 | Binary format, text extraction |
| Rich Text Format | `.rtf` | 🔄 | 🚧 | 🚧 | 🚧 | Cross-platform document format |

### 3.4 OpenDocument (ODF)

| Format | Extensions | Android | iOS | Desktop | Web | Notes |
|--------|-----------|---------|-----|---------|-----|-------|
| ODF Text | `.odt`, `.ott` | ✅ | 🚧 | 🚧 | 🚧 | LibreOffice/OpenOffice documents |
| ODF Spreadsheet | `.ods` | ⚠️ | 🚧 | 🚧 | 🚧 | Needs test file fix |
| ODF Presentation | `.odp` | ✅ | 🚧 | 🚧 | 🚧 | Slide deck with text extraction |

### 3.5 Apple iWork

| Format | Extensions | Android | iOS | Desktop | Web | Notes |
|--------|-----------|---------|-----|---------|-----|-------|
| Pages | `.pages` | 🔄 | 🚧 | 🚧 | 🚧 | Apple word processor |
| Numbers | `.numbers` | 🔄 | 🚧 | 🚧 | 🚧 | Apple spreadsheet |
| Keynote | `.key` | 🔄 | 🚧 | 🚧 | 🚧 | Apple presentations |

---

## 4. eBook Formats

| Format | Extensions | Android | iOS | Desktop | Web | Notes |
|--------|-----------|---------|-----|---------|-----|-------|
| EPUB | `.epub` | 📦 | 🚧 | 🚧 | 🚧 | Available in release builds only |
| MOBI | `.mobi` | 🔄 | 🚧 | 🚧 | 🚧 | Amazon Kindle format |
| AZW3 | `.azw3` | 🔄 | 🚧 | 🚧 | 🚧 | Kindle Format 8 |
| FictionBook | `.fb2` | 🔄 | 🚧 | 🚧 | 🚧 | Russian eBook standard |
| PalmDoc | `.lrf`, `.pdb`, `.snb` | 🔄 | 🚧 | 🚧 | 🚧 | Legacy Palm formats |

---

## 5. Font Formats

| Format | Extensions | Android | iOS | Desktop | Web | Notes |
|--------|-----------|---------|-----|---------|-----|-------|
| TrueType | `.ttf` | 📦 | 🚧 | 🚧 | 🚧 | Available in release builds |
| OpenType | `.otf` | 📦 | 🚧 | 🚧 | 🚧 | Available in release builds |
| WOFF | `.woff` | 📦 | 🚧 | 🚧 | 🚧 | Web Open Font Format |
| WOFF2 | `.woff2` | 📦 | 🚧 | 🚧 | 🚧 | WOFF version 2 |
| PostScript | `.pfb`, `.ps` | 📦 | 🚧 | 🚧 | 🚧 | Type 1 fonts |
| CFF | `.cff` | 📦 | 🚧 | 🚧 | 🚧 | Compact Font Format |
| dfont | `.dfont` | 📦 | 🚧 | 🚧 | 🚧 | macOS font suitcase |
| SFD | `.sfd` | 📦 | 🚧 | 🚧 | 🚧 | FontForge source |

---

## 6. Archive Formats

| Format | Extensions | Android | iOS | Desktop | Web | Notes |
|--------|-----------|---------|-----|---------|-----|-------|
| ZIP | `.zip` | 📦 | 🚧 | 🚧 | 🚧 | Available in release builds |
| TAR | `.tar` | 📦 | 🚧 | 🚧 | 🚧 | Unix tape archive |
| TAR.GZ | `.tgz`, `.tar.gz` | 📦 | 🚧 | 🚧 | 🚧 | Gzip compressed tar |
| GZ | `.gz` | 📦 | 🚧 | 🚧 | 🚧 | Gzip single-file compression |
| 7-Zip | `.7z` | 📦 | 🚧 | 🚧 | 🚧 | High compression ratio |
| RAR | `.rar` | ❌ | ❌ | ❌ | ❌ | Proprietary format - not supported |

---

## 7. Media Formats

### 7.1 Video Formats

| Format | Extensions | Android | iOS | Desktop | Web | Notes |
|--------|-----------|---------|-----|---------|-----|-------|
| MP4 | `.mp4`, `.m4v` | ✅ | 🚧 | 🚧 | 🚧 | Native HTML5 video player |
| WebM | `.webm` | ✅ | 🚧 | 🚧 | 🚧 | Open web format |
| Matroska | `.mkv` | ✅ | 🚧 | 🚧 | 🚧 | Container format |
| QuickTime | `.mov` | ✅ | 🚧 | 🚧 | 🚧 | Apple video format |
| AVI | `.avi` | ✅ | 🚧 | 🚧 | 🚧 | Legacy Windows format |
| MPEG | `.mpg`, `.mpeg`, `.m2v` | ✅ | 🚧 | 🚧 | 🚧 | MPEG-1/2 video |
| 3GPP | `.3gp` | ✅ | 🚧 | 🚧 | 🚧 | Mobile video format |
| WMV | `.wmv`, `.asf` | ✅ | 🚧 | 🚧 | 🚧 | Windows Media Video |
| FLV | `.flv` | ✅ | 🚧 | 🚧 | 🚧 | Flash video |
| F4V | `.f4v` | ✅ | 🚧 | 🚧 | 🚧 | Flash MP4 |
| HEVC | `.hevc` | ✅ | 🚧 | 🚧 | 🚧 | H.265 video |
| M2TS | `.m2ts` | ✅ | 🚧 | 🚧 | 🚧 | Blu-ray BDAV |
| MJPEG | `.mjpeg` | ✅ | 🚧 | 🚧 | 🚧 | Motion JPEG |
| MTS | `.mts` | ✅ | 🚧 | 🚧 | 🚧 | AVCHD video |
| MXF | `.mxf` | ✅ | 🚧 | 🚧 | 🚧 | Professional broadcast |
| OGV | `.ogv` | ✅ | 🚧 | 🚧 | 🚧 | Ogg video |
| RealMedia | `.rm` | ✅ | 🚧 | 🚧 | 🚧 | Legacy streaming |
| SWF | `.swf` | ✅ | 🚧 | 🚧 | 🚧 | Flash animation |
| VOB | `.vob` | ✅ | 🚧 | 🚧 | 🚧 | DVD video |
| WTV | `.wtv` | ✅ | 🚧 | 🚧 | 🚧 | Windows recorded TV |
| TS | `.ts` | ✅ | 🚧 | 🚧 | 🚧 | MPEG transport stream |

### 7.2 Audio Formats

| Format | Extensions | Android | iOS | Desktop | Web | Notes |
|--------|-----------|---------|-----|---------|-----|-------|
| MP3 | `.mp3` | ✅ | 🚧 | 🚧 | 🚧 | Native HTML5 audio player |
| M4A | `.m4a`, `.m4r` | ✅ | 🚧 | 🚧 | 🚧 | Apple audio format |
| AAC | `.aac` | ✅ | 🚧 | 🚧 | 🚧 | Advanced Audio Coding |
| FLAC | `.flac` | ✅ | 🚧 | 🚧 | 🚧 | Lossless compression |
| OGG | `.ogg`, `.oga` | ✅ | 🚧 | 🚧 | 🚧 | Ogg Vorbis |
| WAV | `.wav` | ✅ | 🚧 | 🚧 | 🚧 | Uncompressed audio |
| WMA | `.wma` | ✅ | 🚧 | 🚧 | 🚧 | Windows Media Audio |
| Opus | `.opus` | ✅ | 🚧 | 🚧 | 🚧 | Modern codec |
| AIFF | `.aiff` | ✅ | 🚧 | 🚧 | 🚧 | Apple audio format |
| AU | `.au` | ✅ | 🚧 | 🚧 | 🚧 | Sun/Unix audio |
| MP2 | `.mp2` | ✅ | 🚧 | 🚧 | 🚧 | MPEG-1 Audio Layer II |
| AC3 | `.ac3` | ✅ | 🚧 | 🚧 | 🚧 | Dolby Digital |
| DTS | `.dts` | ✅ | 🚧 | 🚧 | 🚧 | Digital Theater Systems |
| TTA | `.tta` | ✅ | 🚧 | 🚧 | 🚧 | True Audio lossless |
| WV | `.wv` | ✅ | 🚧 | 🚧 | 🚧 | WavPack |
| SPX | `.spx` | ✅ | 🚧 | 🚧 | 🚧 | Speex codec |
| RealAudio | `.ra` | ✅ | 🚧 | 🚧 | 🚧 | Legacy streaming |
| AMB | `.amb` | ✅ | 🚧 | 🚧 | 🚧 | Ambisonic audio |
| AVR | `.avr` | ✅ | 🚧 | 🚧 | 🚧 | Audio Visual Research |
| CAF | `.caf` | ✅ | 🚧 | 🚧 | 🚧 | Core Audio Format |
| CDDA | `.cdda` | ✅ | 🚧 | 🚧 | 🚧 | CD Digital Audio |
| CVS/CVSD/CVU | `.cvs`, `.cvsd`, `.cvu` | ✅ | 🚧 | 🚧 | 🚧 | Continuously Variable Slope |
| DVMS | `.dvms` | ✅ | 🚧 | 🚧 | 🚧 | DVMS audio |
| FAP | `.fap` | ✅ | 🚧 | 🚧 | 🚧 | FAP audio |
| FSSD | `.fssd` | ✅ | 🚧 | 🚧 | 🚧 | FSSD format |
| GSRT | `.gsrt` | ✅ | 🚧 | 🚧 | 🚧 | Grandstream ring tone |
| HCOM | `.hcom` | ✅ | 🚧 | 🚧 | 🚧 | Macintosh HCOM |
| HTK | `.htk` | ✅ | 🚧 | 🚧 | 🚧 | HMM Toolkit |
| IMA | `.ima` | ✅ | 🚧 | 🚧 | 🚧 | IMA ADPCM |
| IRCAM | `.ircam` | ✅ | 🚧 | 🚧 | 🚧 | IRCAM audio |
| MAUD | `.maud` | ✅ | 🚧 | 🚧 | 🚧 | Amiga audio |
| NIST | `.nist` | ✅ | 🚧 | 🚧 | 🚧 | NIST SPHERE |
| PAF | `.paf` | ✅ | 🚧 | 🚧 | 🚧 | Ensoniq PARIS |
| PRC | `.prc` | ✅ | 🚧 | 🚧 | 🚧 | Psion audio |
| PVF | `.pvf` | ✅ | 🚧 | 🚧 | 🚧 | Portable Voice Format |
| SD2 | `.sd2` | ✅ | 🚧 | 🚧 | 🚧 | Sound Designer II |
| SLN | `.sln` | ✅ | 🚧 | 🚧 | 🚧 | Asterisk raw audio |
| SMP | `.smp` | ✅ | 🚧 | 🚧 | 🚧 | Sample audio |
| SND/SNDR/SNDT | `.snd`, `.sndr`, `.sndt` | ✅ | 🚧 | 🚧 | 🚧 | Various sound formats |
| SOU | `.sou` | ✅ | 🚧 | 🚧 | 🚧 | SBStudio II |
| SPH | `.sph` | ✅ | 🚧 | 🚧 | 🚧 | NIST SPHERE |
| TXW | `.txw` | ✅ | 🚧 | 🚧 | 🚧 | Yamaha TX-16W |
| VMS | `.vms` | ✅ | 🚧 | 🚧 | 🚧 | VMS audio |
| VOC | `.voc` | ✅ | 🚧 | 🚧 | 🚧 | Creative Voice |
| VOX | `.vox` | ✅ | 🚧 | 🚧 | 🚧 | Dialogic ADPCM |
| W64 | `.w64` | ✅ | 🚧 | 🚧 | 🚧 | Sony Wave64 |
| WVE | `.wve` | ✅ | 🚧 | 🚧 | 🚧 | Psion wave |
| 8SVX | `.8svx` | ✅ | 🚧 | 🚧 | 🚧 | Amiga 8-bit sound |

---

## Summary Statistics

### Total Format Count by Category

| Category | Formats | Android Status |
|----------|---------|----------------|
| Text & Code | 80+ | ✅ Fully supported |
| Images | 60+ | ✅ Common formats tested, esoteric formats implemented |
| Documents | 15 | ✅ DOCX/XLSX/PPTX/XLS/PPT/ODT/ODP tested, DOC/ODS need fixes |
| eBooks | 5 | 📦 EPUB in release builds, others placeholder |
| Fonts | 8 | 📦 Release builds only |
| Archives | 6 | 📦 Release builds only (RAR not supported) |
| Video | 22 | ✅ Native HTML5 playback |
| Audio | 55+ | ✅ Native HTML5 playback |
| **Total** | **250+** | |

### Feature Availability by Build Type

**Development Builds** (default, `fmt-text-only` feature):
- ✅ Text, code, markdown, JSON, CSV
- ✅ Images (native WebView)
- ✅ PDF (native WebView with PDF.js)
- ✅ Video/Audio (native HTML5)
- ❌ Office documents (placeholder)
- ❌ eBooks (placeholder)
- ❌ Archives (placeholder)
- ❌ Fonts (placeholder)

**Release Builds** (`fmt-everything-lite` feature):
- ✅ All text & code formats
- ✅ All image formats
- ✅ All office documents (DOCX, XLSX, PPTX, XLS, PPT, ODT, ODS, ODP, DOC)
- ✅ EPUB eBooks
- ✅ All archives (ZIP, TAR, 7Z, TAR.GZ)
- ✅ Font metadata viewing
- ✅ All media formats

### Platform Testing Status

- **Android (API 34)**: Primary platform, 46 test files validated
- **iOS**: Not yet tested
- **Desktop** (Windows/Linux/macOS): Not yet tested
- **Web**: Future expansion planned

---

## Notes

1. **Magic Byte Detection**: Most formats use magic byte sniffing for accurate detection even without file extensions
2. **WebView Rendering**: Images, PDFs, and media use native WebView capabilities for optimal performance
3. **Text Encoding**: Automatic detection of UTF-8, Latin-1, and other encodings via `chardetng`
4. **Large Files**: 32 MB memory cap for in-memory parsing; larger files show "Open with..." prompt
5. **Encryption**: Password-protected files (encrypted ZIP, Office docs) show appropriate error messages

## Related Documentation

- [ADR 0002: Format Detection Strategy](../docs/adr/0002-format-detection.md)
- [ADR 0004: Unsupported Formats](../docs/adr/0004-unsupported-formats.md)
- [ADR 0010: Media Rendering](../docs/adr/0010-media-rendering.md)
- [Testing Guide](../docs/TESTING.md)

