use fileuc_types::ConvertError;
use lopdf::{Dictionary, Document, Object};
use md5::{Digest, Md5};
use pdf_writer::{Content, Filter, Finish, Name, Pdf, Rect, Ref};
use std::path::{Path, PathBuf};

pub fn images_to_pdf(images: &[PathBuf], output: &Path) -> Result<(), ConvertError> {
    if images.is_empty() {
        return Err(ConvertError::ConverterError("No images provided".to_string()));
    }

    let mut pdf = Pdf::new();
    let mut page_refs = Vec::new();
    let mut next_ref = 1;

    let catalog_ref = Ref::new(next_ref);
    next_ref += 1;
    let page_tree_ref = Ref::new(next_ref);
    next_ref += 1;

    struct PageInfo {
        page_ref: Ref,
        image_ref: Ref,
        width: f32,
        height: f32,
        jpeg_data: Vec<u8>,
        pixel_w: u32,
        pixel_h: u32,
    }

    let mut page_infos = Vec::new();

    for img_path in images {
        if !img_path.exists() {
            return Err(ConvertError::SourceNotFound(img_path.to_string_lossy().to_string()));
        }

        let img = crate::image::open_image_with_orientation(img_path)?;
        let (pw, ph) = (img.width(), img.height());

        let mut jpeg_bytes = Vec::new();
        let rgb_img = img.to_rgb8();
        let mut enc = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut jpeg_bytes, 85);
        enc.encode(&rgb_img, pw, ph, image::ExtendedColorType::Rgb8)
            .map_err(|e| ConvertError::ConverterError(e.to_string()))?;

        let page_ref = Ref::new(next_ref);
        next_ref += 1;
        let image_ref = Ref::new(next_ref);
        next_ref += 1;

        page_refs.push(page_ref);
        page_infos.push(PageInfo {
            page_ref,
            image_ref,
            width: pw as f32,
            height: ph as f32,
            jpeg_data: jpeg_bytes,
            pixel_w: pw,
            pixel_h: ph,
        });
    }

    pdf.catalog(catalog_ref).pages(page_tree_ref);
    pdf.pages(page_tree_ref)
        .kids(page_refs.iter().copied())
        .count(page_refs.len() as i32);

    for info in page_infos {
        let content_ref = Ref::new(next_ref);
        next_ref += 1;
        let image_name = Name(b"Im0");

        let mut page = pdf.page(info.page_ref);
        page.media_box(Rect::new(0.0, 0.0, info.width, info.height));
        page.parent(page_tree_ref);
        page.contents(content_ref);
        page.resources().x_objects().pair(image_name, info.image_ref);
        page.finish();

        let mut content = Content::new();
        content.save_state();
        content.transform([info.width, 0.0, 0.0, info.height, 0.0, 0.0]);
        content.x_object(image_name);
        content.restore_state();
        pdf.stream(content_ref, &content.finish());

        let mut image_stream = pdf.image_xobject(info.image_ref, &info.jpeg_data);
        image_stream.width(info.pixel_w as i32);
        image_stream.height(info.pixel_h as i32);
        image_stream.color_space().device_rgb();
        image_stream.bits_per_component(8);
        image_stream.filter(Filter::DctDecode);
        image_stream.finish();
    }

    std::fs::write(output, pdf.finish()).map_err(|e| ConvertError::IoError(e.to_string()))?;
    Ok(())
}

/// Checks if a PDF is protected with an Open/User password that prevents reading without decryption.
/// Returns `false` for unencrypted PDFs and owner-locked PDFs with empty user passwords.
pub fn is_pdf_password_protected(path: &Path) -> bool {
    // 1. Try pure-Rust lopdf
    if let Ok(mut doc) = Document::load(path) {
        if doc.is_encrypted() {
            // If empty password fails to decrypt, the document requires an Open Password
            if doc.decrypt(b"").is_err() {
                return true;
            }
        }
        return false;
    }

    // 2. Fallback to Pdfium
    if let Ok(pdfium) = crate::pdfium::init_pdfium() {
        if let Err(e) = pdfium.load_pdf_from_file(path, None) {
            let err_str = e.to_string().to_lowercase();
            if err_str.contains("password") {
                return pdfium.load_pdf_from_file(path, Some("")).is_err();
            }
        }
    }

    false
}

