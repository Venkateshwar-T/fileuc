# High-Fidelity Document Conversion Engine Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a production-grade, multi-format document reconstruction engine in FileUC that converts PDFs into high-fidelity, genuinely editable DOCX, PPTX, and XLSX files.

**Architecture:** A modular pipeline where PDFium extracts low-level glyphs, text matrices, vector paths, and images; a document structure analyzer runs recursive XY-Cut reading-order segmentation and lattice/stream table detection; a shared Intermediate Representation (IR) decouples parsing from generation; and dedicated emitters construct standards-compliant OOXML packages.

**Tech Stack:** Rust 2021, Google PDFium (`pdfium-render`), `zip-rs`, `image`, Windows Media OCR.

**Spec:** `docs/superpowers/specs/2026-11-10-high-fidelity-document-conversion-engine-design.md`

## Global Constraints

- No full-page screenshot/rasterization shortcuts; output must contain editable text runs, native tables, and shapes.
- Strictly no AGPL or GPL dependencies (MuPDF, Poppler are prohibited); use PDFium (BSD/Apache) and pure-Rust OOXML zip generation.
- Zero extra memory footprint: must preserve FileUC's <2 MB idle RAM on desktop.
- OOXML files must open without corruption warnings in Microsoft Office and LibreOffice.

## Review Focus

1. **Multi-Column Reading Order:** Text in 2-column or 3-column articles must not intertwine horizontally; XY-Cut vertical gutter segmentation must preserve column order.
2. **Table Border Alignment:** Vector strokes with slight sub-pixel misalignments (up to 2.0 pt) must snap cleanly into continuous grid cells.
3. **Empty / Scanned PDF Detection:** When a page has zero text objects and image area > 80%, route gracefully through high-resolution OCR without panicking.
4. **Font Sizing & Overflow:** Text bounding boxes in PowerPoint and Word must account for font family metrics so lines do not wrap prematurely.
5. **Special XML Character Escaping:** All text strings in OOXML must escape `&`, `<`, `>`, `"`, and control characters (`\x00-\x1F` except tab and newline).

---

### Task 1: Shared Document Intermediate Representation (IR) Data Types

**Files:**
- Create: `crates/fileuc-convert/src/ir/mod.rs`
- Create: `crates/fileuc-convert/src/ir/types.rs`
- Test: `crates/fileuc-convert/tests/ir_types_test.rs`

**Interfaces:**
- Produces: `DocumentIR`, `PageIR`, `BlockIR`, `ParagraphIR`, `TextLineIR`, `TextRunIR`, `TableIR`, `TableCellIR`, `ImageIR`, `ShapeIR`, `Rect`, `PageOrientation`

- [ ] **Step 1: Write the failing test**

```rust
// crates/fileuc-convert/tests/ir_types_test.rs
use fileuc_convert::ir::types::*;

#[test]
fn test_document_ir_creation() {
    let mut doc = DocumentIR::new(Some("Sample Document".to_string()));
    assert_eq!(doc.title.as_deref(), Some("Sample Document"));
    assert!(doc.pages.is_empty());

    let page = PageIR::new(1, 595.0, 842.0, PageOrientation::Portrait);
    doc.pages.push(page);
    assert_eq!(doc.pages.len(), 1);
    assert_eq!(doc.pages[0].width_pt, 595.0);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --test ir_types_test`
Expected: FAIL with "unresolved import `fileuc_convert::ir`"

- [ ] **Step 3: Implement IR types in `crates/fileuc-convert/src/ir/types.rs` and `mod.rs`**

Define `DocumentIR`, `PageIR`, `BlockIR`, `ParagraphIR`, `TextLineIR`, `TextRunIR`, `TableIR`, `TableCellIR`, `ImageIR`, `ShapeIR`, `Rect`, and `PageOrientation` with constructor helpers.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --test ir_types_test`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/fileuc-convert/src/ir crates/fileuc-convert/tests/ir_types_test.rs
git commit -m "feat(ir): define shared document intermediate representation data types"
```

---

### Task 2: PDFium Low-Level Geometry & Raw Content Extractor

**Files:**
- Create: `crates/fileuc-convert/src/ir/pdfium_extractor.rs`
- Modify: `crates/fileuc-convert/src/ir/mod.rs`
- Test: `crates/fileuc-convert/tests/pdfium_extractor_test.rs`

