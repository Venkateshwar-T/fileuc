use fileuc_types::ConvertError;
use image::{DynamicImage, RgbaImage};
use std::path::Path;

#[cfg(windows)]
use windows::{
    core::PCWSTR,
    Win32::{
        Foundation::GENERIC_READ,
        Graphics::Imaging::{
            CLSID_WICImagingFactory, IWICImagingFactory, GUID_WICPixelFormat32bppRGBA,
            WICBitmapDitherTypeNone, WICBitmapPaletteTypeCustom, WICDecodeMetadataCacheOnDemand,
        },
        System::Com::{CoCreateInstance, CoInitializeEx, CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED},
    },
};

pub fn decode_heic(src: &Path) -> Result<DynamicImage, ConvertError> {
    if !src.exists() {
        return Err(ConvertError::SourceNotFound(src.to_string_lossy().to_string()));
    }

    #[cfg(windows)]
    {
        decode_heic_wic(src)
    }

    #[cfg(not(windows))]
    {
        Err(ConvertError::UnsupportedFormat(
            "HEIC decoding via WIC is only available on Windows".to_string(),
        ))
    }
}

#[cfg(windows)]
fn decode_heic_wic(src: &Path) -> Result<DynamicImage, ConvertError> {
    use std::os::windows::ffi::OsStrExt;

    unsafe {
        let _ = CoInitializeEx(None, COINIT_MULTITHREADED);

        let factory: IWICImagingFactory = CoCreateInstance(&CLSID_WICImagingFactory, None, CLSCTX_INPROC_SERVER)
            .map_err(|e| ConvertError::EngineMissing(format!("WIC factory creation failed: {}", e)))?;

        let wide_path: Vec<u16> = src.as_os_str().encode_wide().chain(std::iter::once(0)).collect();

        let decoder = factory
            .CreateDecoderFromFilename(
                PCWSTR(wide_path.as_ptr()),
                None,
                GENERIC_READ,
                WICDecodeMetadataCacheOnDemand,
            )
            .map_err(|e| {
                ConvertError::ConverterError(format!(
                    "WIC failed to open HEIC file (ensure HEIF Image Extensions are installed): {}",
                    e
                ))
            })?;

        let frame = decoder
            .GetFrame(0)
            .map_err(|e| ConvertError::ConverterError(format!("Failed to get frame: {}", e)))?;

        let converter = factory
            .CreateFormatConverter()
            .map_err(|e| ConvertError::ConverterError(format!("Format converter creation failed: {}", e)))?;

        converter
            .Initialize(
                &frame,
                &GUID_WICPixelFormat32bppRGBA,
                WICBitmapDitherTypeNone,
                None,
                0.0,
                WICBitmapPaletteTypeCustom,
            )
            .map_err(|e| ConvertError::ConverterError(format!("Format conversion failed: {}", e)))?;

        let mut width: u32 = 0;
        let mut height: u32 = 0;
        converter
            .GetSize(&mut width, &mut height)
            .map_err(|e| ConvertError::ConverterError(format!("Failed to get image size: {}", e)))?;

        let stride = width.checked_mul(4)
            .ok_or_else(|| ConvertError::ConverterError("Image width causes stride overflow".into()))?;
        let buffer_size = (stride as usize)
            .checked_mul(height as usize)
            .ok_or_else(|| ConvertError::ConverterError("Image dimensions cause buffer overflow".into()))?;

        // Sanity limit: max 512MB for a single decompressed HEIC frame
        if buffer_size > 512 * 1024 * 1024 {
            return Err(ConvertError::ConverterError(format!("HEIC image exceeds maximum memory limit: {} bytes", buffer_size)));
        }

        let mut buffer = vec![0u8; buffer_size];

        converter
            .CopyPixels(std::ptr::null(), stride, &mut buffer)
            .map_err(|e| ConvertError::ConverterError(format!("Failed to copy pixels: {}", e)))?;

        let img_buf = RgbaImage::from_raw(width, height, buffer)
            .ok_or_else(|| ConvertError::ConverterError("Failed to build image buffer from WIC pixels".to_string()))?;

        Ok(DynamicImage::ImageRgba8(img_buf))
    }
}