pub fn merge_pdfs(pdfs: &[PathBuf], output: &Path) -> Result<(), ConvertError> {
    if pdfs.is_empty() {
        return Err(ConvertError::ConverterError("No PDFs to merge".to_string()));
    }

    for path in pdfs {
        if !path.exists() {
            return Err(ConvertError::SourceNotFound(path.to_string_lossy().to_string()));
        }
    }

    // Pre-flight validation: Ensure no PDF is locked with an Open Password.
    // Merging encrypted PDFs without decryption corrupts content streams and breaks the output.
    for path in pdfs {
        if is_pdf_password_protected(path) {
            let full_name = path.file_name().unwrap_or_default().to_string_lossy();
            let display_name = if full_name.chars().count() > 24 {
                let prefix: String = full_name.chars().take(21).collect();
                format!("{}...", prefix)
            } else {
                full_name.to_string()
            };
            return Err(ConvertError::ConverterError(format!(
                "Cannot merge: '{}' is password-protected.\nPlease unlock it before merging.",
                display_name
            )));
        }
    }

    // Try PDFium first (Google Chrome PDF engine: supports modern xref streams & PDF 1.0-2.0)
    if let Ok(pdfium) = crate::pdfium::init_pdfium() {
        if let Ok(mut merged_doc) = pdfium.create_new_pdf() {
            let mut all_success = true;
            for path in pdfs {
                let load_res = pdfium
                    .load_pdf_from_file(path, None)
                    .or_else(|_| pdfium.load_pdf_from_file(path, Some("")));
                if let Ok(doc) = load_res {
                    if merged_doc.pages_mut().append(&doc).is_err() {
                        all_success = false;
                        break;
                    }
                } else {
                    all_success = false;
                    break;
                }
            }
            if all_success && merged_doc.save_to_file(output).is_ok() {
                return Ok(());
            }
        }
    }

    // Fallback: pure-Rust lopdf implementation
    let mut max_id = 1;
    let mut target_doc = Document::with_version("1.5");
    let mut target_pages = Vec::new();

    for path in pdfs {
        let mut doc = Document::load(path)
            .map_err(|e| ConvertError::ConverterError(format!("Failed to load PDF: {}", e)))?;

        // If doc is encrypted (e.g. owner/permission locked), strip encryption so target_doc stays clean
        if doc.is_encrypted() {
            if doc.decrypt(b"").is_ok() {
                strip_encrypt_dict(&mut doc);
            } else {
                let full_name = path.file_name().unwrap_or_default().to_string_lossy();
                let display_name = if full_name.chars().count() > 24 {
                    let prefix: String = full_name.chars().take(21).collect();
                    format!("{}...", prefix)
                } else {
                    full_name.to_string()
                };
                return Err(ConvertError::ConverterError(format!(
                    "Cannot merge: '{}' is password-protected.\nPlease unlock it before merging.",
                    display_name
                )));
            }
        }

        doc.renumber_objects_with(max_id);
        max_id = doc.max_id + 1;

        let pages = doc.get_pages();
        for (_, &page_id) in pages.iter() {
            target_pages.push(page_id);
        }
        for (id, object) in doc.objects {
            target_doc.objects.insert(id, object);
        }
    }

    let pages_id = (max_id, 0);
    max_id += 1;
    let catalog_id = (max_id, 0);

    let mut pages_obj = Dictionary::new();
    pages_obj.set("Type", "Pages");
    pages_obj.set("Kids", Object::Array(target_pages.iter().map(|&id| Object::Reference(id)).collect()));
    pages_obj.set("Count", Object::Integer(target_pages.len() as i64));
    target_doc.objects.insert(pages_id, Object::Dictionary(pages_obj));

    for &page_id in &target_pages {
        if let Ok(page) = target_doc.get_object_mut(page_id) {
            if let Object::Dictionary(dict) = page {
                dict.set("Parent", Object::Reference(pages_id));
            }
        }
    }

    let mut catalog_obj = Dictionary::new();
    catalog_obj.set("Type", "Catalog");
    catalog_obj.set("Pages", Object::Reference(pages_id));
    target_doc.objects.insert(catalog_id, Object::Dictionary(catalog_obj));
    target_doc.trailer.set("Root", Object::Reference(catalog_id));

    target_doc
        .save(output)
        .map_err(|e| ConvertError::ConverterError(format!("Failed to save merged PDF: {}", e)))?;
    Ok(())
}