**Interfaces:**
- Consumes: `pdfium_render::prelude::PdfDocument`
- Produces: `pub fn extract_raw_page_elements(page: &PdfPage) -> Result<RawPageContent, ConvertError>`
  where `RawPageContent` contains raw text objects with exact bounds, font details, colors, path objects, and images.

- [ ] **Step 1: Write the failing test**

```rust
// crates/fileuc-convert/tests/pdfium_extractor_test.rs
use fileuc_convert::ir::pdfium_extractor::extract_raw_page_elements;
use fileuc_convert::pdfium::init_pdfium;

#[test]
fn test_extract_raw_elements_from_dummy_pdf() {
    let pdfium = init_pdfium().expect("pdfium loaded");
    // Verify function signature exists and handles pages cleanly
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --test pdfium_extractor_test`
Expected: FAIL with "unresolved import"

- [ ] **Step 3: Implement `extract_raw_page_elements` in `crates/fileuc-convert/src/ir/pdfium_extractor.rs`**

Iterate over `page.objects()`, extract text glyph bounds, font family name, size, bold/italic flags, RGB fill colors, vector path bounding boxes and stroke dimensions, and raw image bytes.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --test pdfium_extractor_test`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/fileuc-convert/src/ir/pdfium_extractor.rs crates/fileuc-convert/tests/pdfium_extractor_test.rs
git commit -m "feat(ir): implement low-level PDFium geometric and content extractor"
```

---

### Task 3: Layout Structure Analyzer (Recursive XY-Cut & Line Clustering)

**Files:**
- Create: `crates/fileuc-convert/src/ir/layout_analyzer.rs`
- Modify: `crates/fileuc-convert/src/ir/mod.rs`
- Test: `crates/fileuc-convert/tests/layout_analyzer_test.rs`

**Interfaces:**
- Consumes: `RawPageContent` from Task 2
- Produces: `pub fn analyze_page_layout(raw: RawPageContent, page_w: f32, page_h: f32) -> PageIR`
  Reconstructs reading order via XY-Cut, clusters horizontal lines into paragraphs with consistent line spacing, and calculates paragraph alignments.

- [ ] **Step 1: Write the failing test**

```rust
// crates/fileuc-convert/tests/layout_analyzer_test.rs
use fileuc_convert::ir::layout_analyzer::*;
use fileuc_convert::ir::types::*;

#[test]
fn test_line_clustering_and_xy_cut() {
    // Tests that two items at same Y become a single line, and items spaced vertically become separate lines/paragraphs.
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --test layout_analyzer_test`
Expected: FAIL

- [ ] **Step 3: Implement XY-Cut and Line Clustering in `crates/fileuc-convert/src/ir/layout_analyzer.rs`**

Implement vertical and horizontal projection profile segmentation; cluster words within $dy \le 3.0\text{ pt}$ into `TextLineIR`; group lines within $1.5 \times \text{font\_size}$ into `ParagraphIR`.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --test layout_analyzer_test`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/fileuc-convert/src/ir/layout_analyzer.rs crates/fileuc-convert/tests/layout_analyzer_test.rs
git commit -m "feat(ir): implement recursive XY-Cut reading order and paragraph clustering"
```

---

### Task 4: Lattice & Stream Table Detection Engine

**Files:**
- Create: `crates/fileuc-convert/src/ir/table_detector.rs`
- Modify: `crates/fileuc-convert/src/ir/mod.rs`
- Test: `crates/fileuc-convert/tests/table_detector_test.rs`

**Interfaces:**
- Consumes: `raw_paths: &[RawPath]`, `text_items: &[RawTextItem]`
- Produces: `pub fn detect_tables(paths: &[RawPath], items: &mut Vec<RawTextItem>) -> Vec<TableIR>`
  Snaps vector line endpoints within 2.0 pt tolerance, identifies rectangular grid cells, assigns contained text runs to cells, and removes assigned text items from normal paragraph flow.

- [ ] **Step 1: Write the failing test**

```rust
// crates/fileuc-convert/tests/table_detector_test.rs
use fileuc_convert::ir::table_detector::*;

#[test]
fn test_detect_grid_table() {
    // Test with intersecting horizontal and vertical line segments forming a 2x2 grid
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --test table_detector_test`
Expected: FAIL

- [ ] **Step 3: Implement table detection in `crates/fileuc-convert/src/ir/table_detector.rs`**

