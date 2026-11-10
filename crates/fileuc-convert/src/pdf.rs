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

        let img = image::open(img_path)
            .map_err(|e| ConvertError::ConverterError(format!("Failed to open image: {}", e)))?;
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

pub fn merge_pdfs(pdfs: &[PathBuf], output: &Path) -> Result<(), ConvertError> {
    if pdfs.is_empty() {
        return Err(ConvertError::ConverterError("No PDFs to merge".to_string()));
    }

    for path in pdfs {
        if !path.exists() {
            return Err(ConvertError::SourceNotFound(path.to_string_lossy().to_string()));
        }
    }

    // Try PDFium first (Google Chrome PDF engine: supports modern xref streams & PDF 1.0-2.0)
    if let Ok(pdfium) = crate::pdfium::init_pdfium() {
        if let Ok(mut merged_doc) = pdfium.create_new_pdf() {
            let mut all_success = true;
            for path in pdfs {
                if let Ok(doc) = pdfium.load_pdf_from_file(path, None) {
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
    if !src.exists() {
        return Err(ConvertError::SourceNotFound(src.to_string_lossy().to_string()));
    }
    if password.is_empty() {
        return Err(ConvertError::ConverterError("Password cannot be empty".to_string()));
    }

    let mut doc = Document::load(src)
        .map_err(|e| ConvertError::ConverterError(format!("Failed to load PDF: {}", e)))?;

    // 1. Prepare user password padded to 32 bytes
    let mut u_pass_pad = [0u8; 32];
    let u_len = password.as_bytes().len().min(32);
    u_pass_pad[..u_len].copy_from_slice(&password.as_bytes()[..u_len]);
    u_pass_pad[u_len..].copy_from_slice(&PAD_BYTES[..32 - u_len]);

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
    hasher.update(&u_pass_pad);
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
/// - `Ok(true)`: Successfully decrypted and saved to `output`.
///   (Handles permission-locked / owner-locked PDFs instantly with empty password, matching iLovePDF).
/// - `Ok(false)`: Document requires a user password, none was supplied.
/// - `Err(...)`: Wrong password supplied or I/O error.
pub fn unlock_pdf(src: &Path, password: Option<&str>, output: &Path) -> Result<bool, ConvertError> {
    if !src.exists() {
        return Err(ConvertError::SourceNotFound(src.to_string_lossy().to_string()));
    }

    // 1. Try PDFium first (Chromium PDF engine: handles modern xref streams, AES-256, and all PDF versions)
    if let Ok(pdfium) = crate::pdfium::init_pdfium() {
        // Case A: User provided a password explicitly
        if let Some(pass) = password {
            match pdfium.load_pdf_from_file(src, Some(pass)) {
                Ok(doc) => {
                    let mut clean_doc = pdfium
                        .create_new_pdf()
                        .map_err(|e| ConvertError::ConverterError(format!("Failed to create clean PDF: {}", e)))?;
                    clean_doc
                        .pages_mut()
                        .append(&doc)
                        .map_err(|e| ConvertError::ConverterError(format!("Failed to transfer pages: {}", e)))?;
                    clean_doc
                        .save_to_file(output)
                        .map_err(|e| ConvertError::IoError(format!("Failed to save unlocked PDF: {}", e)))?;
                    return Ok(true);
                }
                Err(_) => {
                    return Err(ConvertError::ConverterError("Incorrect password for locked PDF".to_string()));
                }
            }
        }

        // Case B: No password provided yet
        // First check if the PDF can be loaded directly without any password (unencrypted or permission-locked)
        if let Ok(doc) = pdfium.load_pdf_from_file(src, None) {
            let mut clean_doc = pdfium
                .create_new_pdf()
                .map_err(|e| ConvertError::ConverterError(format!("Failed to create clean PDF: {}", e)))?;
            clean_doc
                .pages_mut()
                .append(&doc)
                .map_err(|e| ConvertError::ConverterError(format!("Failed to transfer pages: {}", e)))?;
            clean_doc
                .save_to_file(output)
                .map_err(|e| ConvertError::IoError(format!("Failed to save unlocked PDF: {}", e)))?;
            return Ok(true);
        }

        // Second, try quick common passwords (empty string, default tokens)
        let quick_passwords: [&str; 6] = ["", "fileuc", "password", "123456", "1234", "admin"];
        for candidate in quick_passwords {
            if let Ok(doc) = pdfium.load_pdf_from_file(src, Some(candidate)) {
                let mut clean_doc = pdfium
                    .create_new_pdf()
                    .map_err(|e| ConvertError::ConverterError(format!("Failed to create clean PDF: {}", e)))?;
                clean_doc
                    .pages_mut()
                    .append(&doc)
                    .map_err(|e| ConvertError::ConverterError(format!("Failed to transfer pages: {}", e)))?;
                clean_doc
                    .save_to_file(output)
                    .map_err(|e| ConvertError::IoError(format!("Failed to save unlocked PDF: {}", e)))?;
                return Ok(true);
            }
        }

        // If none matched, document requires user password input
        return Ok(false);
    }

    // 2. Fallback: pure-Rust lopdf implementation (for environments without pdfium.dll)
    let mut doc = Document::load(src)
        .map_err(|e| ConvertError::ConverterError(format!("Failed to load PDF: {}", e)))?;

    if !doc.is_encrypted() {
        doc.save(output)
            .map_err(|e| ConvertError::IoError(e.to_string()))?;
        return Ok(true);
    }

    // Try empty password first (handles owner/permission locks instantly without prompting)
    if doc.decrypt(b"").is_ok() {
        doc.save(output)
            .map_err(|e| ConvertError::IoError(e.to_string()))?;
        return Ok(true);
    }

    // Try common dictionary passwords
    let quick_passwords: [&[u8]; 5] = [b"fileuc", b"password", b"123456", b"1234", b"admin"];
    for &candidate in &quick_passwords {
        let mut retry_doc = Document::load(src)
            .map_err(|e| ConvertError::ConverterError(format!("Failed to reload PDF: {}", e)))?;
        if retry_doc.decrypt(candidate).is_ok() {
            retry_doc.save(output)
                .map_err(|e| ConvertError::IoError(e.to_string()))?;
            return Ok(true);
        }
    }

    // If password was provided, try it
    if let Some(pass) = password {
        let mut retry_doc = Document::load(src)
            .map_err(|e| ConvertError::ConverterError(format!("Failed to reload PDF: {}", e)))?;
        if retry_doc.decrypt(pass.as_bytes()).is_ok() {
            retry_doc.save(output)
                .map_err(|e| ConvertError::IoError(e.to_string()))?;
            return Ok(true);
        } else {
            return Err(ConvertError::ConverterError("Incorrect password for locked PDF".to_string()));
        }
    }

    // User password required
    Ok(false)
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

fn xml_escape_text(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            c if (c as u32) >= 0x20 || c == '\t' || c == '\n' || c == '\r' => out.push(c),
            _ => {}
        }
    }
    out
}

pub fn pdf_to_docx(src: &Path, output: &Path) -> Result<(), ConvertError> {
    if !src.exists() {
        return Err(ConvertError::SourceNotFound(src.to_string_lossy().to_string()));
    }

    // 1. Try structured layout analysis via PDFium first (load document ONCE)
    if let Ok(pdfium) = crate::pdfium::init_pdfium() {
        if let Ok(document) = pdfium.load_pdf_from_file(src, None) {
            if let Ok(layouts) = crate::pdfium::extract_structured_layout_from_doc(&document) {
                if !layouts.is_empty() {
                    let has_text = layouts.iter().any(|l| !l.items.is_empty());
                    if has_text {
                        return render_structured_layout_to_docx(src, &layouts, output);
                    } else {
                        return render_image_pdf_doc_to_docx(&document, &layouts, output);
                    }
                }
            }
        }
    }

    // 2. High-fidelity Document IR reconstruction engine
    if let Ok(()) = crate::ir::reconstruct_pdf_to_docx(src, output) {
        if output.exists() {
            if let Ok(meta) = std::fs::metadata(output) {
                if meta.len() > 0 {
                    return Ok(());
                }
            }
        }
    }

    // 3. Genuine last-resort fallback: Pure Rust lopdf text extraction if PDFium is unavailable
    let paragraphs = extract_pdf_paragraphs(src)?;
    render_paragraphs_to_docx(&paragraphs, output)
}

pub fn pdf_to_xlsx(src: &Path, output: &Path) -> Result<(), ConvertError> {
    if !src.exists() {
        return Err(ConvertError::SourceNotFound(src.to_string_lossy().to_string()));
    }
    crate::ir::reconstruct_pdf_to_xlsx(src, output)
}

fn render_paragraphs_to_docx(paragraphs: &[String], output: &Path) -> Result<(), ConvertError> {
    use std::io::Write;

    let file = std::fs::File::create(output).map_err(|e| ConvertError::IoError(e.to_string()))?;
    let mut zip = zip::ZipWriter::new(file);
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);

    // [Content_Types].xml
    zip.start_file("[Content_Types].xml", options)
        .map_err(|e| ConvertError::IoError(e.to_string()))?;
    zip.write_all(br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
  <Default Extension="xml" ContentType="application/xml"/>
  <Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/>
  <Override PartName="/word/styles.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.styles+xml"/>
</Types>"#)
        .map_err(|e| ConvertError::IoError(e.to_string()))?;

    // _rels/.rels
    zip.start_file("_rels/.rels", options)
        .map_err(|e| ConvertError::IoError(e.to_string()))?;
    zip.write_all(br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/>
</Relationships>"#)
        .map_err(|e| ConvertError::IoError(e.to_string()))?;

    // word/_rels/document.xml.rels
    zip.start_file("word/_rels/document.xml.rels", options)
        .map_err(|e| ConvertError::IoError(e.to_string()))?;
    zip.write_all(br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles" Target="styles.xml"/>
</Relationships>"#)
        .map_err(|e| ConvertError::IoError(e.to_string()))?;

    // word/styles.xml
    zip.start_file("word/styles.xml", options)
        .map_err(|e| ConvertError::IoError(e.to_string()))?;
    zip.write_all(br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:styles xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:docDefaults>
    <w:rPrDefault>
      <w:rPr>
        <w:rFonts w:ascii="Calibri" w:hAnsi="Calibri" w:cs="Calibri"/>
        <w:sz w:val="22"/>
        <w:szCs w:val="22"/>
        <w:lang w:val="en-US"/>
      </w:rPr>
    </w:rPrDefault>
  </w:docDefaults>
  <w:style w:type="paragraph" w:default="1" w:styleId="Normal">
    <w:name w:val="Normal"/>
    <w:qFormat/>
  </w:style>
</w:styles>"#)
        .map_err(|e| ConvertError::IoError(e.to_string()))?;

    // word/document.xml
    zip.start_file("word/document.xml", options)
        .map_err(|e| ConvertError::IoError(e.to_string()))?;

    let mut doc_xml = String::from(r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:body>
"#);

    for p in paragraphs {
        let escaped = xml_escape_text(p);
        doc_xml.push_str(&format!(
            "    <w:p><w:r><w:t xml:space=\"preserve\">{}</w:t></w:r></w:p>\n",
            escaped
        ));
    }

    doc_xml.push_str("    <w:sectPr/>\n  </w:body>\n</w:document>");

    zip.write_all(doc_xml.as_bytes())
        .map_err(|e| ConvertError::IoError(e.to_string()))?;

    zip.finish()
        .map_err(|e| ConvertError::IoError(e.to_string()))?;

    Ok(())
}

pub fn render_image_pdf_to_docx(
    src: &Path,
    layouts: &[crate::pdfium::PageLayout],
    output: &Path,
) -> Result<(), ConvertError> {
    let pdfium = crate::pdfium::init_pdfium()?;
    let document = pdfium
        .load_pdf_from_file(src, None)
        .map_err(|e| ConvertError::ConverterError(format!("Pdfium failed to open PDF: {}", e)))?;
    render_image_pdf_doc_to_docx(&document, layouts, output)
}

fn render_image_pdf_doc_to_docx(
    document: &pdfium_render::prelude::PdfDocument,
    layouts: &[crate::pdfium::PageLayout],
    output: &Path,
) -> Result<(), ConvertError> {
    use std::io::Write;

    let file = std::fs::File::create(output).map_err(|e| ConvertError::IoError(e.to_string()))?;
    let mut zip = zip::ZipWriter::new(file);
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);

    // 1. Render each page to an image directly from the open document and run Windows OCR
    let mut page_images: Vec<(usize, Vec<u8>, f32, f32, Option<String>)> = Vec::new();
    for (page_idx, layout) in layouts.iter().enumerate() {
        let target_width = (layout.width_pt * 2.5).clamp(1600.0, 2400.0) as i32;
        let img = crate::pdfium::render_page_to_image_from_doc(document, page_idx, target_width)?;
        let pw = img.width() as f32;
        let ph = img.height() as f32;

        // Run Windows OCR on the page image so the DOCX has selectable/editable text
        let rgba = img.to_rgba8();
        let ocr_text = crate::ocr::ocr_rgba_bytes(img.width(), img.height(), &rgba)
            .ok()
            .map(|t| {
                if crate::indic::is_mojibake_or_legacy_font(&t, None) {
                    crate::indic::convert_legacy_text(&t, None)
                } else {
                    clean_pdf_text(&t)
                }
            })
            .filter(|t| !t.trim().is_empty());

        let mut png_bytes = Vec::new();
        img.write_to(&mut std::io::Cursor::new(&mut png_bytes), image::ImageFormat::Png)
            .map_err(|e| ConvertError::ConverterError(format!("Failed to encode page image: {}", e)))?;
        page_images.push((page_idx, png_bytes, pw, ph, ocr_text));
    }

    // 2. [Content_Types].xml
    zip.start_file("[Content_Types].xml", options)
        .map_err(|e| ConvertError::IoError(e.to_string()))?;
    zip.write_all(br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
  <Default Extension="xml" ContentType="application/xml"/>
  <Default Extension="png" ContentType="image/png"/>
  <Default Extension="jpg" ContentType="image/jpeg"/>
  <Default Extension="jpeg" ContentType="image/jpeg"/>
  <Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/>
  <Override PartName="/word/styles.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.styles+xml"/>
</Types>"#)
        .map_err(|e| ConvertError::IoError(e.to_string()))?;

    // 3. _rels/.rels
    zip.start_file("_rels/.rels", options)
        .map_err(|e| ConvertError::IoError(e.to_string()))?;
    zip.write_all(br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/>
</Relationships>"#)
        .map_err(|e| ConvertError::IoError(e.to_string()))?;

    // 4. word/_rels/document.xml.rels
    zip.start_file("word/_rels/document.xml.rels", options)
        .map_err(|e| ConvertError::IoError(e.to_string()))?;
    let mut d_rels = String::from(r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles" Target="styles.xml"/>
"#);
    for (page_idx, _, _, _, _) in &page_images {
        let rel_id = page_idx + 2;
        d_rels.push_str(&format!(
            "  <Relationship Id=\"rId{}\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/image\" Target=\"media/image{}.png\"/>\n",
            rel_id,
            page_idx + 1
        ));
    }
    d_rels.push_str("</Relationships>");
    zip.write_all(d_rels.as_bytes()).map_err(|e| ConvertError::IoError(e.to_string()))?;

    // 5. word/styles.xml
    zip.start_file("word/styles.xml", options)
        .map_err(|e| ConvertError::IoError(e.to_string()))?;
    zip.write_all(br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:styles xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:docDefaults>
    <w:rPrDefault>
      <w:rPr>
        <w:rFonts w:ascii="Calibri" w:hAnsi="Calibri" w:cs="Calibri"/>
        <w:sz w:val="22"/>
        <w:szCs w:val="22"/>
        <w:lang w:val="en-US"/>
      </w:rPr>
    </w:rPrDefault>
  </w:docDefaults>
  <w:style w:type="paragraph" w:default="1" w:styleId="Normal">
    <w:name w:val="Normal"/>
    <w:qFormat/>
  </w:style>
</w:styles>"#).map_err(|e| ConvertError::IoError(e.to_string()))?;

    // 6. Write image files into word/media/image{N}.png
    for (page_idx, bytes, _, _, _) in &page_images {
        zip.start_file(format!("word/media/image{}.png", page_idx + 1), options)
            .map_err(|e| ConvertError::IoError(e.to_string()))?;
        zip.write_all(bytes).map_err(|e| ConvertError::IoError(e.to_string()))?;
    }

    // 7. word/document.xml
    zip.start_file("word/document.xml", options)
        .map_err(|e| ConvertError::IoError(e.to_string()))?;

    let mut doc_xml = String::from(r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"
            xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"
            xmlns:wp="http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing"
            xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"
            xmlns:pic="http://schemas.openxmlformats.org/drawingml/2006/picture">
  <w:body>
"#);

    let first_layout = &layouts[0];
    let page_w_pt = if first_layout.width_pt > 0.0 { first_layout.width_pt } else { 612.0 };
    let page_h_pt = if first_layout.height_pt > 0.0 { first_layout.height_pt } else { 792.0 };
    let is_landscape = page_w_pt > page_h_pt;

    let margin_dxa = 360; // 0.25 inch margin (18 pt)
    let margin_pt = 18.0;
    let printable_w_emu = ((page_w_pt - 2.0 * margin_pt).max(72.0) * 12700.0).round() as f32;
    let printable_h_emu = ((page_h_pt - 2.0 * margin_pt).max(72.0) * 12700.0).round() as f32;

    for (page_idx, _, pw, ph, ocr_text) in &page_images {
        if *page_idx > 0 {
            doc_xml.push_str("    <w:p><w:pPr><w:spacing w:before=\"0\" w:after=\"0\"/></w:pPr><w:r><w:br w:type=\"page\"/></w:r></w:p>\n");
        }

        let aspect = pw / ph.max(1.0);
        let (cx, cy) = if printable_w_emu / printable_h_emu > aspect {
            ((printable_h_emu * aspect).round() as u64, printable_h_emu.round() as u64)
        } else {
            (printable_w_emu.round() as u64, (printable_w_emu / aspect).round() as u64)
        };

        let pic_id = page_idx + 1;
        let rel_id = page_idx + 2;

        doc_xml.push_str(&format!(r#"    <w:p>
      <w:pPr>
        <w:jc w:val="center"/>
        <w:spacing w:before="0" w:after="0"/>
      </w:pPr>
      <w:r>
        <w:drawing>
          <wp:inline distT="0" distB="0" distL="0" distR="0">
            <wp:extent cx="{}" cy="{}"/>
            <wp:effectExtent l="0" t="0" r="0" b="0"/>
            <wp:docPr id="{}" name="Page {}"/>
            <wp:cNvGraphicFramePr>
              <a:graphicFrameLocks xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" noChangeAspect="1"/>
            </wp:cNvGraphicFramePr>
            <a:graphic xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main">
              <a:graphicData uri="http://schemas.openxmlformats.org/drawingml/2006/picture">
                <pic:pic xmlns:pic="http://schemas.openxmlformats.org/drawingml/2006/picture">
                  <pic:nvPicPr>
                    <pic:cNvPr id="{}" name="PageImage {}"/>
                    <pic:cNvPicPr/>
                  </pic:nvPicPr>
                  <pic:blipFill>
                    <a:blip r:embed="rId{}"/>
                    <a:stretch><a:fillRect/></a:stretch>
                  </pic:blipFill>
                  <pic:spPr>
                    <a:xfrm>
                      <a:off x="0" y="0"/>
                      <a:ext cx="{}" cy="{}"/>
                    </a:xfrm>
                    <a:prstGeom prst="rect"><a:avLst/></a:prstGeom>
                  </pic:spPr>
                </pic:pic>
              </a:graphicData>
            </a:graphic>
          </wp:inline>
        </w:drawing>
      </w:r>
    </w:p>
"#, cx, cy, pic_id, pic_id, pic_id, pic_id, rel_id, cx, cy));

        // If OCR extracted text, emit selectable and editable text paragraphs
        if let Some(text) = ocr_text {
            for line in text.lines() {
                let trimmed = line.trim();
                if !trimmed.is_empty() {
                    let esc = xml_escape_text(trimmed);
                    doc_xml.push_str(&format!(
                        "    <w:p><w:pPr><w:spacing w:before=\"60\" w:after=\"60\"/></w:pPr><w:r><w:t xml:space=\"preserve\">{}</w:t></w:r></w:p>\n",
                        esc
                    ));
                }
            }
        }
    }

    let page_w_dxa = (page_w_pt * 20.0).round() as u64;
    let page_h_dxa = (page_h_pt * 20.0).round() as u64;
    let orient_attr = if is_landscape { " w:orient=\"landscape\"" } else { "" };

    doc_xml.push_str(&format!(r#"    <w:sectPr>
      <w:pgSz w:w="{}" w:h="{}"{}/>
      <w:pgMar w:top="{}" w:right="{}" w:bottom="{}" w:left="{}" w:header="0" w:footer="0" w:gutter="0"/>
    </w:sectPr>
  </w:body>
</w:document>"#, page_w_dxa, page_h_dxa, orient_attr, margin_dxa, margin_dxa, margin_dxa, margin_dxa));

    zip.write_all(doc_xml.as_bytes()).map_err(|e| ConvertError::IoError(e.to_string()))?;
    zip.finish().map_err(|e| ConvertError::IoError(e.to_string()))?;

    Ok(())
}

fn render_structured_layout_to_docx(
    src: &Path,
    layouts: &[crate::pdfium::PageLayout],
    output: &Path,
) -> Result<(), ConvertError> {
    use std::io::Write;

    let file = std::fs::File::create(output).map_err(|e| ConvertError::IoError(e.to_string()))?;
    let mut zip = zip::ZipWriter::new(file);
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);

    // 1. Gather all embedded images across layouts
    let mut media_files: Vec<(String, Vec<u8>)> = Vec::new();
    let mut image_rel_map: std::collections::HashMap<(usize, usize), (String, usize)> = std::collections::HashMap::new();

    for (page_idx, layout) in layouts.iter().enumerate() {
        if layout.items.is_empty() && layout.images.is_empty() {
            let target_width = (layout.width_pt * 2.5).clamp(1600.0, 2400.0) as i32;
            if let Ok(img) = crate::pdfium::render_page_to_image(src, page_idx, target_width) {
                let mut png_bytes = Vec::new();
                if img.write_to(&mut std::io::Cursor::new(&mut png_bytes), image::ImageFormat::Png).is_ok() {
                    let filename = format!("rendered_p{}.png", page_idx + 1);
                    let rel_id = media_files.len() + 2;
                    media_files.push((filename.clone(), png_bytes));
                    image_rel_map.insert((page_idx, 9999), (filename, rel_id));
                }
            }
        }
        for (img_i, img) in layout.images.iter().enumerate() {
            let filename = format!("img_p{}_i{}.png", page_idx + 1, img_i + 1);
            let rel_id = media_files.len() + 2;
            media_files.push((filename.clone(), img.data.clone()));
            image_rel_map.insert((page_idx, img_i), (filename, rel_id));
        }
    }

    // 2. [Content_Types].xml
    zip.start_file("[Content_Types].xml", options)
        .map_err(|e| ConvertError::IoError(e.to_string()))?;
    zip.write_all(br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
  <Default Extension="xml" ContentType="application/xml"/>
  <Default Extension="png" ContentType="image/png"/>
  <Default Extension="jpg" ContentType="image/jpeg"/>
  <Default Extension="jpeg" ContentType="image/jpeg"/>
  <Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/>
  <Override PartName="/word/styles.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.styles+xml"/>
</Types>"#)
        .map_err(|e| ConvertError::IoError(e.to_string()))?;

    // 3. _rels/.rels
    zip.start_file("_rels/.rels", options)
        .map_err(|e| ConvertError::IoError(e.to_string()))?;
    zip.write_all(br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/>
</Relationships>"#)
        .map_err(|e| ConvertError::IoError(e.to_string()))?;

    // 4. word/_rels/document.xml.rels
    zip.start_file("word/_rels/document.xml.rels", options)
        .map_err(|e| ConvertError::IoError(e.to_string()))?;
    let mut d_rels = String::from(r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles" Target="styles.xml"/>
"#);
    for (idx, (filename, _)) in media_files.iter().enumerate() {
        d_rels.push_str(&format!(
            "  <Relationship Id=\"rId{}\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/image\" Target=\"media/{}\"/>\n",
            idx + 2,
            filename
        ));
    }
    d_rels.push_str("</Relationships>");
    zip.write_all(d_rels.as_bytes()).map_err(|e| ConvertError::IoError(e.to_string()))?;

    // 5. word/styles.xml
    zip.start_file("word/styles.xml", options)
        .map_err(|e| ConvertError::IoError(e.to_string()))?;
    zip.write_all(br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:styles xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:docDefaults>
    <w:rPrDefault>
      <w:rPr>
        <w:rFonts w:ascii="Calibri" w:hAnsi="Calibri" w:cs="Calibri"/>
        <w:sz w:val="22"/>
        <w:szCs w:val="22"/>
        <w:lang w:val="en-US"/>
      </w:rPr>
    </w:rPrDefault>
  </w:docDefaults>
  <w:style w:type="paragraph" w:default="1" w:styleId="Normal">
    <w:name w:val="Normal"/>
    <w:qFormat/>
  </w:style>
</w:styles>"#).map_err(|e| ConvertError::IoError(e.to_string()))?;

    // 6. Write media files
    for (filename, bytes) in &media_files {
        zip.start_file(format!("word/media/{}", filename), options)
            .map_err(|e| ConvertError::IoError(e.to_string()))?;
        zip.write_all(bytes).map_err(|e| ConvertError::IoError(e.to_string()))?;
    }

    // 7. word/document.xml
    zip.start_file("word/document.xml", options)
        .map_err(|e| ConvertError::IoError(e.to_string()))?;

    let mut doc_xml = String::from(r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"
            xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"
            xmlns:wp="http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing"
            xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"
            xmlns:pic="http://schemas.openxmlformats.org/drawingml/2006/picture">
  <w:body>
"#);

    let has_any_outer_border = layouts.iter().any(|l| l.has_outer_border);
    let mut pic_counter = 100usize;

    for (page_idx, layout) in layouts.iter().enumerate() {
        if page_idx > 0 {
            // Page break
            doc_xml.push_str("    <w:p><w:r><w:br w:type=\"page\"/></w:r></w:p>\n");
        }

        let mut emitted_img_indices = std::collections::HashSet::new();

        if layout.items.is_empty() && layout.images.is_empty() {
            if let Some((_, rel_id)) = image_rel_map.get(&(page_idx, 9999)) {
                pic_counter += 1;
                let max_w_emu = 5486400.0;
                let aspect = layout.width_pt / layout.height_pt.max(1.0);
                let img_w_emu = max_w_emu;
                let img_h_emu = (img_w_emu / aspect).round();

                doc_xml.push_str(&format!(r#"    <w:p>
      <w:pPr><w:jc w:val="center"/><w:spacing w:before="0" w:after="0"/></w:pPr>
      <w:r>
        <w:drawing>
          <wp:inline distT="0" distB="0" distL="0" distR="0">
            <wp:extent cx="{}" cy="{}"/>
            <wp:effectExtent l="0" t="0" r="0" b="0"/>
            <wp:docPr id="{}" name="Picture {}"/>
            <wp:cNvGraphicFramePr>
              <a:graphicFrameLocks xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" noChangeAspect="1"/>
            </wp:cNvGraphicFramePr>
            <a:graphic xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main">
              <a:graphicData uri="http://schemas.openxmlformats.org/drawingml/2006/picture">
                <pic:pic xmlns:pic="http://schemas.openxmlformats.org/drawingml/2006/picture">
                  <pic:nvPicPr>
                    <pic:cNvPr id="{}" name="Image {}"/>
                    <pic:cNvPicPr/>
                  </pic:nvPicPr>
                  <pic:blipFill>
                    <a:blip r:embed="rId{}"/>
                    <a:stretch><a:fillRect/></a:stretch>
                  </pic:blipFill>
                  <pic:spPr>
                    <a:xfrm>
                      <a:off x="0" y="0"/>
                      <a:ext cx="{}" cy="{}"/>
                    </a:xfrm>
                    <a:prstGeom prst="rect"><a:avLst/></a:prstGeom>
                  </pic:spPr>
                </pic:pic>
              </a:graphicData>
            </a:graphic>
          </wp:inline>
        </w:drawing>
      </w:r>
    </w:p>
"#, img_w_emu as u64, img_h_emu as u64, pic_counter, pic_counter, pic_counter, pic_counter, rel_id, img_w_emu as u64, img_h_emu as u64));
            }
        }

        let mut items = layout.items.clone();

        // 1. Sort items by vertical position top-to-bottom, then left-to-right
        items.sort_by(|a, b| {
            if (a.y - b.y).abs() <= 5.0 {
                a.x.partial_cmp(&b.x).unwrap_or(std::cmp::Ordering::Equal)
            } else {
                a.y.partial_cmp(&b.y).unwrap_or(std::cmp::Ordering::Equal)
            }
        });

        // 2. Cluster into horizontal rows
        let mut rows: Vec<Vec<crate::pdfium::LayoutItem>> = Vec::new();
        let mut cur_row: Vec<crate::pdfium::LayoutItem> = Vec::new();
        let mut cur_row_y = 0.0;

        for item in items {
            if cur_row.is_empty() {
                cur_row_y = item.y;
                cur_row.push(item);
            } else if (item.y - cur_row_y).abs() <= 5.0 {
                cur_row.push(item);
            } else {
                rows.push(std::mem::take(&mut cur_row));
                cur_row_y = item.y;
                cur_row.push(item);
            }
        }
        if !cur_row.is_empty() {
            rows.push(cur_row);
        }

        let mut prev_row_bottom_y = 0.0;

        for row in &rows {
            if row.is_empty() {
                continue;
            }

            let row_y = row[0].y;

            // Emit any page images that are located above this row
            for (img_i, img) in layout.images.iter().enumerate() {
                if !emitted_img_indices.contains(&img_i) && img.y <= row_y {
                    emitted_img_indices.insert(img_i);
                    if let Some((_, rel_id)) = image_rel_map.get(&(page_idx, img_i)) {
                        pic_counter += 1;
                        let max_w_emu = 5486400.0; // 6 inches in EMUs
                        let aspect = img.width / img.height.max(1.0);
                        let img_w_emu = (img.width * 12700.0).round().min(max_w_emu);
                        let img_h_emu = (img_w_emu / aspect).round();

                        doc_xml.push_str(&format!(r#"    <w:p>
      <w:pPr><w:jc w:val="center"/><w:spacing w:before="120" w:after="120"/></w:pPr>
      <w:r>
        <w:drawing>
          <wp:inline distT="0" distB="0" distL="0" distR="0">
            <wp:extent cx="{}" cy="{}"/>
            <wp:effectExtent l="0" t="0" r="0" b="0"/>
            <wp:docPr id="{}" name="Picture {}"/>
            <wp:cNvGraphicFramePr>
              <a:graphicFrameLocks xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" noChangeAspect="1"/>
            </wp:cNvGraphicFramePr>
            <a:graphic xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main">
              <a:graphicData uri="http://schemas.openxmlformats.org/drawingml/2006/picture">
                <pic:pic xmlns:pic="http://schemas.openxmlformats.org/drawingml/2006/picture">
                  <pic:nvPicPr>
                    <pic:cNvPr id="{}" name="Image {}"/>
                    <pic:cNvPicPr/>
                  </pic:nvPicPr>
                  <pic:blipFill>
                    <a:blip r:embed="rId{}"/>
                    <a:stretch><a:fillRect/></a:stretch>
                  </pic:blipFill>
                  <pic:spPr>
                    <a:xfrm>
                      <a:off x="0" y="0"/>
                      <a:ext cx="{}" cy="{}"/>
                    </a:xfrm>
                    <a:prstGeom prst="rect"><a:avLst/></a:prstGeom>
                  </pic:spPr>
                </pic:pic>
              </a:graphicData>
            </a:graphic>
          </wp:inline>
        </w:drawing>
      </w:r>
    </w:p>
"#, img_w_emu as u64, img_h_emu as u64, pic_counter, pic_counter, pic_counter, pic_counter, rel_id, img_w_emu as u64, img_h_emu as u64));
                    }
                }
            }

            // Merge very close items in the same row
            let mut merged_items: Vec<crate::pdfium::LayoutItem> = Vec::new();
            for item in row {
                if let Some(last) = merged_items.last_mut() {
                    let gap = item.x - (last.x + last.width);
                    if gap < 8.0 && !item.is_checkbox && !last.is_checkbox {
                        last.text.push(' ');
                        last.text.push_str(&item.text);
                        last.width = (item.x + item.width) - last.x;
                        last.is_bold = last.is_bold || item.is_bold;
                        continue;
                    }
                }
                merged_items.push(item.clone());
            }

            // Check if any divider line is between prev row and this row
            for &div_y in &layout.divider_lines {
                if div_y > prev_row_bottom_y && div_y < row_y {
                    doc_xml.push_str("    <w:p><w:pPr><w:pBdr><w:bottom w:val=\"single\" w:sz=\"6\" w:space=\"1\" w:color=\"999999\"/></w:pBdr><w:spacing w:before=\"80\" w:after=\"80\"/></w:pPr></w:p>\n");
                }
            }

            let gap = if prev_row_bottom_y > 0.0 {
                (row_y - prev_row_bottom_y).max(0.0)
            } else {
                0.0
            };
            let spacing_before = if gap > 14.0 {
                ((gap - 10.0) * 20.0).min(800.0).round() as i32
            } else {
                0
            };

            prev_row_bottom_y = row_y + merged_items[0].height;

            // Format paragraph
            if merged_items.len() == 1 {
                let item = &merged_items[0];
                let mid = item.x + item.width / 2.0;
                let page_mid = layout.width_pt / 2.0;
                let is_centered = (mid - page_mid).abs() < 40.0 && item.width < layout.width_pt * 0.75;

                doc_xml.push_str("    <w:p><w:pPr>");
                if is_centered {
                    doc_xml.push_str("<w:jc w:val=\"center\"/>");
                } else if item.x > 60.0 {
                    let ind_dxa = (item.x * 20.0).round() as i32;
                    doc_xml.push_str(&format!("<w:ind w:left=\"{}\"/>", ind_dxa));
                }
                if spacing_before > 0 {
                    doc_xml.push_str(&format!("<w:spacing w:before=\"{}\" w:after=\"0\" w:line=\"440\" w:lineRule=\"atLeast\"/>", spacing_before));
                } else {
                    doc_xml.push_str("<w:spacing w:before=\"0\" w:after=\"0\" w:line=\"440\" w:lineRule=\"atLeast\"/>");
                }
                doc_xml.push_str("</w:pPr>");

                // Run
                doc_xml.push_str("<w:r>");
                if item.is_bold {
                    doc_xml.push_str("<w:rPr><w:b/><w:bCs/></w:rPr>");
                }
                let esc = xml_escape_text(&item.text);
                doc_xml.push_str(&format!("<w:t xml:space=\"preserve\">{}</w:t></w:r></w:p>\n", esc));
            } else {
                // Multi-column or side-by-side elements on the same row: use OpenXML tabs
                doc_xml.push_str("    <w:p><w:pPr><w:tabs>");
                for item in &merged_items[1..] {
                    let tab_pos = (item.x * 20.0).round() as i32;
                    doc_xml.push_str(&format!("<w:tab w:val=\"left\" w:pos=\"{}\"/>", tab_pos));
                }
                doc_xml.push_str("</w:tabs>");
                if merged_items[0].x > 60.0 {
                    let ind_dxa = (merged_items[0].x * 20.0).round() as i32;
                    doc_xml.push_str(&format!("<w:ind w:left=\"{}\"/>", ind_dxa));
                }
                if spacing_before > 0 {
                    doc_xml.push_str(&format!("<w:spacing w:before=\"{}\" w:after=\"0\" w:line=\"440\" w:lineRule=\"atLeast\"/>", spacing_before));
                } else {
                    doc_xml.push_str("<w:spacing w:before=\"0\" w:after=\"0\" w:line=\"440\" w:lineRule=\"atLeast\"/>");
                }
                doc_xml.push_str("</w:pPr>");

                // First item
                doc_xml.push_str("<w:r>");
                if merged_items[0].is_bold {
                    doc_xml.push_str("<w:rPr><w:b/><w:bCs/></w:rPr>");
                }
                let esc0 = xml_escape_text(&merged_items[0].text);
                doc_xml.push_str(&format!("<w:t xml:space=\"preserve\">{}</w:t></w:r>", esc0));

                // Subsequent items separated by <w:tab/>
                for item in &merged_items[1..] {
                    doc_xml.push_str("<w:r><w:tab/></w:r>");
                    doc_xml.push_str("<w:r>");
                    if item.is_bold {
                        doc_xml.push_str("<w:rPr><w:b/><w:bCs/></w:rPr>");
                    }
                    let esc = xml_escape_text(&item.text);
                    doc_xml.push_str(&format!("<w:t xml:space=\"preserve\">{}</w:t></w:r>", esc));
                }
                doc_xml.push_str("</w:p>\n");
            }
        }

        // Emit any remaining page images
        for (img_i, img) in layout.images.iter().enumerate() {
            if !emitted_img_indices.contains(&img_i) {
                emitted_img_indices.insert(img_i);
                if let Some((_, rel_id)) = image_rel_map.get(&(page_idx, img_i)) {
                    pic_counter += 1;
                    let max_w_emu = 5486400.0; // 6 inches in EMUs
                    let aspect = img.width / img.height.max(1.0);
                    let img_w_emu = (img.width * 12700.0).round().min(max_w_emu);
                    let img_h_emu = (img_w_emu / aspect).round();

                    doc_xml.push_str(&format!(r#"    <w:p>
      <w:pPr><w:jc w:val="center"/><w:spacing w:before="120" w:after="120"/></w:pPr>
      <w:r>
        <w:drawing>
          <wp:inline distT="0" distB="0" distL="0" distR="0">
            <wp:extent cx="{}" cy="{}"/>
            <wp:effectExtent l="0" t="0" r="0" b="0"/>
            <wp:docPr id="{}" name="Picture {}"/>
            <wp:cNvGraphicFramePr>
              <a:graphicFrameLocks xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" noChangeAspect="1"/>
            </wp:cNvGraphicFramePr>
            <a:graphic xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main">
              <a:graphicData uri="http://schemas.openxmlformats.org/drawingml/2006/picture">
                <pic:pic xmlns:pic="http://schemas.openxmlformats.org/drawingml/2006/picture">
                  <pic:nvPicPr>
                    <pic:cNvPr id="{}" name="Image {}"/>
                    <pic:cNvPicPr/>
                  </pic:nvPicPr>
                  <pic:blipFill>
                    <a:blip r:embed="rId{}"/>
                    <a:stretch><a:fillRect/></a:stretch>
                  </pic:blipFill>
                  <pic:spPr>
                    <a:xfrm>
                      <a:off x="0" y="0"/>
                      <a:ext cx="{}" cy="{}"/>
                    </a:xfrm>
                    <a:prstGeom prst="rect"><a:avLst/></a:prstGeom>
                  </pic:spPr>
                </pic:pic>
              </a:graphicData>
            </a:graphic>
          </wp:inline>
        </w:drawing>
      </w:r>
    </w:p>
"#, img_w_emu as u64, img_h_emu as u64, pic_counter, pic_counter, pic_counter, pic_counter, rel_id, img_w_emu as u64, img_h_emu as u64));
                }
            }
        }
    }

    // Section properties & outer page borders
    let first_layout = &layouts[0];
    let page_w_pt = if first_layout.width_pt > 0.0 { first_layout.width_pt } else { 612.0 };
    let page_h_pt = if first_layout.height_pt > 0.0 { first_layout.height_pt } else { 792.0 };
    let is_landscape = page_w_pt > page_h_pt;
    let page_w_dxa = (page_w_pt * 20.0).round() as u64;
    let page_h_dxa = (page_h_pt * 20.0).round() as u64;
    let orient_attr = if is_landscape { " w:orient=\"landscape\"" } else { "" };

    if has_any_outer_border {
        doc_xml.push_str(&format!(r#"    <w:sectPr>
      <w:pgSz w:w="{}" w:h="{}"{}/>
      <w:pgMar w:top="720" w:right="720" w:bottom="720" w:left="720" w:header="720" w:footer="720" w:gutter="0"/>
      <w:pgBdr w:offsetFrom="page">
        <w:top w:val="single" w:sz="8" w:space="24" w:color="000000"/>
        <w:left w:val="single" w:sz="8" w:space="24" w:color="000000"/>
        <w:bottom w:val="single" w:sz="8" w:space="24" w:color="000000"/>
        <w:right w:val="single" w:sz="8" w:space="24" w:color="000000"/>
      </w:pgBdr>
    </w:sectPr>
  </w:body>
</w:document>"#, page_w_dxa, page_h_dxa, orient_attr));
    } else {
        doc_xml.push_str(&format!(r#"    <w:sectPr>
      <w:pgSz w:w="{}" w:h="{}"{}/>
      <w:pgMar w:top="720" w:right="720" w:bottom="720" w:left="720" w:header="0" w:footer="0" w:gutter="0"/>
    </w:sectPr>
  </w:body>
</w:document>"#, page_w_dxa, page_h_dxa, orient_attr));
    }

    zip.write_all(doc_xml.as_bytes())
        .map_err(|e| ConvertError::IoError(e.to_string()))?;
    zip.finish()
        .map_err(|e| ConvertError::IoError(e.to_string()))?;

    Ok(())
}

pub fn pdf_to_doc(src: &Path, output: &Path) -> Result<(), ConvertError> {
    let paragraphs = extract_pdf_paragraphs(src)?;

    let mut rtf = String::from(
        r#"{\rtf1\ansi\ansicpg1252\deff0\nouicompat
{\fonttbl{\f0\froman\fcharset0 Times New Roman;}{\f1\fswiss\fcharset0 Arial;}{\f2\fnil\fcharset0 Calibri;}}
{\colortbl ;\red0\green0\blue0;}
\viewkind4\uc1 
\pard\sa200\sl276\slmult1\f2\fs22\lang1033 
"#,
    );

    for p in &paragraphs {
        let mut escaped = String::new();
        for c in p.chars() {
            match c {
                '\\' => escaped.push_str(r"\\"),
                '{' => escaped.push_str(r"\{"),
                '}' => escaped.push_str(r"\}"),
                // Standard printable ASCII
                c if (c as u32) >= 0x20 && (c as u32) <= 0x7E => escaped.push(c),
                // Windows-1252 high ANSI
                c if (c as u32) >= 0xA0 && (c as u32) <= 0xFF => {
                    escaped.push_str(&format!(r"\'{:02x}", c as u8));
                }
                // Unicode characters
                c => {
                    let code = c as u32;
                    let signed = if code <= 32767 {
                        code as i32
                    } else {
                        code as i32 - 65536
                    };
                    escaped.push_str(&format!(r"\u{} ", signed));
                }
            }
        }
        rtf.push_str(&escaped);
        rtf.push_str("\\par\r\n");
    }

    rtf.push_str("}\r\n");

    std::fs::write(output, rtf.as_bytes()).map_err(|e| ConvertError::IoError(e.to_string()))?;
    Ok(())
}
