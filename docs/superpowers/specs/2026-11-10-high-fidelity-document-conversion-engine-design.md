# High-Fidelity Document Conversion Engine Design

## 1. Overview & Objective

Build a production-grade document reconstruction engine for FileUC that converts PDF documents into genuinely editable, high-fidelity Microsoft Office formats:
1. **PDF → PPTX** (PowerPoint Presentation)
2. **PDF → DOCX** (Word Document)
3. **PDF → XLSX** (Excel Spreadsheet)

The engine rejects full-page screenshot rasterization as a conversion strategy. Output documents must contain genuine editable text runs, native tables, shapes, lines, and extracted images with precise spatial and typographic attributes.

---

## 2. Architectural Pipeline

```
PDF File
   │
   ▼
[ PDFium Low-Level Extractor ]
   │ (Glyphs, Text Matrices, Font Metadata, Vector Paths, Raster Bitmaps)
   ▼
[ Layout & Document Structure Analyzer ]
   │ (Recursive XY-Cut Reading Order, Line Clustering, Lattice/Stream Table Detection)
   ▼
[ Shared Document Intermediate Representation (IR) ]
   │ (Pages, Blocks, Runs, Tables, Cells, Images, Shapes)
   ├──────────────────────────┬──────────────────────────┐
   ▼                          ▼                          ▼
[ PPTX Emitter ]       [ DOCX Emitter ]          [ XLSX Emitter ]
(Canvas & Shapes)      (Flow + Anchored)         (Sheet & Tables)
   │                          │                          │
   └──────────────────────────┼──────────────────────────┘
                              ▼
               [ Automated Validation Harness ]
            (Headless Render vs PDFium Ground-Truth)
```

---

## 3. Shared Intermediate Representation (IR)

The Intermediate Representation is decoupled from both the PDF parser and the Office emitters. It resides in `crates/fileuc-convert/src/ir/`.

### Core Data Models

```rust
#[derive(Debug, Clone)]
pub struct DocumentIR {
    pub title: Option<String>,
    pub pages: Vec<PageIR>,
}

#[derive(Debug, Clone)]
pub struct PageIR {
    pub page_number: usize,
    pub width_pt: f32,
    pub height_pt: f32,
    pub orientation: PageOrientation,
    pub blocks: Vec<BlockIR>,
    pub tables: Vec<TableIR>,
    pub images: Vec<ImageIR>,
    pub shapes: Vec<ShapeIR>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PageOrientation {
    Portrait,
    Landscape,
}

#[derive(Debug, Clone)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

#[derive(Debug, Clone)]
pub enum BlockIR {
    Paragraph(ParagraphIR),
    Heading(HeadingIR),
}

#[derive(Debug, Clone)]
pub struct ParagraphIR {
    pub bounds: Rect,
    pub alignment: TextAlignment,
    pub line_spacing_pt: f32,
    pub space_after_pt: f32,
    pub lines: Vec<TextLineIR>,
}

#[derive(Debug, Clone)]
pub struct TextLineIR {
    pub bounds: Rect,
    pub baseline_y: f32,
    pub runs: Vec<TextRunIR>,
}

#[derive(Debug, Clone)]
pub struct TextRunIR {
    pub text: String,
    pub bounds: Rect,
    pub font_family: String,
    pub font_size_pt: f32,
    pub is_bold: bool,
    pub is_italic: bool,
    pub color_rgb: [u8; 3],
    pub char_spacing_pt: f32,
}

#[derive(Debug, Clone)]
pub struct TableIR {
    pub bounds: Rect,
    pub rows: Vec<TableRowIR>,
    pub column_widths_pt: Vec<f32>,
    pub cells: Vec<TableCellIR>,
}

#[derive(Debug, Clone)]
pub struct TableCellIR {
    pub row: usize,
    pub col: usize,
    pub row_span: usize,
    pub col_span: usize,
    pub bounds: Rect,
    pub background_rgb: Option<[u8; 3]>,
    pub borders: CellBorders,
    pub text_runs: Vec<TextRunIR>,
}

#[derive(Debug, Clone, Default)]
pub struct CellBorders {
    pub top: Option<BorderLine>,
    pub right: Option<BorderLine>,
    pub bottom: Option<BorderLine>,
    pub left: Option<BorderLine>,
}

#[derive(Debug, Clone)]
pub struct BorderLine {
    pub width_pt: f32,
    pub color_rgb: [u8; 3],
}

#[derive(Debug, Clone)]
pub struct ImageIR {
    pub bounds: Rect,
    pub data: Vec<u8>,
    pub format: ImageFormat,
}

#[derive(Debug, Clone)]
pub struct ShapeIR {
    pub bounds: Rect,
    pub shape_type: ShapeType,
    pub stroke_color: Option<[u8; 3]>,
    pub stroke_width_pt: f32,
    pub fill_color: Option<[u8; 3]>,
}
```

