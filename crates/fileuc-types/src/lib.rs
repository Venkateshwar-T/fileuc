#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum ConversionAction {
    ToPng,
    ToJpg,
    ToJpeg,
    ToWebp,
    ToBmp,
    ToIco,
    ToTiff,
    ToPdf,
    ToSvg,
    ToGrayscale,
    Resize,
    SvgToPng,
    SvgToJpg,
    SvgToWebp,
    MergePdf,
    SplitPdf,
    CompressPdf,
    PdfToImages,
    PdfToDocx,
    PdfToDoc,
    PdfToPptx,
    PdfToXlsx,
    PptxToPdf,
    DocxToPptx,
    PptxToDocx,
    DocToPdf,
    CreateZip,
    Create7z,
    OneZip,
    ExtractArchive,
    VideoToGif,
    VideoToMp3,
    CompressVideo,
    Ocr,
    Share,
    LockPdf,
    UnlockPdf,
    ExcelToPdf,
    ToHtml,
}

impl ConversionAction {
    pub fn label(&self) -> &'static str {
        match self {
            ConversionAction::ToPng => "PNG",
            ConversionAction::ToJpg => "JPG",
            ConversionAction::ToJpeg => "JPEG",
            ConversionAction::ToWebp => "WebP",
            ConversionAction::ToBmp => "BMP",
            ConversionAction::ToIco => "ICO",
            ConversionAction::ToTiff => "TIFF",
            ConversionAction::ToPdf => "PDF",
            ConversionAction::ToSvg => "SVG",
            ConversionAction::ToGrayscale => "Grayscale",
            ConversionAction::Resize => "Resize",
            ConversionAction::SvgToPng => "PNG",
            ConversionAction::SvgToJpg => "JPG",
            ConversionAction::SvgToWebp => "WebP",
            ConversionAction::MergePdf => "Merge PDF",
            ConversionAction::SplitPdf => "Split PDF",
            ConversionAction::CompressPdf => "Compress",
            ConversionAction::PdfToImages => "Images",
            ConversionAction::PdfToDocx => "DOCX",
            ConversionAction::PdfToDoc => "DOC",
            ConversionAction::PdfToPptx => "PPTX",
            ConversionAction::PdfToXlsx => "Excel",
            ConversionAction::PptxToPdf => "PDF",
            ConversionAction::DocxToPptx => "PPTX",
            ConversionAction::PptxToDocx => "DOCX",
            ConversionAction::DocToPdf => "PDF",
            ConversionAction::CreateZip => "ZIP",
            ConversionAction::Create7z => "7Z",
            ConversionAction::OneZip => "One Zip",
            ConversionAction::ExtractArchive => "Extract",
            ConversionAction::VideoToGif => "GIF",
            ConversionAction::VideoToMp3 => "MP3",
            ConversionAction::CompressVideo => "Compress",
            ConversionAction::Ocr => "OCR",
            ConversionAction::Share => "Share",
            ConversionAction::LockPdf => "Lock",
            ConversionAction::UnlockPdf => "Unlock",
            ConversionAction::ExcelToPdf => "PDF",
            ConversionAction::ToHtml => "HTML",
        }
    }

    pub fn caption(&self) -> &'static str {
        match self {
            ConversionAction::Share => "Mobile",
            ConversionAction::CreateZip | ConversionAction::Create7z | ConversionAction::OneZip => "Archive",
            ConversionAction::ExtractArchive => "Unpack",
            ConversionAction::MergePdf => "Combine",
            ConversionAction::SplitPdf => "Separate",
            ConversionAction::CompressPdf | ConversionAction::CompressVideo => "Shrink",
            ConversionAction::Ocr => "Text",
            ConversionAction::PdfToPptx | ConversionAction::DocxToPptx => "Slides",
            ConversionAction::PptxToDocx => "Document",
            ConversionAction::LockPdf => "Protect",
            ConversionAction::UnlockPdf => "Decrypt",
            ConversionAction::ExcelToPdf | ConversionAction::PdfToXlsx => "Spreadsheet",
            ConversionAction::PdfToImages => "Extract",
            ConversionAction::ToHtml => "Markup",
            _ => "Convert",
        }
    }

    /// Action prompt displayed in the HUD center circle when targeting or hovering over this slice.
    pub fn center_prompt(&self) -> &'static str {
        match self {
            ConversionAction::Share => "Share to Mobile",
            ConversionAction::LockPdf => "Lock PDF",
            ConversionAction::UnlockPdf => "Unlock PDF",
            ConversionAction::SplitPdf => "Split PDF",
            ConversionAction::CompressPdf | ConversionAction::CompressVideo => "Compress this file",
            ConversionAction::MergePdf => "Merge PDF",
            ConversionAction::ExtractArchive => "Extract Archive",
            ConversionAction::Resize => "Resize Image",
            ConversionAction::ToPng | ConversionAction::SvgToPng => "Convert to PNG",
            ConversionAction::ToJpg | ConversionAction::ToJpeg | ConversionAction::SvgToJpg => "Convert to JPG",
            ConversionAction::ToWebp | ConversionAction::SvgToWebp => "Convert to WebP",
            ConversionAction::ToBmp => "Convert to BMP",
            ConversionAction::ToIco => "Convert to ICO",
            ConversionAction::ToTiff => "Convert to TIFF",
            ConversionAction::ToPdf
            | ConversionAction::DocToPdf
            | ConversionAction::PptxToPdf
            | ConversionAction::ExcelToPdf => "Convert to PDF",
            ConversionAction::ToSvg => "Convert to SVG",
            ConversionAction::PdfToDocx | ConversionAction::PptxToDocx => "Convert to DOCX",
            ConversionAction::PdfToDoc => "Convert to DOC",
            ConversionAction::PdfToPptx | ConversionAction::DocxToPptx => "Convert to PPTX",
            ConversionAction::PdfToXlsx => "Convert to Excel",
            ConversionAction::CreateZip | ConversionAction::OneZip => "Convert to ZIP",
            ConversionAction::Create7z => "Convert to 7Z",
            ConversionAction::VideoToGif => "Convert to GIF",
            ConversionAction::VideoToMp3 => "Convert to MP3",
            ConversionAction::PdfToImages => "Extract Images",
            ConversionAction::ToHtml => "Convert to HTML",
            ConversionAction::Ocr => "Convert to OCR",
            ConversionAction::ToGrayscale => "Convert to Grayscale",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum ConvertError {
    SourceNotFound(String),
    SourceLocked(String),
    UnsupportedFormat(String),
    EngineMissing(String),
    IoError(String),
    ConverterError(String),
}

impl std::fmt::Display for ConvertError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConvertError::SourceNotFound(p) => write!(f, "Source not found: {}", p),
            ConvertError::SourceLocked(p) => write!(f, "Source locked: {}", p),
            ConvertError::UnsupportedFormat(fmt) => write!(f, "Unsupported format: {}", fmt),
            ConvertError::EngineMissing(e) => write!(f, "Engine missing: {}", e),
            ConvertError::IoError(e) => write!(f, "I/O error: {}", e),
            ConvertError::ConverterError(e) => write!(f, "Converter error: {}", e),
        }
    }
}

impl std::error::Error for ConvertError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum ImageFormat {
    Png,
    Jpeg,
    WebP,
    Bmp,
    Ico,
    Tiff,
    Avif,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_center_prompts_match_specification() {
        assert_eq!(ConversionAction::Share.center_prompt(), "Share to Mobile");
        assert_eq!(ConversionAction::LockPdf.center_prompt(), "Lock PDF");
        assert_eq!(ConversionAction::UnlockPdf.center_prompt(), "Unlock PDF");
        assert_eq!(ConversionAction::SplitPdf.center_prompt(), "Split PDF");
        assert_eq!(ConversionAction::CompressPdf.center_prompt(), "Compress this file");
        assert_eq!(ConversionAction::CompressVideo.center_prompt(), "Compress this file");
        assert_eq!(ConversionAction::MergePdf.center_prompt(), "Merge PDF");
        assert_eq!(ConversionAction::ToPng.center_prompt(), "Convert to PNG");
        assert_eq!(ConversionAction::ToJpg.center_prompt(), "Convert to JPG");
        assert_eq!(ConversionAction::CreateZip.center_prompt(), "Convert to ZIP");
        assert_eq!(ConversionAction::Create7z.center_prompt(), "Convert to 7Z");
    }
}