pub fn split_pdf(src: &Path, output_dir: &Path) -> Result<Vec<PathBuf>, ConvertError> {
    if !src.exists() {
        return Err(ConvertError::SourceNotFound(src.to_string_lossy().to_string()));
    }
    std::fs::create_dir_all(output_dir).map_err(|e| ConvertError::IoError(e.to_string()))?;

    let stem = src.file_stem().unwrap_or_default().to_string_lossy();

    // Try PDFium first
    if let Ok(pdfium) = crate::pdfium::init_pdfium() {
        if let Ok(doc) = pdfium.load_pdf_from_file(src, None) {
            let mut outputs = Vec::new();
            let total_pages = doc.pages().len();
            let mut all_ok = true;
            for page_idx in 0..total_pages {
                if let Ok(mut page_doc) = pdfium.create_new_pdf() {
                    let p_idx = page_idx as i32;
                    if page_doc
                        .pages_mut()
                        .copy_page_range_from_document(&doc, p_idx..=p_idx, 0)
                        .is_ok()
                    {
                        let out_path = output_dir.join(format!("{}_page_{}.pdf", stem, page_idx + 1));
                        if page_doc.save_to_file(&out_path).is_ok() {
                            outputs.push(out_path);
                            continue;
                        }
                    }
                }
                all_ok = false;
                break;
            }
            if all_ok && outputs.len() == total_pages as usize {
                return Ok(outputs);
            }
        }
    }

    // Fallback: pure-Rust lopdf implementation
    let mut outputs = Vec::new();
    let doc = Document::load(src)
        .map_err(|e| ConvertError::ConverterError(format!("Failed to load PDF: {}", e)))?;

    let pages = doc.get_pages();
    for (page_idx, (&page_num, _)) in pages.iter().enumerate() {
        let mut doc_copy = doc.clone();
        let pages_to_prune: Vec<u32> = pages.keys().copied().filter(|&p| p != page_num).collect();
        doc_copy.delete_pages(&pages_to_prune);
        doc_copy.prune_objects();

        let out_path = output_dir.join(format!("{}_page_{}.pdf", stem, page_idx + 1));
        doc_copy
            .save(&out_path)
            .map_err(|e| ConvertError::ConverterError(format!("Failed to save page PDF: {}", e)))?;
        outputs.push(out_path);
    }

    Ok(outputs)
}

pub fn compress_pdf(src: &Path, output: &Path) -> Result<(), ConvertError> {
    if !src.exists() {
        return Err(ConvertError::SourceNotFound(src.to_string_lossy().to_string()));
    }

    let mut doc = Document::load(src)
        .map_err(|e| ConvertError::ConverterError(format!("Failed to load PDF: {}", e)))?;

    // 1. Optimize embedded raster image streams (the primary cause of PDF bloat)
    optimize_pdf_images(&mut doc);

    // 2. Compress all text and structural streams using FlateDecode
    doc.compress();

    // 3. Remove unreferenced objects and orphaned metadata
    doc.prune_objects();

    doc.save(output)
        .map_err(|e| ConvertError::ConverterError(format!("Failed to compress PDF: {}", e)))?;
    Ok(())
}