Find all horizontal and vertical stroke lines; snap endpoints within 2.0 pt; compute intersection matrix; construct rectangular cells; calculate column widths and row heights; capture cell background fills and borders.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --test table_detector_test`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/fileuc-convert/src/ir/table_detector.rs crates/fileuc-convert/tests/table_detector_test.rs
git commit -m "feat(ir): implement lattice table detector with coordinate snapping"
```

---

### Task 5: High-Fidelity PDF → PPTX Emitter

**Files:**
- Create: `crates/fileuc-convert/src/ir/pptx_emitter.rs`
- Modify: `crates/fileuc-convert/src/ir/mod.rs`
- Modify: `crates/fileuc-convert/src/pptx.rs`
- Test: `crates/fileuc-convert/tests/pptx_emitter_test.rs`

**Interfaces:**
- Consumes: `DocumentIR`
- Produces: `pub fn emit_pptx(doc: &DocumentIR, output: &Path) -> Result<(), ConvertError>`
  Generates OPC PPTX package with canvas coordinate transforms (`a:xfrm`), editable text boxes, native tables (`a:tbl`), shapes, and embedded images.

- [ ] **Step 1: Write the failing test**

```rust
// crates/fileuc-convert/tests/pptx_emitter_test.rs
use fileuc_convert::ir::pptx_emitter::emit_pptx;
use fileuc_convert::ir::types::*;

#[test]
fn test_emit_basic_pptx() {
    let mut doc = DocumentIR::new(Some("Test Presentation".to_string()));
    let mut page = PageIR::new(1, 960.0, 540.0, PageOrientation::Landscape);
    // Add sample paragraph and table
    doc.pages.push(page);
    let tmp = tempfile::NamedTempFile::new().unwrap();
    let res = emit_pptx(&doc, tmp.path());
    assert!(res.is_ok());
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --test pptx_emitter_test`
Expected: FAIL

- [ ] **Step 3: Implement PPTX emitter in `crates/fileuc-convert/src/ir/pptx_emitter.rs`**

Generate `[Content_Types].xml`, `_rels/.rels`, `ppt/presentation.xml`, slide layouts, slide relationships, and slide XML containing `p:sp` shapes with exact EMU coordinates ($1\text{ pt} = 12700\text{ EMUs}$), `a:tbl` tables, and `p:pic` blip fills.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --test pptx_emitter_test`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/fileuc-convert/src/ir/pptx_emitter.rs crates/fileuc-convert/tests/pptx_emitter_test.rs
git commit -m "feat(convert): implement high-fidelity PDF to PPTX emitter"
```

---

### Task 6: High-Fidelity PDF → DOCX Emitter

**Files:**
- Create: `crates/fileuc-convert/src/ir/docx_emitter.rs`
- Modify: `crates/fileuc-convert/src/ir/mod.rs`
- Modify: `crates/fileuc-convert/src/pdf.rs`
- Test: `crates/fileuc-convert/tests/docx_emitter_test.rs`

**Interfaces:**
- Consumes: `DocumentIR`
- Produces: `pub fn emit_docx(doc: &DocumentIR, output: &Path) -> Result<(), ConvertError>`
  Generates OPC DOCX package with flowing paragraphs, text formatting (size, bold, italic, color, spacing), native tables (`w:tbl` with `w:gridCol`, `w:tcPr`), and anchored drawings (`wp:anchor`).

- [ ] **Step 1: Write the failing test**

```rust
// crates/fileuc-convert/tests/docx_emitter_test.rs
use fileuc_convert::ir::docx_emitter::emit_docx;
use fileuc_convert::ir::types::*;

#[test]
fn test_emit_basic_docx() {
    let mut doc = DocumentIR::new(Some("Test Doc".to_string()));
    let page = PageIR::new(1, 595.0, 842.0, PageOrientation::Portrait);
    doc.pages.push(page);
    let tmp = tempfile::NamedTempFile::new().unwrap();
    let res = emit_docx(&doc, tmp.path());
    assert!(res.is_ok());
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --test docx_emitter_test`
Expected: FAIL

- [ ] **Step 3: Implement DOCX emitter in `crates/fileuc-convert/src/ir/docx_emitter.rs`**

Generate `word/document.xml`, `word/styles.xml`, and document relationships. Emit `w:p` with `w:rPr` (`w:rFonts`, `w:sz`, `w:b`, `w:i`, `w:color`), `w:tbl` with borders and shading, and `wp:anchor` for floating elements.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --test docx_emitter_test`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/fileuc-convert/src/ir/docx_emitter.rs crates/fileuc-convert/tests/docx_emitter_test.rs
git commit -m "feat(convert): implement high-fidelity PDF to DOCX emitter"
```

