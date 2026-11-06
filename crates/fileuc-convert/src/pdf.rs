use fileuc_types::ConvertError;
use lopdf::{Dictionary, Document, Object};
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

    let mut max_id = 1;
    let mut target_doc = Document::with_version("1.5");
    let mut target_pages = Vec::new();

    for path in pdfs {
        if !path.exists() {
            return Err(ConvertError::SourceNotFound(path.to_string_lossy().to_string()));
        }

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
    let mut outputs = Vec::new();

    let doc = Document::load(src)
        .map_err(|e| ConvertError::ConverterError(format!("Failed to load PDF: {}", e)))?;

    let pages = doc.get_pages();
    for (page_idx, (&page_num, _)) in pages.iter().enumerate() {
        let mut doc_copy = doc.clone();
        let pages_to_prune: Vec<u32> = pages.keys().copied().filter(|&p| p != page_num).collect();
        doc_copy.delete_pages(&pages_to_prune);

        let out_path = output_dir.join(format!("{}_page_{}.pdf", stem, page_idx + 1));
        doc_copy
            .save(&out_path)
            .map_err(|e| ConvertError::ConverterError(format!("Failed to save page PDF: {}", e)))?;
        outputs.push(out_path);
    }

    Ok(outputs)
}

pub fn compress_pdf(src: &Path, output: &Path) -> Result<(), ConvertError> {
    let mut doc = Document::load(src)
        .map_err(|e| ConvertError::ConverterError(format!("Failed to load PDF: {}", e)))?;
    doc.compress();
    doc.save(output)
        .map_err(|e| ConvertError::ConverterError(format!("Failed to compress PDF: {}", e)))?;
    Ok(())
}