fn optimize_pdf_images(doc: &mut Document) {
    for (_, obj) in doc.objects.iter_mut() {
        if let Object::Stream(ref mut stream) = obj {
            let is_image = stream.dict.get(b"Subtype")
                .and_then(|v| v.as_name())
                .map(|name| name == b"Image")
                .unwrap_or(false);

            if !is_image {
                continue;
            }

            let width = stream.dict.get(b"Width")
                .and_then(|v| v.as_i64())
                .unwrap_or(0) as u32;
            let height = stream.dict.get(b"Height")
                .and_then(|v| v.as_i64())
                .unwrap_or(0) as u32;

            if width < 50 || height < 50 {
                continue; // Skip small icons/stamps
            }

            let filter_name: &[u8] = stream.dict.get(b"Filter")
                .and_then(|v| v.as_name())
                .unwrap_or(b"".as_slice());

            let colorspace_name: &[u8] = stream.dict.get(b"ColorSpace")
                .and_then(|v| v.as_name())
                .unwrap_or(b"".as_slice());

            // Case A: FlateDecode (uncompressed RGB or Grayscale raster bitmap)
            if filter_name == b"FlateDecode" {
                if let Ok(raw_pixels) = stream.decompressed_content() {
                    let mut jpeg_bytes = Vec::new();
                    let success = if (colorspace_name == b"DeviceRGB" || colorspace_name.is_empty())
                        && raw_pixels.len() == (width * height * 3) as usize
                    {
                        let mut enc = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut jpeg_bytes, 80);
                        enc.encode(&raw_pixels, width, height, image::ExtendedColorType::Rgb8).is_ok()
                    } else if colorspace_name == b"DeviceGray" && raw_pixels.len() == (width * height) as usize {
                        let mut enc = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut jpeg_bytes, 80);
                        enc.encode(&raw_pixels, width, height, image::ExtendedColorType::L8).is_ok()
                    } else {
                        false
                    };

                    if success && !jpeg_bytes.is_empty() && jpeg_bytes.len() < stream.content.len() {
                        stream.set_content(jpeg_bytes);
                        stream.dict.set(b"Filter", Object::Name(b"DCTDecode".to_vec()));
                        stream.dict.remove(b"DecodeParms");
                        continue;
                    }
                }
            }

            // Case B: DCTDecode (existing JPEG that may be excessively high resolution / low compression)
            if filter_name == b"DCTDecode" && (width > 1600 || height > 1600) {
                if let Ok(img) = image::load_from_memory_with_format(&stream.content, image::ImageFormat::Jpeg) {
                    let target_w = width.min(1920);
                    let target_h = height.min(1920);
                    let resized = if width > 1920 || height > 1920 {
                        img.resize(target_w, target_h, image::imageops::FilterType::Lanczos3)
                    } else {
                        img
                    };

                    let mut new_jpeg = Vec::new();
                    let rgb = resized.to_rgb8();
                    let mut enc = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut new_jpeg, 78);
                    if enc.encode(&rgb, resized.width(), resized.height(), image::ExtendedColorType::Rgb8).is_ok() {
                        if !new_jpeg.is_empty() && new_jpeg.len() + 1024 < stream.content.len() {
                            stream.set_content(new_jpeg);
                            stream.dict.set(b"Width", Object::Integer(resized.width() as i64));
                            stream.dict.set(b"Height", Object::Integer(resized.height() as i64));
                        }
                    }
                }
            }
        }
    }
}

const PAD_BYTES: [u8; 32] = [
    0x28, 0xBF, 0x4E, 0x5E, 0x4E, 0x75, 0x8A, 0x41, 0x64, 0x00, 0x4E, 0x56, 0xFF, 0xFA, 0x01, 0x08,
    0x2E, 0x2E, 0x00, 0xB6, 0xD0, 0x68, 0x3E, 0x80, 0x2F, 0x0C, 0xA9, 0xFE, 0x64, 0x53, 0x69, 0x7A,
];

struct SimpleRc4 {
    s: [u8; 256],
    i: u8,
    j: u8,
}

impl SimpleRc4 {
    fn new(key: &[u8]) -> Self {
        let mut s = [0u8; 256];
        for i in 0..256 {
            s[i] = i as u8;
        }
        let mut j = 0u8;
        for i in 0..256 {
            j = j.wrapping_add(s[i]).wrapping_add(key[i % key.len()]);
            s.swap(i, j as usize);
        }
        Self { s, i: 0, j: 0 }
    }

    fn apply(&mut self, data: &mut [u8]) {
        for b in data.iter_mut() {
            self.i = self.i.wrapping_add(1);
            self.j = self.j.wrapping_add(self.s[self.i as usize]);
            self.s.swap(self.i as usize, self.j as usize);
            let t = self.s[self.i as usize].wrapping_add(self.s[self.j as usize]);
            *b ^= self.s[t as usize];
        }
    }
}

/// Locks a PDF with standard ISO 32000-1 128-bit encryption requiring a password to open.
pub fn lock_pdf(src: &Path, password: &str, output: &Path) -> Result<(), ConvertError> {
    if password.is_empty() {
        return Err(ConvertError::ConverterError("Password cannot be empty".to_string()));
    }
    lock_pdf_with_owner(src, password, password, output)
}