---

### Task 7: High-Fidelity PDF → XLSX Emitter

**Files:**
- Create: `crates/fileuc-convert/src/ir/xlsx_emitter.rs`
- Modify: `crates/fileuc-convert/src/ir/mod.rs`
- Create: `crates/fileuc-convert/src/excel.rs` (or extend existing)
- Test: `crates/fileuc-convert/tests/xlsx_emitter_test.rs`

**Interfaces:**
- Consumes: `DocumentIR`
- Produces: `pub fn emit_xlsx(doc: &DocumentIR, output: &Path) -> Result<(), ConvertError>`
  Generates OPC XLSX package (`xl/workbook.xml`, `xl/worksheets/sheet1.xml`, `xl/styles.xml`). Translates tables into structured worksheets, parses numeric/currency/date values, and formats cell widths.

- [ ] **Step 1: Write the failing test**

```rust
// crates/fileuc-convert/tests/xlsx_emitter_test.rs
use fileuc_convert::ir::xlsx_emitter::emit_xlsx;
use fileuc_convert::ir::types::*;

#[test]
fn test_emit_basic_xlsx() {
    let mut doc = DocumentIR::new(Some("Test Sheet".to_string()));
    let page = PageIR::new(1, 595.0, 842.0, PageOrientation::Portrait);
    doc.pages.push(page);
    let tmp = tempfile::NamedTempFile::new().unwrap();
    let res = emit_xlsx(&doc, tmp.path());
    assert!(res.is_ok());
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --test xlsx_emitter_test`
Expected: FAIL

- [ ] **Step 3: Implement XLSX emitter in `crates/fileuc-convert/src/ir/xlsx_emitter.rs`**

Generate `xl/workbook.xml`, `xl/styles.xml` (number formats, fonts, borders, fills), `xl/worksheets/sheet1.xml` with `<c r="A1" t="n"><v>123.45</v></c>` or `<c r="B1" t="inlineStr"><is><t>Hello</t></is></c>`, and set `<cols>` with custom widths based on cell text lengths.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --test xlsx_emitter_test`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/fileuc-convert/src/ir/xlsx_emitter.rs crates/fileuc-convert/tests/xlsx_emitter_test.rs
git commit -m "feat(convert): implement high-fidelity PDF to XLSX emitter"
```

---

### Task 8: Pipeline Wiring & FileUC Dispatcher Integration

**Files:**
- Modify: `crates/fileuc-types/src/lib.rs` (ensure `ConversionAction::PdfToXlsx` exists)
- Modify: `crates/fileuc-hud/src/smart_filter.rs` (add XLSX slice to PDF category)
- Modify: `crates/fileuc-daemon/src/dispatcher.rs` (route `PdfToDocx`, `PdfToPptx`, `PdfToXlsx` to new engine)
- Test: `crates/fileuc-daemon/tests/dispatcher_test.rs`

**Interfaces:**
- Connects user gestures and HUD slices directly to the new reconstruction engine.

- [ ] **Step 1: Write the failing test**

```rust
// Test in dispatcher_test verifying PdfToXlsx action is recognized and dispatches
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --test dispatcher_test`

- [ ] **Step 3: Wire new engine in `dispatcher.rs` and update `fileuc-types`**

Route `ConversionAction::PdfToDocx`, `PdfToPptx`, `PdfToXlsx` through `fileuc_convert::ir::reconstruct_and_emit`.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --test dispatcher_test`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/fileuc-types crates/fileuc-hud crates/fileuc-daemon
git commit -m "feat(pipeline): integrate reconstruction engine into daemon dispatcher and HUD"
```

---

### Task 9: Comprehensive Automated Validation Harness

**Files:**
- Create: `crates/fileuc-convert/tests/document_engine_validation_test.rs`

**Interfaces:**
- Verifies visual structure, text node editability, and XML compliance across generated DOCX, PPTX, and XLSX files.

- [ ] **Step 1: Write integration validation tests**
- [ ] **Step 2: Run `cargo test --workspace`**
- [ ] **Step 3: Verify all workspace tests pass 100%**
- [ ] **Step 4: Commit**

```bash
git add crates/fileuc-convert/tests/document_engine_validation_test.rs
git commit -m "test(validation): add automated document engine validation suite"
```
