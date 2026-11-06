#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum ConversionAction {
    ToPng,
    ToJpg,
    ToWebp,
    ToBmp,
    ToIco,
    ToTiff,
    ToPdf,
    ToGrayscale,
    Resize,
    SvgToPng,
    SvgToJpg,
    MergePdf,
    SplitPdf,
    CompressPdf,
    PdfToImages,
    CreateZip,
    ExtractArchive,
    VideoToGif,
    VideoToMp3,
    CompressVideo,
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