---

## 4. Extraction & Reconstruction Algorithms

### 4.1 Recursive XY-Cut (Reading-Order Segmentation)
To avoid stitching columns horizontally across multi-column pages:
1. Compute the vertical projection profile of glyph bounding boxes across the page width.
2. Find deep vertical whitespace valleys (gutters) larger than the average inter-word spacing threshold.
3. Recursively split the page into column partitions.
4. Within each partition, compute the horizontal projection profile to detect paragraph breaks.
5. Sort blocks in topological reading order: top-down, column-by-column.

### 4.2 Lattice Table Detection (Bordered Tables)
1. Collect all vector path segments from PDFium (`FPDFPage_GetObject`).
2. Filter for horizontal and vertical lines with thickness $\le 3.0\text{ pt}$.
3. Snap line endpoints within a 2.0 pt tolerance grid.
4. Construct an intersection graph; find closed rectangular cycles to form table cells.
5. Cluster cell polygons into table matrices, identifying `row_span` and `col_span`.
6. Assign text items whose bounding boxes fall within cell boundaries to the respective cell.

### 4.3 Stream Table Detection (Borderless Tables)
1. For text regions where columns align vertically across multiple consecutive rows without vector lines:
2. Identify aligned text start and end x-coordinates forming vertical columns.
3. Ensure horizontal gaps between column columns exceed 12 pt.
4. Reconstruct as borderless `w:tbl` or Excel cells.

---

## 5. Target Format Emitters

### 5.1 PDF → PPTX (PowerPoint)
- Canvas-based mapping: 1 PDF page = 1 PPTX slide (`p:sld`).
- Coordinate transform: Points to EMUs ($1\text{ pt} = 12700\text{ EMUs}$).
- Text boxes: `p:sp` with `p:txBody` containing `a:p` and `a:r` runs.
- Images: `p:pic` with `a:blip` referencing `ppt/media/`.
- Native PowerPoint tables: `a:tbl` with `a:tr` and `a:tc` for extracted tables.
- Vector lines & rectangles: `p:sp` with `a:prstGeom prst="rect"` or line geometry.

### 5.2 PDF → DOCX (Word)
- Hybrid flow + anchored layout:
  - Regular prose emits flowing paragraphs (`w:p`) with exact line spacing and character formatting.
  - Tables emit native `w:tbl` with `w:tblGrid`, `w:gridCol`, and cell properties `w:tcPr` (`w:tcBorders`, `w:shd`).
  - Decorative side elements, cards, and floating graphics emit `wp:anchor` drawing frames to maintain exact placement without disrupting paragraph flow.

### 5.3 PDF → XLSX (Excel)
- Table-centric extraction:
  - Each detected table maps to a structured worksheet grid.
  - Cell data types inferred:
    - Numeric: `1234.56` $\to$ cell type `n` with number formatting.
    - Currency / Percentage: Parsed to numeric value with format string (`$#,##0.00`, `0.00%`).
    - Date: ISO-8601 / standard date patterns.
    - String: Shared strings table (`xl/sharedStrings.xml`) or inline strings (`t="inlineStr"`).
  - Cross-page table continuation: Stitches tables spanning page boundaries when column widths and headers match.

---

## 6. Automated Validation Harness

- Renders input PDF pages via PDFium at 150 DPI as the ground-truth reference bitmap.
- In test mode, renders output files into bitmaps using headless LibreOffice or COM.
- Computes:
  1. Structural layout comparison (element count, bounding box deviation).
  2. Visual comparison via SSIM (Structural Similarity Index) and normalized pixel diff.
  3. Text editability verification (verifies XML contains editable `w:t` / `a:t` text nodes rather than flat bitmaps).
