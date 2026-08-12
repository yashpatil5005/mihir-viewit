# ViewIt — Store Listing Draft

## Short description
Open everyday files offline — documents, spreadsheets, slides, archives, fonts, media, ebooks, and more.

## Full description
ViewIt is an offline-first universal file viewer for Android.

Open commonly used files in one place: text and code, images, PDFs, audio/video,
DOCX/XLSX/PPTX, OpenDocument, Apple iWork, ebooks, archives, and fonts. The built-in
viewer provides a lightweight offline preview; optional signed plugins add richer
rendering, extraction, editing, or custom playback.

Highlights:
- local/offline document rendering — files are not uploaded;
- faithful PPTX rendering through an optional offline plugin;
- spreadsheet sheets, formulas, merged cells, and frozen panes;
- archive listings and optional extraction;
- custom player and editor plugin bases;
- plugin downloads are signed and checksum-verified;
- Android system pickers for opening, saving, and folder access;
- small base app — advanced capabilities are downloaded only when wanted.

## Privacy summary
Documents stay on-device. Network access is used for the signed optional-plugin catalog
and user-requested plugin downloads. See `docs/PRIVACY.md`.

## Review notes
ViewIt registers file associations because opening user-selected/shared files is its
primary purpose. It uses content URI grants and Android's Storage Access Framework; no
broad storage permission is required.
