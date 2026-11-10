use crate::ir::types::*;
use fileuc_types::ConvertError;
use std::io::Write;
use std::path::Path;

static PPTX_BLANK_TEMPLATE: &[u8] = include_bytes!("../../assets/blank.pptx");

const PT_TO_EMU: f32 = 12700.0;

fn xml_escape(s: &str) -> String {
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

pub fn emit_pptx(doc: &DocumentIR, output: &Path) -> Result<(), ConvertError> {
    if doc.pages.is_empty() {
        return Err(ConvertError::ConverterError("Document has no pages to emit".to_string()));
    }

    let file = std::fs::File::create(output).map_err(|e| ConvertError::IoError(e.to_string()))?;
    let mut zip = zip::ZipWriter::new(file);
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);

    let first_page = &doc.pages[0];
    let slide_cx = (first_page.width_pt * PT_TO_EMU).round() as u64;
    let slide_cy = (first_page.height_pt * PT_TO_EMU).round() as u64;

    copy_template_base(&mut zip, options, doc.pages.len(), slide_cx, slide_cy)?;

    for (page_idx, page) in doc.pages.iter().enumerate() {
        let slide_num = page_idx + 1;

        // 1. Write slide relationships
        zip.start_file(format!("ppt/slides/_rels/slide{}.xml.rels", slide_num), options)
            .map_err(|e| ConvertError::IoError(e.to_string()))?;

        let mut s_rels = String::from(r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideLayout" Target="../slideLayouts/slideLayout7.xml"/>
"#);

        for (img_i, _) in page.images.iter().enumerate() {
            let img_file = format!("img_{}_{}.png", slide_num, img_i + 1);
            s_rels.push_str(&format!(
                "  <Relationship Id=\"rIdImg{}\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/image\" Target=\"../media/{}\"/>\n",
                img_i + 1,
                img_file
            ));
        }
        s_rels.push_str("</Relationships>");
        zip.write_all(s_rels.as_bytes()).map_err(|e| ConvertError::IoError(e.to_string()))?;

        // 2. Write embedded images
        for (img_i, img) in page.images.iter().enumerate() {
            let img_file = format!("ppt/media/img_{}_{}.png", slide_num, img_i + 1);
            zip.start_file(img_file, options)
                .map_err(|e| ConvertError::IoError(e.to_string()))?;
            zip.write_all(&img.data).map_err(|e| ConvertError::IoError(e.to_string()))?;
        }

        // 3. Write slide XML
        zip.start_file(format!("ppt/slides/slide{}.xml", slide_num), options)
            .map_err(|e| ConvertError::IoError(e.to_string()))?;

        let mut s_xml = String::from(r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<p:sld xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" 
       xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" 
       xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main">
  <p:cSld>
    <p:spTree>
      <p:nvGrpSpPr><p:cNvPr id="1" name=""/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr>
      <p:grpSpPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="0" cy="0"/><a:chOff x="0" y="0"/><a:chExt cx="0" cy="0"/></a:xfrm></p:grpSpPr>
"#);

        let mut shape_id = 2u32;

        // Emit Shapes (Lines and Rectangles)
        for shape in &page.shapes {
            let sx = (shape.bounds.x * PT_TO_EMU).round() as i64;
            let sy = (shape.bounds.y * PT_TO_EMU).round() as i64;
            let scx = (shape.bounds.width * PT_TO_EMU).round().max(12700.0) as i64;
            let scy = (shape.bounds.height * PT_TO_EMU).round().max(12700.0) as i64;

            let fill_xml = if let Some(rgb) = shape.fill_color {
                format!("<a:solidFill><a:srgbClr val=\"{:02X}{:02X}{:02X}\"/></a:solidFill>", rgb[0], rgb[1], rgb[2])
            } else {
                "<a:noFill/>".to_string()
            };

            let stroke_xml = if let Some(rgb) = shape.stroke_color {
                let w_emu = (shape.stroke_width_pt * PT_TO_EMU).round() as i64;
                format!("<a:ln w=\"{}\"><a:solidFill><a:srgbClr val=\"{:02X}{:02X}{:02X}\"/></a:solidFill></a:ln>", w_emu, rgb[0], rgb[1], rgb[2])
            } else {
                "<a:ln><a:noFill/></a:ln>".to_string()
            };

            s_xml.push_str(&format!(r#"      <p:sp>
        <p:nvSpPr><p:cNvPr id="{}" name="Shape {}"/><p:cNvSpPr/><p:nvPr/></p:nvSpPr>
        <p:spPr>
          <a:xfrm><a:off x="{}" y="{}"/><a:ext cx="{}" cy="{}"/></a:xfrm>
          <a:prstGeom prst="rect"><a:avLst/></a:prstGeom>
          {}
          {}
        </p:spPr>
      </p:sp>
"#, shape_id, shape_id, sx, sy, scx, scy, fill_xml, stroke_xml));
            shape_id += 1;
        }

        // Emit Images
        for (img_i, img) in page.images.iter().enumerate() {
            let ix = (img.bounds.x * PT_TO_EMU).round() as i64;
            let iy = (img.bounds.y * PT_TO_EMU).round() as i64;
            let icx = (img.bounds.width * PT_TO_EMU).round().max(12700.0) as i64;
            let icy = (img.bounds.height * PT_TO_EMU).round().max(12700.0) as i64;

            s_xml.push_str(&format!(r#"      <p:pic>
        <p:nvPicPr>
          <p:cNvPr id="{}" name="Picture {}"/>
          <p:cNvPicPr><a:picLocks noChangeAspect="1"/></p:cNvPicPr>
          <p:nvPr/>
        </p:nvPicPr>
        <p:blipFill>
          <a:blip r:embed="rIdImg{}"/>
          <a:stretch><a:fillRect/></a:stretch>
        </p:blipFill>
        <p:spPr>
          <a:xfrm><a:off x="{}" y="{}"/><a:ext cx="{}" cy="{}"/></a:xfrm>
          <a:prstGeom prst="rect"><a:avLst/></a:prstGeom>
        </p:spPr>
      </p:pic>
"#, shape_id, img_i + 1, img_i + 1, ix, iy, icx, icy));
            shape_id += 1;
        }

        // Emit Tables
        for table in &page.tables {
            let tx = (table.bounds.x * PT_TO_EMU).round() as i64;
            let ty = (table.bounds.y * PT_TO_EMU).round() as i64;
            let tcx = (table.bounds.width * PT_TO_EMU).round().max(12700.0) as i64;
            let tcy = (table.bounds.height * PT_TO_EMU).round().max(12700.0) as i64;

            s_xml.push_str(&format!(r#"      <p:graphicFrame>
        <p:nvGraphicFramePr>
          <p:cNvPr id="{}" name="Table {}"/>
          <p:cNvGraphicFramePr><a:graphicFrameLocks noGrp="1"/></p:cNvGraphicFramePr>
          <p:nvPr/>
        </p:nvGraphicFramePr>
        <p:xfrm><a:off x="{}" y="{}"/><a:ext cx="{}" cy="{}"/></p:xfrm>
        <a:graphic>
          <a:graphicData uri="http://schemas.openxmlformats.org/drawingml/2006/table">
            <a:tbl>
              <a:tblPr rtl="0" firstRow="1"/>
              <a:tblGrid>
"#, shape_id, shape_id, tx, ty, tcx, tcy));
            shape_id += 1;

            for w_pt in &table.column_widths_pt {
                let w_emu = (w_pt * PT_TO_EMU).round() as i64;
                s_xml.push_str(&format!("                <a:gridCol w=\"{}\"/>\n", w_emu));
            }
            s_xml.push_str("              </a:tblGrid>\n");

            // Group cells by row
            let num_rows = table.cells.iter().map(|c| c.row).max().map(|m| m + 1).unwrap_or(0);
            for r in 0..num_rows {
                let mut row_cells: Vec<&TableCellIR> = table.cells.iter().filter(|c| c.row == r).collect();
                row_cells.sort_by_key(|c| c.col);

                let row_h_emu = row_cells.first().map(|c| (c.bounds.height * PT_TO_EMU).round() as i64).unwrap_or(370840);
                s_xml.push_str(&format!("              <a:tr h=\"{}\">\n", row_h_emu));

                for cell in row_cells {
                    s_xml.push_str("                <a:tc>\n                  <a:txBody>\n                    <a:bodyPr/><a:lstStyle/>\n");
                    for run in &cell.text_runs {
                        let sz = (run.font_size_pt * 100.0).round().max(600.0) as i32;
                        let b_val = if run.is_bold { "1" } else { "0" };
                        let i_val = if run.is_italic { "1" } else { "0" };
                        let color_hex = format!("{:02X}{:02X}{:02X}", run.color_rgb[0], run.color_rgb[1], run.color_rgb[2]);
                        let esc_text = xml_escape(&run.text);

                        s_xml.push_str(&format!(r#"                    <a:p>
                      <a:r>
                        <a:rPr sz="{}" b="{}" i="{}">
                          <a:solidFill><a:srgbClr val="{}"/></a:solidFill>
                          <a:latin typeface="{}"/>
                        </a:rPr>
                        <a:t>{}</a:t>
                      </a:r>
                    </a:p>
"#, sz, b_val, i_val, color_hex, xml_escape(&run.font_family), esc_text));
                    }
                    if cell.text_runs.is_empty() {
                        s_xml.push_str("                    <a:p><a:endParaRPr/></a:p>\n");
                    }
                    s_xml.push_str("                  </a:txBody>\n                </a:tc>\n");
                }
                s_xml.push_str("              </a:tr>\n");
            }

            s_xml.push_str(r#"            </a:tbl>
          </a:graphicData>
        </a:graphic>
      </p:graphicFrame>
"#);
        }

        // Emit Paragraph Blocks (Text Boxes)
        for block in &page.blocks {
            match block {
                BlockIR::Paragraph(para) => {
                    let px = (para.bounds.x * PT_TO_EMU).round() as i64;
                    let py = (para.bounds.y * PT_TO_EMU).round() as i64;
                    let pcx = ((para.bounds.width * PT_TO_EMU).max(127000.0) + 127000.0).round() as i64;
                    let pcy = ((para.bounds.height * PT_TO_EMU).max(160000.0) + 60000.0).round() as i64;

                    s_xml.push_str(&format!(r#"      <p:sp>
        <p:nvSpPr>
          <p:cNvPr id="{}" name="TextBox {}"/>
          <p:cNvSpPr txBox="1"/>
          <p:nvPr/>
        </p:nvSpPr>
        <p:spPr>
          <a:xfrm><a:off x="{}" y="{}"/><a:ext cx="{}" cy="{}"/></a:xfrm>
          <a:prstGeom prst="rect"><a:avLst/></a:prstGeom>
          <a:noFill/>
        </p:spPr>
        <p:txBody>
          <a:bodyPr vert="horz" wrap="square" rtlCol="0"><a:spAutoFit/></a:bodyPr>
          <a:lstStyle/>
"#, shape_id, shape_id, px, py, pcx, pcy));
                    shape_id += 1;

                    for line in &para.lines {
                        s_xml.push_str("          <a:p>\n");
                        for run in &line.runs {
                            let sz = (run.font_size_pt * 100.0).round().max(600.0) as i32;
                            let b_val = if run.is_bold { "1" } else { "0" };
                            let i_val = if run.is_italic { "1" } else { "0" };
                            let color_hex = format!("{:02X}{:02X}{:02X}", run.color_rgb[0], run.color_rgb[1], run.color_rgb[2]);
                            let esc_text = xml_escape(&run.text);

                            s_xml.push_str(&format!(r#"            <a:r>
              <a:rPr sz="{}" b="{}" i="{}">
                <a:solidFill><a:srgbClr val="{}"/></a:solidFill>
                <a:latin typeface="{}"/>
              </a:rPr>
              <a:t>{}</a:t>
            </a:r>
"#, sz, b_val, i_val, color_hex, xml_escape(&run.font_family), esc_text));
                        }
                        s_xml.push_str("          </a:p>\n");
                    }

                    s_xml.push_str(r#"        </p:txBody>
      </p:sp>
"#);
                }
            }
        }

        s_xml.push_str(r#"    </p:spTree>
  </p:cSld>
  <p:clrMapOvr><a:masterClrMapping/></p:clrMapOvr>
</p:sld>"#);

        zip.write_all(s_xml.as_bytes()).map_err(|e| ConvertError::IoError(e.to_string()))?;
    }

    zip.finish().map_err(|e| ConvertError::IoError(e.to_string()))?;
    Ok(())
}

fn copy_template_base(
    target_zip: &mut zip::ZipWriter<std::fs::File>,
    options: zip::write::SimpleFileOptions,
    num_slides: usize,
    slide_cx: u64,
    slide_cy: u64,
) -> Result<(), ConvertError> {
    use std::io::Read;

    let mut template_archive = zip::ZipArchive::new(std::io::Cursor::new(PPTX_BLANK_TEMPLATE))
        .map_err(|e| ConvertError::ConverterError(format!("Failed to open base PPTX template: {}", e)))?;

    for i in 0..template_archive.len() {
        let mut entry = template_archive.by_index(i)
            .map_err(|e| ConvertError::IoError(e.to_string()))?;
        let name = entry.name().to_string();

        if name == "[Content_Types].xml" {
            let mut ct = String::from(r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="jpeg" ContentType="image/jpeg"/>
  <Default Extension="jpg" ContentType="image/jpeg"/>
  <Default Extension="png" ContentType="image/png"/>
  <Default Extension="gif" ContentType="image/gif"/>
  <Default Extension="bmp" ContentType="image/bmp"/>
  <Default Extension="webp" ContentType="image/webp"/>
  <Default Extension="tiff" ContentType="image/tiff"/>
  <Default Extension="tif" ContentType="image/tiff"/>
  <Default Extension="emf" ContentType="image/x-emf"/>
  <Default Extension="wmf" ContentType="image/x-wmf"/>
  <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
  <Default Extension="xml" ContentType="application/xml"/>
  <Override PartName="/ppt/presentation.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.presentation.main+xml"/>
  <Override PartName="/ppt/slideMasters/slideMaster1.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.slideMaster+xml"/>
  <Override PartName="/ppt/presProps.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.presProps+xml"/>
  <Override PartName="/ppt/viewProps.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.viewProps+xml"/>
  <Override PartName="/ppt/theme/theme1.xml" ContentType="application/vnd.openxmlformats-officedocument.theme+xml"/>
  <Override PartName="/ppt/tableStyles.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.tableStyles+xml"/>
  <Override PartName="/ppt/slideLayouts/slideLayout1.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.slideLayout+xml"/>
  <Override PartName="/ppt/slideLayouts/slideLayout2.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.slideLayout+xml"/>
  <Override PartName="/ppt/slideLayouts/slideLayout3.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.slideLayout+xml"/>
  <Override PartName="/ppt/slideLayouts/slideLayout4.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.slideLayout+xml"/>
  <Override PartName="/ppt/slideLayouts/slideLayout5.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.slideLayout+xml"/>
  <Override PartName="/ppt/slideLayouts/slideLayout6.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.slideLayout+xml"/>
  <Override PartName="/ppt/slideLayouts/slideLayout7.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.slideLayout+xml"/>
  <Override PartName="/ppt/slideLayouts/slideLayout8.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.slideLayout+xml"/>
  <Override PartName="/ppt/slideLayouts/slideLayout9.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.slideLayout+xml"/>
  <Override PartName="/ppt/slideLayouts/slideLayout10.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.slideLayout+xml"/>
  <Override PartName="/ppt/slideLayouts/slideLayout11.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.slideLayout+xml"/>
  <Override PartName="/docProps/core.xml" ContentType="application/vnd.openxmlformats-package.core-properties+xml"/>
  <Override PartName="/docProps/app.xml" ContentType="application/vnd.openxmlformats-officedocument.extended-properties+xml"/>
"#);
            for s in 1..=num_slides {
                ct.push_str(&format!(
                    "  <Override PartName=\"/ppt/slides/slide{}.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.presentationml.slide+xml\"/>\n",
                    s
                ));
            }
            ct.push_str("</Types>");
            target_zip.start_file("[Content_Types].xml", options).map_err(|e| ConvertError::IoError(e.to_string()))?;
            target_zip.write_all(ct.as_bytes()).map_err(|e| ConvertError::IoError(e.to_string()))?;
        } else if name == "ppt/_rels/presentation.xml.rels" {
            let mut pr_rels = String::from(r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideMaster" Target="slideMasters/slideMaster1.xml"/>
  <Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/presProps" Target="presProps.xml"/>
  <Relationship Id="rId3" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/viewProps" Target="viewProps.xml"/>
  <Relationship Id="rId4" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/theme" Target="theme/theme1.xml"/>
  <Relationship Id="rId5" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/tableStyles" Target="tableStyles.xml"/>
"#);
            for s in 1..=num_slides {
                pr_rels.push_str(&format!(
                    "  <Relationship Id=\"rId{}\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/slide\" Target=\"slides/slide{}.xml\"/>\n",
                    s + 5,
                    s
                ));
            }
            pr_rels.push_str("</Relationships>");
            target_zip.start_file("ppt/_rels/presentation.xml.rels", options).map_err(|e| ConvertError::IoError(e.to_string()))?;
            target_zip.write_all(pr_rels.as_bytes()).map_err(|e| ConvertError::IoError(e.to_string()))?;
        } else if name == "ppt/presentation.xml" {
            let mut p_xml = String::new();
            entry.read_to_string(&mut p_xml).map_err(|e| ConvertError::IoError(e.to_string()))?;

            if let Some(pos) = p_xml.find("<p:sldSz") {
                if let Some(end_pos) = p_xml[pos..].find("/>") {
                    let old_tag = &p_xml[pos..pos + end_pos + 2];
                    let new_tag = format!("<p:sldSz cx=\"{}\" cy=\"{}\"/>", slide_cx, slide_cy);
                    p_xml = p_xml.replace(old_tag, &new_tag);
                }
            }

            let mut sld_list = String::from("<p:sldIdLst>");
            for s in 1..=num_slides {
                sld_list.push_str(&format!(
                    "<p:sldId id=\"{}\" r:id=\"rId{}\"/>",
                    255 + s,
                    s + 5
                ));
            }
            sld_list.push_str("</p:sldIdLst>");

            if let Some(start) = p_xml.find("<p:sldIdLst>") {
                if let Some(end) = p_xml.find("</p:sldIdLst>") {
                    let old_chunk = &p_xml[start..end + "</p:sldIdLst>".len()];
                    p_xml = p_xml.replace(old_chunk, &sld_list);
                }
            } else if let Some(_pos) = p_xml.find("<p:sldMasterIdLst>") {
                if let Some(end) = p_xml.find("</p:sldMasterIdLst>") {
                    let insert_pos = end + "</p:sldMasterIdLst>".len();
                    p_xml.insert_str(insert_pos, &sld_list);
                }
            }

            target_zip.start_file("ppt/presentation.xml", options).map_err(|e| ConvertError::IoError(e.to_string()))?;
            target_zip.write_all(p_xml.as_bytes()).map_err(|e| ConvertError::IoError(e.to_string()))?;
        } else if name.starts_with("ppt/slides/") || name.starts_with("docProps/thumbnail") {
            continue;
        } else {
            let mut bytes = Vec::new();
            entry.read_to_end(&mut bytes).map_err(|e| ConvertError::IoError(e.to_string()))?;
            target_zip.start_file(name, options).map_err(|e| ConvertError::IoError(e.to_string()))?;
            target_zip.write_all(&bytes).map_err(|e| ConvertError::IoError(e.to_string()))?;
        }
    }
    Ok(())
}
