use fileuc_types::ConvertError;
use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};
use zip::write::SimpleFileOptions;
use zip::{ZipArchive, ZipWriter};

pub fn create_zip(sources: &[PathBuf], output: &Path) -> Result<(), ConvertError> {
    if sources.is_empty() {
        return Err(ConvertError::ConverterError("No sources provided to zip".to_string()));
    }

    let file = File::create(output).map_err(|e| ConvertError::IoError(e.to_string()))?;
    let mut zip = ZipWriter::new(file);
    let options = SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);

    for src in sources {
        if !src.exists() {
            return Err(ConvertError::SourceNotFound(src.to_string_lossy().to_string()));
        }

        if src.is_file() {
            let file_name = src
                .file_name()
                .ok_or_else(|| ConvertError::ConverterError("Invalid file name".to_string()))?
                .to_string_lossy();

            zip.start_file(file_name, options)
                .map_err(|e| ConvertError::ConverterError(e.to_string()))?;

            let mut f = File::open(src).map_err(|e| ConvertError::IoError(e.to_string()))?;
            std::io::copy(&mut f, &mut zip).map_err(|e| ConvertError::IoError(e.to_string()))?;
        } else if src.is_dir() {
            add_dir_to_zip(&mut zip, src, src, options)?;
        }
    }

    zip.finish().map_err(|e| ConvertError::ConverterError(e.to_string()))?;
    Ok(())
}

fn add_dir_to_zip<W: Write + std::io::Seek>(
    zip: &mut ZipWriter<W>,
    base_dir: &Path,
    current_dir: &Path,
    options: SimpleFileOptions,
) -> Result<(), ConvertError> {
    let entries = std::fs::read_dir(current_dir).map_err(|e| ConvertError::IoError(e.to_string()))?;

    for entry in entries.flatten() {
        let path = entry.path();
        let relative = path
            .strip_prefix(base_dir)
            .unwrap_or(&path)
            .to_string_lossy()
            .replace('\\', "/");

        if path.is_file() {
            zip.start_file(relative, options)
                .map_err(|e| ConvertError::ConverterError(e.to_string()))?;
            let mut f = File::open(&path).map_err(|e| ConvertError::IoError(e.to_string()))?;
            std::io::copy(&mut f, zip).map_err(|e| ConvertError::IoError(e.to_string()))?;
        } else if path.is_dir() {
            zip.add_directory(format!("{}/", relative), options)
                .map_err(|e| ConvertError::ConverterError(e.to_string()))?;
            add_dir_to_zip(zip, base_dir, &path, options)?;
        }
    }

    Ok(())
}

pub fn extract_archive(src: &Path, output_dir: &Path) -> Result<(), ConvertError> {
    if !src.exists() {
        return Err(ConvertError::SourceNotFound(src.to_string_lossy().to_string()));
    }

    std::fs::create_dir_all(output_dir).map_err(|e| ConvertError::IoError(e.to_string()))?;

    let file_name = src.file_name().unwrap_or_default().to_string_lossy().to_ascii_lowercase();

    if file_name.ends_with(".tar.gz") || file_name.ends_with(".tgz") {
        let tar_gz = File::open(src).map_err(|e| ConvertError::IoError(e.to_string()))?;
        let tar = flate2::read::GzDecoder::new(tar_gz);
        let mut archive = tar::Archive::new(tar);
        archive
            .unpack(output_dir)
            .map_err(|e| ConvertError::ConverterError(format!("Failed to unpack tar.gz: {}", e)))?;
        return Ok(());
    }

    if file_name.ends_with(".tar") {
        let tar_file = File::open(src).map_err(|e| ConvertError::IoError(e.to_string()))?;
        let mut archive = tar::Archive::new(tar_file);
        archive
            .unpack(output_dir)
            .map_err(|e| ConvertError::ConverterError(format!("Failed to unpack tar: {}", e)))?;
        return Ok(());
    }

    // Default to ZIP
    let file = File::open(src).map_err(|e| ConvertError::IoError(e.to_string()))?;
    let mut archive = ZipArchive::new(file)
        .map_err(|e| ConvertError::ConverterError(format!("Failed to open zip archive: {}", e)))?;

    archive
        .extract(output_dir)
        .map_err(|e| ConvertError::ConverterError(format!("Failed to extract zip archive: {}", e)))?;

    Ok(())
}
