#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl Rect {
    pub fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self { x, y, width, height }
    }

    pub fn right(&self) -> f32 {
        self.x + self.width
    }

    pub fn bottom(&self) -> f32 {
        self.y + self.height
    }

    pub fn contains_point(&self, px: f32, py: f32) -> bool {
        px >= self.x && px <= self.right() && py >= self.y && py <= self.bottom()
    }

    pub fn intersects(&self, other: &Rect) -> bool {
        self.x < other.right()
            && self.right() > other.x
            && self.y < other.bottom()
            && self.bottom() > other.y
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PageOrientation {
    Portrait,
    Landscape,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextAlignment {
    Left,
    Center,
    Right,
    Justified,
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
pub struct TextLineIR {
    pub bounds: Rect,
    pub baseline_y: f32,
    pub runs: Vec<TextRunIR>,
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
pub enum BlockIR {
    Paragraph(ParagraphIR),
}

#[derive(Debug, Clone, Default)]
pub struct BorderLine {
    pub width_pt: f32,
    pub color_rgb: [u8; 3],
}

#[derive(Debug, Clone, Default)]
pub struct CellBorders {
    pub top: Option<BorderLine>,
    pub right: Option<BorderLine>,
    pub bottom: Option<BorderLine>,
    pub left: Option<BorderLine>,
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

#[derive(Debug, Clone)]
pub struct TableIR {
    pub bounds: Rect,
    pub column_widths_pt: Vec<f32>,
    pub cells: Vec<TableCellIR>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageFormat {
    Png,
    Jpeg,
}

#[derive(Debug, Clone)]
pub struct ImageIR {
    pub bounds: Rect,
    pub data: Vec<u8>,
    pub format: ImageFormat,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShapeType {
    Rectangle,
    Line,
}

#[derive(Debug, Clone)]
pub struct ShapeIR {
    pub bounds: Rect,
    pub shape_type: ShapeType,
    pub stroke_color: Option<[u8; 3]>,
    pub stroke_width_pt: f32,
    pub fill_color: Option<[u8; 3]>,
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

impl PageIR {
    pub fn new(page_number: usize, width_pt: f32, height_pt: f32, orientation: PageOrientation) -> Self {
        Self {
            page_number,
            width_pt,
            height_pt,
            orientation,
            blocks: Vec::new(),
            tables: Vec::new(),
            images: Vec::new(),
            shapes: Vec::new(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct DocumentIR {
    pub title: Option<String>,
    pub pages: Vec<PageIR>,
}

impl DocumentIR {
    pub fn new(title: Option<String>) -> Self {
        Self {
            title,
            pages: Vec::new(),
        }
    }
}
