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
    PptxToPdf,
    DocToPdf,
    PdfToDocx,
    CreateZip,
    Create7z,
    OneZip,
    ExtractArchive,
    VideoToGif,
    VideoToMp3,
    CompressVideo,
    Share,
    LockPdf,
    UnlockPdf,
    ToHtml,
    MdToPdf,
    TxtToPdf,
    HtmlToPdf,
    ExcelToPdf,
    PdfToExcel,
    PdfOcr,
    XlsxToCsv,
    XlsxToJson,
    CsvToXlsx,
    CsvToJson,
    JsonToXlsx,
    JsonToCsv,
    MdToTxt,
    TxtToMd,
    StripMetadata,
    ReduceSize,
    BulkRename,
    OfficeGroup,
    IconGroup,
    CreateIconPack,
    ToMp3,
    ToWav,
    ToOgg,
    ToAac,
    ToFlac,
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
            ConversionAction::PptxToPdf => "PDF",
            ConversionAction::DocToPdf => "PDF",
            ConversionAction::PdfToDocx => "DOCX",
            ConversionAction::CreateZip => "ZIP",
            ConversionAction::Create7z => "7Z",
            ConversionAction::OneZip => "One Zip",
            ConversionAction::ExtractArchive => "Extract",
            ConversionAction::VideoToGif => "GIF",
            ConversionAction::VideoToMp3 => "MP3",
            ConversionAction::CompressVideo => "Compress",
            ConversionAction::Share => "Share",
            ConversionAction::LockPdf => "Lock",
            ConversionAction::UnlockPdf => "Unlock",
            ConversionAction::ToHtml => "HTML",
            ConversionAction::MdToPdf => "PDF",
            ConversionAction::TxtToPdf => "PDF",
            ConversionAction::HtmlToPdf => "PDF",
            ConversionAction::ExcelToPdf => "PDF",
            ConversionAction::PdfToExcel => "EXCEL",
            ConversionAction::PdfOcr => "OCR",
            ConversionAction::XlsxToCsv => "CSV",
            ConversionAction::XlsxToJson => "JSON",
            ConversionAction::CsvToXlsx => "EXCEL",
            ConversionAction::CsvToJson => "JSON",
            ConversionAction::JsonToXlsx => "EXCEL",
            ConversionAction::JsonToCsv => "CSV",
            ConversionAction::MdToTxt => "TXT",
            ConversionAction::TxtToMd => "MD",
            ConversionAction::StripMetadata => "Clean",
            ConversionAction::ReduceSize => "Reduce",
            ConversionAction::BulkRename => "Rename",
            ConversionAction::OfficeGroup => "Office",
            ConversionAction::IconGroup => "Icon",
            ConversionAction::CreateIconPack => "Icon Pack",
            ConversionAction::ToMp3 => "MP3",
            ConversionAction::ToWav => "WAV",
            ConversionAction::ToOgg => "OGG",
            ConversionAction::ToAac => "AAC",
            ConversionAction::ToFlac => "FLAC",
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
            ConversionAction::LockPdf => "Protect",
            ConversionAction::UnlockPdf => "Decrypt",
            ConversionAction::PdfToDocx => "Word",
            ConversionAction::PdfToImages => "Extract",
            ConversionAction::ToHtml => "Markup",
            ConversionAction::PdfToExcel => "Spreadsheet",
            ConversionAction::PdfOcr => "OCR",
            ConversionAction::XlsxToCsv | ConversionAction::JsonToCsv => "Data",
            ConversionAction::XlsxToJson | ConversionAction::CsvToJson => "Data",
            ConversionAction::CsvToXlsx | ConversionAction::JsonToXlsx => "Spreadsheet",
            ConversionAction::MdToTxt => "Plain Text",
            ConversionAction::TxtToMd => "Markdown",
            ConversionAction::StripMetadata => "No Meta",
            ConversionAction::ReduceSize => "Size Limit",
            ConversionAction::BulkRename => "Bulk",
            ConversionAction::OfficeGroup => "Documents",
            ConversionAction::IconGroup => "Icons",
            ConversionAction::CreateIconPack => "Assets",
            ConversionAction::ToMp3
            | ConversionAction::ToWav
            | ConversionAction::ToOgg
            | ConversionAction::ToAac
            | ConversionAction::ToFlac => "Audio",
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
            | ConversionAction::MdToPdf
            | ConversionAction::TxtToPdf
            | ConversionAction::HtmlToPdf
            | ConversionAction::ExcelToPdf => "Convert to PDF",
            ConversionAction::PdfToDocx => "Convert to DOCX",
            ConversionAction::PdfToExcel => "Extract to Excel",
            ConversionAction::PdfOcr => "Make PDF Searchable",
            ConversionAction::ToSvg => "Convert to SVG",
            ConversionAction::CreateZip | ConversionAction::OneZip => "Convert to ZIP",
            ConversionAction::Create7z => "Convert to 7Z",
            ConversionAction::VideoToGif => "Convert to GIF",
            ConversionAction::VideoToMp3 => "Convert to MP3",
            ConversionAction::PdfToImages => "Extract Images",
            ConversionAction::ToHtml => "Convert to HTML",
            ConversionAction::XlsxToCsv | ConversionAction::JsonToCsv => "Convert to CSV",
            ConversionAction::XlsxToJson | ConversionAction::CsvToJson => "Convert to JSON",
            ConversionAction::CsvToXlsx | ConversionAction::JsonToXlsx => "Convert to Excel",
            ConversionAction::MdToTxt => "Convert to Text",
            ConversionAction::TxtToMd => "Convert to Markdown",
            ConversionAction::ToGrayscale => "Convert to Grayscale",
            ConversionAction::StripMetadata => "Strip Metadata",
            ConversionAction::ReduceSize => "Reduce File Size",
            ConversionAction::BulkRename => "Bulk Rename Files",
            ConversionAction::OfficeGroup => "Office Formats",
            ConversionAction::IconGroup => "Icon Formats",
            ConversionAction::CreateIconPack => "Generate App/Web Icon Pack",
            ConversionAction::ToMp3 => "Convert to MP3",
            ConversionAction::ToWav => "Convert to WAV",
            ConversionAction::ToOgg => "Convert to OGG",
            ConversionAction::ToAac => "Convert to AAC",
            ConversionAction::ToFlac => "Convert to FLAC",
        }
    }

    /// Returns true if this action is an expandable category group that opens sub-slices on hover.
    pub fn is_group(&self) -> bool {
        matches!(self, ConversionAction::OfficeGroup | ConversionAction::IconGroup)
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

impl ImageFormat {
    pub fn extension(&self) -> &'static str {
        match self {
            ImageFormat::Png => "png",
            ImageFormat::Jpeg => "jpg",
            ImageFormat::WebP => "webp",
            ImageFormat::Bmp => "bmp",
            ImageFormat::Ico => "ico",
            ImageFormat::Tiff => "tiff",
            ImageFormat::Avif => "avif",
        }
    }

    pub fn mime_type(&self) -> &'static str {
        match self {
            ImageFormat::Png => "image/png",
            ImageFormat::Jpeg => "image/jpeg",
            ImageFormat::WebP => "image/webp",
            ImageFormat::Bmp => "image/bmp",
            ImageFormat::Ico => "image/x-icon",
            ImageFormat::Tiff => "image/tiff",
            ImageFormat::Avif => "image/avif",
        }
    }

    pub fn from_extension(ext: &str) -> Option<Self> {
        match ext.trim_start_matches('.').to_ascii_lowercase().as_str() {
            "png" => Some(ImageFormat::Png),
            "jpg" | "jpeg" => Some(ImageFormat::Jpeg),
            "webp" => Some(ImageFormat::WebP),
            "bmp" => Some(ImageFormat::Bmp),
            "ico" => Some(ImageFormat::Ico),
            "tiff" | "tif" => Some(ImageFormat::Tiff),
            "avif" => Some(ImageFormat::Avif),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum IpcMessage {
    OpenSettings,
    OpenSettingsTab(i32),
    ReloadSettings,
    ToggleService(bool),
    CancelActiveConversion,
    Quit,
}

#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct ActiveConversionInfo {
    pub is_active: bool,
    pub file_summary: String,
    pub action_display: String,
    pub progress: f32,
    pub eta_text: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_center_prompts_match_specification() {
        assert_eq!(ConversionAction::Share.center_prompt(), "Share to Mobile");
        assert_eq!(ConversionAction::PdfToDocx.center_prompt(), "Convert to DOCX");
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