/// Locks a PDF with configurable user (open) and owner (permissions) passwords.
/// If `user_pass` is empty `""`, the PDF can be opened without a password prompt
/// but retains owner permission restrictions (standard owner/permission locked PDF).
pub fn lock_pdf_with_owner(
    src: &Path,
    user_pass: &str,
    owner_pass: &str,
    output: &Path,
) -> Result<(), ConvertError> {
    if !src.exists() {
        return Err(ConvertError::SourceNotFound(src.to_string_lossy().to_string()));
    }
    if user_pass.is_empty() && owner_pass.is_empty() {
        return Err(ConvertError::ConverterError("At least one password must be provided".to_string()));
    }

    let mut doc = Document::load(src)
        .map_err(|e| ConvertError::ConverterError(format!("Failed to load PDF: {}", e)))?;

    // 1. Prepare user & owner passwords padded to 32 bytes
    let mut u_pass_pad = [0u8; 32];
    let u_len = user_pass.as_bytes().len().min(32);
    u_pass_pad[..u_len].copy_from_slice(&user_pass.as_bytes()[..u_len]);
    u_pass_pad[u_len..].copy_from_slice(&PAD_BYTES[..32 - u_len]);

    let eff_owner = if owner_pass.is_empty() { user_pass } else { owner_pass };
    let mut o_pass_pad = [0u8; 32];
    let o_len = eff_owner.as_bytes().len().min(32);
    o_pass_pad[..o_len].copy_from_slice(&eff_owner.as_bytes()[..o_len]);
    o_pass_pad[o_len..].copy_from_slice(&PAD_BYTES[..32 - o_len]);

    // 2. Obtain or generate 16-byte document ID
    let file_id: Vec<u8> = if let Ok(id_arr) = doc.trailer.get(b"ID").and_then(Object::as_array) {
        if let Some(first_id) = id_arr.first().and_then(|o| o.as_str().ok()) {
            first_id.to_vec()
        } else {
            b"FILEUC_DOC_ID_16".to_vec()
        }
    } else {
        let id_val = b"FILEUC_DOC_ID_16".to_vec();
        doc.trailer.set(
            b"ID",
            Object::Array(vec![
                Object::String(id_val.clone(), lopdf::StringFormat::Literal),
                Object::String(id_val.clone(), lopdf::StringFormat::Literal),
            ]),
        );
        id_val
    };

    // 3. Compute Owner Hash (/O) - ISO 32000-1 Algorithm 3 (Rev 3)
    let mut hasher = Md5::new();
    hasher.update(&o_pass_pad);
    let mut o_key = hasher.finalize();
    for _ in 0..50 {
        let mut h = Md5::new();
        h.update(&o_key);
        o_key = h.finalize();
    }
    let mut o_val = u_pass_pad;
    SimpleRc4::new(&o_key).apply(&mut o_val);
    for i in 1..=19 {
        let cur_key: Vec<u8> = o_key.iter().map(|b| b ^ (i as u8)).collect();
        SimpleRc4::new(&cur_key).apply(&mut o_val);
    }

    // 4. Compute Document Encryption Key - ISO 32000-1 Algorithm 2 (Rev 3, 128-bit)
    let p_int: i32 = -3904; // Standard permissions
    let mut hasher = Md5::new();
    hasher.update(&u_pass_pad);
    hasher.update(&o_val);
    hasher.update(&p_int.to_le_bytes());
    hasher.update(&file_id);
    let mut doc_key = hasher.finalize();
    for _ in 0..50 {
        let mut h = Md5::new();
        h.update(&doc_key);
        doc_key = h.finalize();
    }
    let enc_key = doc_key;

    // 5. Compute User Hash (/U) - ISO 32000-1 Algorithm 3 (Rev 3)
    let mut hasher = Md5::new();
    hasher.update(&PAD_BYTES);
    hasher.update(&file_id);
    let u_hash = hasher.finalize();
    let mut u_val = [0u8; 32];
    u_val[..16].copy_from_slice(&u_hash);
    SimpleRc4::new(&enc_key).apply(&mut u_val[..16]);
    for i in 1..=19 {
        let cur_key: Vec<u8> = enc_key.iter().map(|b| b ^ (i as u8)).collect();
        SimpleRc4::new(&cur_key).apply(&mut u_val[..16]);
    }
    u_val[16..].fill(0);

    // 6. Encrypt all strings and streams in the document
    let obj_ids: Vec<lopdf::ObjectId> = doc.objects.keys().copied().collect();
    for id in obj_ids {
        let mut h = Md5::new();
        h.update(&enc_key);
        h.update(&[
            id.0 as u8,
            (id.0 >> 8) as u8,
            (id.0 >> 16) as u8,
            id.1 as u8,
            (id.1 >> 8) as u8,
        ]);
        let obj_key = &h.finalize()[..16];

        if let Some(obj) = doc.objects.get_mut(&id) {
            match obj {
                Object::String(ref mut s, _) => {
                    SimpleRc4::new(obj_key).apply(s);
                }
                Object::Stream(ref mut stream) => {
                    SimpleRc4::new(obj_key).apply(&mut stream.content);
                }
                _ => {}
            }
        }
    }

    // 7. Insert /Encrypt Dictionary into document trailer
    let mut enc_dict = Dictionary::new();
    enc_dict.set(b"Filter", Object::Name(b"Standard".to_vec()));
    enc_dict.set(b"V", Object::Integer(2));
    enc_dict.set(b"R", Object::Integer(3));
    enc_dict.set(b"Length", Object::Integer(128));
    enc_dict.set(b"P", Object::Integer(p_int as i64));
    enc_dict.set(b"O", Object::String(o_val.to_vec(), lopdf::StringFormat::Literal));
    enc_dict.set(b"U", Object::String(u_val.to_vec(), lopdf::StringFormat::Literal));
    let enc_id = doc.add_object(Object::Dictionary(enc_dict));
    doc.trailer.set(b"Encrypt", Object::Reference(enc_id));

    doc.save(output)
        .map_err(|e| ConvertError::IoError(format!("Failed to save locked PDF: {}", e)))?;
    Ok(())
}

