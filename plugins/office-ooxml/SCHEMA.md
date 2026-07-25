# Office OOXML Plugin Output Schema

This document describes the structured JSON produced by the `office-ooxml` plugin
for each supported format. Frontend viewers consume these schemas and render
progressively: fields that are absent fall back to simpler rendering paths.

## Common envelope

Every plugin render returns a JSON object with these top-level fields:

| Field | Type | Always present | Description |
|---|---|---|---|
| `kind` | `string` | yes | `"docx"`, `"xlsx"`, or `"pptx"` |
| `byte_len` | `number` | yes | original file size in bytes |
| `warnings` | `string[]` | yes | non-fatal issues detected during inspection |
| `fidelity` | `Fidelity` | yes | summary of what was extracted and what is missing |

### `Fidelity`

| Field | Type | Description |
|---|---|---|
| `level` | `string` | current fidelity tier, e.g. `"text+structure"`, `"text+layout"` |
| `supports` | `string[]` | features this output actually contains |
| `missing` | `string[]` | known features not yet extracted |

## DOCX

```json
{
  "kind": "docx",
  "blocks": [Block],
  "byte_len": 12345,
  "warnings": [],
  "fidelity": { "level": "text+structure", "supports": [...], "missing": [...] },
  "search_text": ["string", "..."]
}
```

### DOCX `Block`

| `kind` | Fields | Description |
|---|---|---|
| `paragraph` | `text: string`, `heading: number?` | normal paragraph or heading 1–6 |
| `list-item` | `text: string`, `level: number` | numbered or bulleted list item |
| `table` | `rows: string[][]` | plain-text table rows |

`search_text` is a flat array of all paragraph/list/table cell strings in
document order, useful for client-side search without re-parsing.

## XLSX

```json
{
  "kind": "xlsx",
  "sheets": [Sheet],
  "byte_len": 12345,
  "warnings": [],
  "fidelity": { "level": "text+structure", "supports": [...], "missing": [...] }
}
```

### XLSX `Sheet`

| Field | Type | Description |
|---|---|---|
| `name` | `string` | sheet tab name |
| `header` | `string[]` | first row, treated as header |
| `preview_rows` | `string[][]` | up to 200 data rows after the header |
| `total_rows_hint` | `number` | total data rows including header |
| `total_cols_hint` | `number` | widest row length |

## PPTX

```json
{
  "kind": "pptx",
  "slide_count": 3,
  "slides": [Slide],
  "byte_len": 12345,
  "asset_path": "",
  "warnings": [],
  "fidelity": { "level": "text+layout", "supports": [...], "missing": [...] }
}
```

### PPTX `Slide`

| Field | Type | Description |
|---|---|---|
| `title` | `string` | first text element text (legacy field) |
| `body` | `string` | remaining text elements joined by newlines (legacy) |
| `width` | `number` | slide width in EMUs |
| `height` | `number` | slide height in EMUs |
| `elements` | `Element[]` | positioned layout elements |

### PPTX `Element`

| `kind` | Fields | Description |
|---|---|---|
| `text` | `text: string`, `x/y/w/h: number`, `font_size: number`, `paragraphs?: PptxParagraph[]` | text shape |
| `image` | `src: string (data: URL)`, `x/y/w/h: number` | embedded image |

### PPTX `Paragraph` (optional `element.paragraphs`)

| Field | Type | Description |
|---|---|---|
| `runs` | `PptxRun[]` | rich text runs within the paragraph |
| `bullet` | `boolean?` | true if paragraph has a bullet marker |

### PPTX `Run` (within `paragraph.runs`)

| Field | Type | Description |
|---|---|---|
| `text` | `string` | text content of this run |
| `bold` | `boolean?` | true if bold |
| `italic` | `boolean?` | true if italic |
| `underline` | `boolean?` | true if underlined |
| `font_size` | `number?` | font size in points |
| `color` | `string?` | explicit text color (hex, e.g. `#FF0000`) |

### PPTX `Background` (optional `slide.background`)

| Field | Type | Description |
|---|---|---|
| `type` | `string` | `"solid"` or `"gradient"` |
| `color` | `string?` | hex color for solid fills |
| `gradient` | `string?` | CSS gradient string |

## Backward compatibility

- Older frontends that only read `kind` + `blocks`/`sheets`/`slides` continue to
  work because all legacy fields remain present.
- Newer frontends should check `fidelity.missing` before assuming a feature is
  available.
- `warnings` may grow over time as the plugin detects more unsupported features.
