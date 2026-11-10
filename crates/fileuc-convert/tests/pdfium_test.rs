use pdfium_render::prelude::*;

#[test]
fn test_pdfium_initialization() {
    let bindings = Pdfium::bind_to_library(Pdfium::pdfium_platform_library_name_at_path("./"))
        .or_else(|_| Pdfium::bind_to_library("pdfium.dll"))
        .or_else(|_| Pdfium::bind_to_library(r"C:\Program Files\Filee\pdfium.dll"))
        .or_else(|_| Pdfium::bind_to_system_library());

    match bindings {
        Ok(b) => {
            let pdfium = Pdfium::new(b);
            println!("Pdfium initialized successfully!");
            let doc = pdfium.create_new_pdf();
            assert!(doc.is_ok());
            println!("Pdfium created new PDF successfully!");
        }
        Err(e) => {
            panic!("Pdfium bindings failed: {:?}", e);
        }
    }
}

#[test]
fn test_inspect_student_application_layout() {
    let pdf_path = std::path::Path::new(r"C:\Users\Sharad Chand\Downloads\StudentbuspassApplication.pdf");
    if !pdf_path.exists() {
        return;
    }
    let pdfium = fileuc_convert::pdfium::init_pdfium().unwrap();
    let doc = pdfium.load_pdf_from_file(pdf_path, None).unwrap();
    let page = doc.pages().get(0).unwrap();

    println!("Page width: {:?}, height: {:?}", page.width(), page.height());
    println!("Total objects on page 0: {}", page.objects().len());

    let mut text_count = 0;
    let mut path_count = 0;
    let mut image_count = 0;

    for (idx, obj) in page.objects().iter().enumerate() {
        let bounds = obj.bounds();
        let obj_type = obj.object_type();
        match obj_type {
            PdfPageObjectType::Text => {
                text_count += 1;
                if let Some(text_obj) = obj.as_text_object() {
                    let text = text_obj.text();
                    let trimmed = text.trim();
                    if !trimmed.is_empty() {
                        let quad = bounds.unwrap();
                        println!("TextObj #{}: x={:.1}, y={:.1}, text={:?}", idx, quad.x1().value, quad.y1().value, trimmed);
                    }
                }
            }
            PdfPageObjectType::Path => {
                path_count += 1;
                if let Ok(b) = bounds {
                    let w = (b.x2().value - b.x1().value).abs();
                    let h = (b.y2().value - b.y1().value).abs();
                    println!("PathObj #{}: x1={:.1}, y1={:.1}, w={:.1}, h={:.1}", idx, b.x1().value, b.y1().value, w, h);
                }
            }
            PdfPageObjectType::Image => {
                image_count += 1;
                println!("ImageObj #{}: bounds={:?}", idx, bounds);
                if let Some(img_obj) = obj.as_image_object() {
                    if let Ok(raw_img) = img_obj.get_raw_image() {
                        println!("Image dimensions: {}x{}", raw_img.width(), raw_img.height());
                    }
                }
            }
            _ => {}
        }
    }
    println!("Summary: text={}, path={}, image={}", text_count, path_count, image_count);
}