/// Unlocks a PDF document.
///
/// Returns:
/// - `Ok(true)`: Successfully decrypted, /Encrypt stripped, and saved to `output`.
///   (Handles permission-locked / owner-locked PDFs instantly with empty password, matching iLovePDF).
/// - `Ok(false)`: Document is protected with an Open Password, and none was supplied.
/// - `Err(...)`: Invalid password, high-grade security that cannot be decrypted without key, or I/O error.
pub fn unlock_pdf(src: &Path, password: Option<&str>, output: &Path) -> Result<bool, ConvertError> {
    if !src.exists() {
        return Err(ConvertError::SourceNotFound(src.to_string_lossy().to_string()));
    }

    // 1. Pure-Rust lopdf: Fast, native, and directly removes /Encrypt
    // without enforcing Adobe DRM permission restrictions (FPDF_PERM_EXTRACT).
    if let Ok(mut doc) = Document::load(src) {
        if !doc.is_encrypted() {
            doc.save(output)
                .map_err(|e| ConvertError::IoError(e.to_string()))?;
            return Ok(true);
        }

        if let Some(pass) = password {
            if doc.decrypt(pass.as_bytes()).is_ok() {
                strip_encrypt_dict(&mut doc);
                if doc.save(output).is_ok() {
                    return Ok(true);
                }
            }
        } else {
            // No password supplied: try empty password b"" (handles permission/owner-locked PDFs instantly)
            if doc.decrypt(b"").is_ok() {
                strip_encrypt_dict(&mut doc);
                if doc.save(output).is_ok() {
                    return Ok(true);
                }
            }
        }
    }

    // 2. Secondary Engine: Chromium Pdfium (handles modern xrefs and AES-256 structures)
    if let Ok(pdfium) = crate::pdfium::init_pdfium() {
        let pdoc_res = match password {
            Some(pass) => pdfium.load_pdf_from_file(src, Some(pass)),
            None => pdfium
                .load_pdf_from_file(src, None)
                .or_else(|_| pdfium.load_pdf_from_file(src, Some(""))),
        };

        if let Ok(pdoc) = pdoc_res {
            if let Ok(mut clean_doc) = pdfium.create_new_pdf() {
                if clean_doc.pages_mut().append(&pdoc).is_ok() {
                    if clean_doc.save_to_file(output).is_ok() {
                        return Ok(true);
                    }
                }
            }
        }
    }

    // 3. Evaluation of result:
    if password.is_some() {
        // A password was provided, but could not unlock the document
        Err(ConvertError::ConverterError(
            "This document is highly secured and cannot be unlocked with the provided password.".to_string(),
        ))
    } else {
        // No password was provided, and empty password failed -> Open Password required
        Ok(false)
    }
}

