use crate::ir::types::*;
use fileuc_types::ConvertError;
use std::io::Write;
use std::path::Path;

const PT_TO_DXA: f32 = 20.0;
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

pub fn emit_docx(doc: &DocumentIR, output: &Path) -> Result<(), ConvertError> {
    if doc.pages.is_empty() {
        return Err(ConvertError::ConverterError("Document has no pages to emit".to_string()));
    }

    let file = std::fs::File::create(output).map_err(|e| ConvertError::IoError(e.to_string()))?;
    let mut zip = zip::ZipWriter::new(file);
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);

    // 1. Collect all images across pages
    let mut media_files: Vec<(String, Vec<u8>)> = Vec::new();
    let mut image_rel_map: std::collections::HashMap<(usize, usize), usize> = std::collections::HashMap::new();

    for (page_idx, page) in doc.pages.iter().enumerate() {
        for (img_i, img) in page.images.iter().enumerate() {
            let filename = format!("image_p{}_{}.png", page_idx + 1, img_i + 1);
            let rel_id = media_files.len() + 2; // rId1 is styles.xml
            media_files.push((filename, img.data.clone()));
            image_rel_map.insert((page_idx, img_i), rel_id);
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
</Types>"#).map_err(|e| ConvertError::IoError(e.to_string()))?;

    // 3. _rels/.rels
    zip.start_file("_rels/.rels", options)
        .map_err(|e| ConvertError::IoError(e.to_string()))?;
    zip.write_all(br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/>
</Relationships>"#).map_err(|e| ConvertError::IoError(e.to_string()))?;

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
  <w:style w:type="table" w:default="1" w:styleId="TableGrid">
    <w:name w:val="Table Grid"/>
    <w:qFormat/>
    <w:tblPr>
      <w:tblBorders>
        <w:top w:val="single" w:sz="4" w:space="0" w:color="CBD5E1"/>
        <w:left w:val="single" w:sz="4" w:space="0" w:color="CBD5E1"/>
        <w:bottom w:val="single" w:sz="4" w:space="0" w:color="CBD5E1"/>
        <w:right w:val="single" w:sz="4" w:space="0" w:color="CBD5E1"/>
        <w:insideH w:val="single" w:sz="4" w:space="0" w:color="E2E8F0"/>
        <w:insideV w:val="single" w:sz="4" w:space="0" w:color="E2E8F0"/>
      </w:tblBorders>
    </w:tblPr>
  </w:style>
</w:styles>"#).map_err(|e| ConvertError::IoError(e.to_string()))?;

    // 6. Write binary media files
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

    let mut pic_id_counter = 100usize;

    for (page_idx, page) in doc.pages.iter().enumerate() {
        if page_idx > 0 {
            doc_xml.push_str("    <w:p><w:pPr><w:spacing w:before=\"0\" w:after=\"0\"/></w:pPr><w:r><w:br w:type=\"page\"/></w:r></w:p>\n");
        }

        // Emit Tables
        for table in &page.tables {
            doc_xml.push_str("    <w:tbl>\n      <w:tblPr>\n        <w:tblStyle w:val=\"TableGrid\"/>\n        <w:tblW w:w=\"0\" w:type=\"auto\"/>\n        <w:jc w:val=\"center\"/>\n      </w:tblPr>\n      <w:tblGrid>\n");

            for w_pt in &table.column_widths_pt {
                let w_dxa = (w_pt * PT_TO_DXA).round() as i64;
                doc_xml.push_str(&format!("        <w:gridCol w=\"{}\"/>\n", w_dxa));
            }
            doc_xml.push_str("      </w:tblGrid>\n");

            let num_rows = table.cells.iter().map(|c| c.row).max().map(|m| m + 1).unwrap_or(0);
            for r in 0..num_rows {
                let mut row_cells: Vec<&TableCellIR> = table.cells.iter().filter(|c| c.row == r).collect();
                row_cells.sort_by_key(|c| c.col);

                doc_xml.push_str("      <w:tr>\n");
                for cell in row_cells {
                    let w_dxa = (cell.bounds.width * PT_TO_DXA).round() as i64;
                    doc_xml.push_str(&format!("        <w:tc>\n          <w:tcPr>\n            <w:tcW w:w=\"{}\" w:type=\"dxa\"/>\n", w_dxa));

                    if let Some(rgb) = cell.background_rgb {
                        doc_xml.push_str(&format!("            <w:shd w:val=\"clear\" w:color=\"auto\" w:fill=\"{:02X}{:02X}{:02X}\"/>\n", rgb[0], rgb[1], rgb[2]));
                    }
                    doc_xml.push_str("          </w:tcPr>\n");

                    doc_xml.push_str("          <w:p>\n");
                    for run in &cell.text_runs {
                        let sz = (run.font_size_pt * 2.0).round().max(12.0) as i32;
                        let b_tag = if run.is_bold { "<w:b/><w:bCs/>" } else { "" };
                        let i_tag = if run.is_italic { "<w:i/><w:iCs/>" } else { "" };
                        let color_hex = format!("{:02X}{:02X}{:02X}", run.color_rgb[0], run.color_rgb[1], run.color_rgb[2]);
                        let esc_text = xml_escape(&run.text);
                        let esc_font = xml_escape(&run.font_family);

                        doc_xml.push_str(&format!(r#"            <w:r>
              <w:rPr>
                <w:rFonts w:ascii="{}" w:hAnsi="{}" w:cs="{}"/>
                {}{}
                <w:sz w:val="{}"/>
                <w:szCs w:val="{}"/>
                <w:color w:val="{}"/>
              </w:rPr>
              <w:t xml:space="preserve">{}</w:t>
            </w:r>
"#, esc_font, esc_font, esc_font, b_tag, i_tag, sz, sz, color_hex, esc_text));
                    }
                    doc_xml.push_str("          </w:p>\n        </w:tc>\n");
                }
                doc_xml.push_str("      </w:tr>\n");
            }
            doc_xml.push_str("    </w:tbl>\n");
        }

        // Emit Images
        for (img_i, img) in page.images.iter().enumerate() {
            if let Some(&rel_id) = image_rel_map.get(&(page_idx, img_i)) {
                pic_id_counter += 1;
                let cx = (img.bounds.width * PT_TO_EMU).round().max(12700.0) as u64;
                let cy = (img.bounds.height * PT_TO_EMU).round().max(12700.0) as u64;

                doc_xml.push_str(&format!(r#"    <w:p>
      <w:pPr><w:jc w:val="center"/><w:spacing w:before="60" w:after="60"/></w:pPr>
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
                    <a:xfrm><a:off x="0" y="0"/><a:ext cx="{}" cy="{}"/></a:xfrm>
                    <a:prstGeom prst="rect"><a:avLst/></a:prstGeom>
                  </pic:spPr>
                </pic:pic>
              </a:graphicData>
            </a:graphic>
          </wp:inline>
        </w:drawing>
      </w:r>
    </w:p>
"#, cx, cy, pic_id_counter, pic_id_counter, pic_id_counter, pic_id_counter, rel_id, cx, cy));
            }
        }

        // Emit Paragraphs
        for block in &page.blocks {
            match block {
                BlockIR::Paragraph(para) => {
                    let before_dxa = 0;
                    let after_dxa = (para.space_after_pt * PT_TO_DXA).round() as i64;
                    let line_spacing_dxa = (para.line_spacing_pt * PT_TO_DXA).round() as i64;

                    let jc_val = match para.alignment {
                        TextAlignment::Left => "left",
                        TextAlignment::Center => "center",
                        TextAlignment::Right => "right",
                        TextAlignment::Justified => "both",
                    };

                    doc_xml.push_str(&format!(r#"    <w:p>
      <w:pPr>
        <w:jc w:val="{}"/>
        <w:spacing w:before="{}" w:after="{}" w:line="{}" w:lineRule="auto"/>
      </w:pPr>
"#, jc_val, before_dxa, after_dxa, line_spacing_dxa));

                    for line in &para.lines {
                        for run in &line.runs {
                            let sz = (run.font_size_pt * 2.0).round().max(12.0) as i32;
                            let b_tag = if run.is_bold { "<w:b/><w:bCs/>" } else { "" };
                            let i_tag = if run.is_italic { "<w:i/><w:iCs/>" } else { "" };
                            let color_hex = format!("{:02X}{:02X}{:02X}", run.color_rgb[0], run.color_rgb[1], run.color_rgb[2]);
                            let esc_text = xml_escape(&run.text);
                            let esc_font = xml_escape(&run.font_family);

                            doc_xml.push_str(&format!(r#"      <w:r>
        <w:rPr>
          <w:rFonts w:ascii="{}" w:hAnsi="{}" w:cs="{}"/>
          {}{}
          <w:sz w:val="{}"/>
          <w:szCs w:val="{}"/>
          <w:color w:val="{}"/>
        </w:rPr>
        <w:t xml:space="preserve">{}</w:t>
      </w:r>
"#, esc_font, esc_font, esc_font, b_tag, i_tag, sz, sz, color_hex, esc_text));
                        }
                    }
                    doc_xml.push_str("    </w:p>\n");
                }
            }
        }
    }

    let first_page = &doc.pages[0];
    let page_w_dxa = (first_page.width_pt * PT_TO_DXA).round() as u64;
    let page_h_dxa = (first_page.height_pt * PT_TO_DXA).round() as u64;
    let orient_attr = if first_page.orientation == PageOrientation::Landscape { " w:orient=\"landscape\"" } else { "" };

    doc_xml.push_str(&format!(r#"    <w:sectPr>
      <w:pgSz w:w="{}" w:h="{}"{}/>
      <w:pgMar w:top="1440" w:right="1440" w:bottom="1440" w:left="1440" w:header="720" w:footer="720" w:gutter="0"/>
    </w:sectPr>
  </w:body>
</w:document>"#, page_w_dxa, page_h_dxa, orient_attr));

    zip.write_all(doc_xml.as_bytes()).map_err(|e| ConvertError::IoError(e.to_string()))?;
    zip.finish().map_err(|e| ConvertError::IoError(e.to_string()))?;

    Ok(())
}