fn strip_encrypt_dict(doc: &mut Document) {
    if let Ok(enc_ref) = doc.trailer.get(b"Encrypt").and_then(|o| o.as_reference()) {
        doc.objects.remove(&enc_ref);
    }
    doc.trailer.remove(b"Encrypt");
}

pub fn clean_pdf_text(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            // Standard whitespace
            '\t' | '\n' | '\r' => out.push(c),
            // Common PDF ligatures -> expand to plain readable English
            '\u{FB00}' => out.push_str("ff"),
            '\u{FB01}' => out.push_str("fi"),
            '\u{FB02}' => out.push_str("fl"),
            '\u{FB03}' => out.push_str("ffi"),
            '\u{FB04}' => out.push_str("ffl"),
            '\u{FB05}' | '\u{FB06}' => out.push_str("st"),
            // Smart quotes, dashes, bullets
            '\u{2018}' | '\u{2019}' | '\u{201A}' | '\u{201B}' => out.push('\''),
            '\u{201C}' | '\u{201D}' | '\u{201E}' => out.push('"'),
            '\u{2013}' | '\u{2014}' | '\u{2015}' => out.push('-'),
            '\u{2022}' | '\u{2023}' | '\u{25AA}' | '\u{25CF}' => out.push('*'),
            '\u{00A0}' => out.push(' '), // Non-breaking space
            '\u{200B}'..='\u{200D}' | '\u{FEFF}' => {} // Strip zero-width spaces/BOM
            // Filter out non-printable ASCII control characters (0x00..0x1F, 0x7F..0x9F)
            c if (c as u32) < 0x20 || ((c as u32) >= 0x7F && (c as u32) <= 0x9F) => {}
            // Filter out Unicode Private Use Area (0xE000..=0xF8FF) which are unmapped PDF CID glyphs
            '\u{E000}'..='\u{F8FF}' => {}
            // Filter out Unicode replacement / special chars
            '\u{FFF0}'..='\u{FFFF}' => {}
            // Normal printable characters
            c => out.push(c),
        }
    }
    out
}

pub fn extract_pdf_paragraphs(src: &Path) -> Result<Vec<String>, ConvertError> {
    if !src.exists() {
        return Err(ConvertError::SourceNotFound(src.to_string_lossy().to_string()));
    }

    // Try Google Chromium PDFium engine first for 100% accurate font, ligature, and layout extraction
    if let Ok(paras) = crate::pdfium::extract_text(src) {
        if !paras.is_empty() {
            let processed: Vec<String> = paras
                .into_iter()
                .map(|p| {
                    if crate::indic::is_mojibake_or_legacy_font(&p, None) {
                        crate::indic::convert_legacy_text(&p, None)
                    } else {
                        clean_pdf_text(&p)
                    }
                })
                .filter(|s| !s.trim().is_empty())
                .collect();
            if !processed.is_empty() {
                return Ok(processed);
            }
        }
    }

    // Pure Rust lopdf fallback
    let doc = Document::load(src)
        .map_err(|e| ConvertError::ConverterError(format!("Failed to load PDF: {}", e)))?;
    let pages = doc.get_pages();
    let mut paragraphs = Vec::new();

    for (&page_num, _) in pages.iter() {
        if let Ok(text) = doc.extract_text(&[page_num]) {
            let converted = if crate::indic::is_mojibake_or_legacy_font(&text, None) {
                crate::indic::convert_legacy_text(&text, None)
            } else {
                clean_pdf_text(&text)
            };
            for line in converted.lines() {
                let trimmed = line.trim();
                if !trimmed.is_empty() {
                    paragraphs.push(trimmed.to_string());
                }
            }
        }
    }

    if paragraphs.is_empty() {
        paragraphs.push(format!(
            "Document: {}",
            src.file_name().unwrap_or_default().to_string_lossy()
        ));
    }

    Ok(paragraphs)
}
