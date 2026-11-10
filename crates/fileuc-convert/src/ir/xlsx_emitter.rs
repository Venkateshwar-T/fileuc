use crate::ir::types::*;
use fileuc_types::ConvertError;
use std::io::Write;
use std::path::Path;

fn col_to_letter(mut col: usize) -> String {
    let mut s = String::new();
    loop {
        let rem = col % 26;
        s.insert(0, (b'A' + rem as u8) as char);
        if col < 26 {
            break;
        }
        col = (col / 26) - 1;
    }
    s
}

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

pub fn emit_xlsx(doc: &DocumentIR, output: &Path) -> Result<(), ConvertError> {
    if doc.pages.is_empty() {
        return Err(ConvertError::ConverterError("Document has no pages to emit".to_string()));
    }

    let file = std::fs::File::create(output).map_err(|e| ConvertError::IoError(e.to_string()))?;
    let mut zip = zip::ZipWriter::new(file);
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);

    // 1. [Content_Types].xml
    zip.start_file("[Content_Types].xml", options)
        .map_err(|e| ConvertError::IoError(e.to_string()))?;
    zip.write_all(br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
  <Default Extension="xml" ContentType="application/xml"/>
  <Override PartName="/xl/workbook.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml"/>
  <Override PartName="/xl/worksheets/sheet1.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml"/>
  <Override PartName="/xl/styles.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.styles+xml"/>
</Types>"#).map_err(|e| ConvertError::IoError(e.to_string()))?;

    // 2. _rels/.rels
    zip.start_file("_rels/.rels", options)
        .map_err(|e| ConvertError::IoError(e.to_string()))?;
    zip.write_all(br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="xl/workbook.xml"/>
</Relationships>"#).map_err(|e| ConvertError::IoError(e.to_string()))?;

    // 3. xl/_rels/workbook.xml.rels
    zip.start_file("xl/_rels/workbook.xml.rels", options)
        .map_err(|e| ConvertError::IoError(e.to_string()))?;
    zip.write_all(br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet1.xml"/>
  <Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles" Target="styles.xml"/>
</Relationships>"#).map_err(|e| ConvertError::IoError(e.to_string()))?;

    // 4. xl/styles.xml
    zip.start_file("xl/styles.xml", options)
        .map_err(|e| ConvertError::IoError(e.to_string()))?;
    zip.write_all(br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<styleSheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
  <fonts count="2">
    <font><sz val="11"/><name val="Calibri"/><family val="2"/></font>
    <font><b/><sz val="11"/><name val="Calibri"/><family val="2"/></font>
  </fonts>
  <fills count="3">
    <fill><patternFill patternType="none"/></fill>
    <fill><patternFill patternType="gray125"/></fill>
    <fill><patternFill patternType="solid"><fgColor rgb="FFE0E7FF"/></patternFill></fill>
  </fills>
  <borders count="2">
    <border><left/><right/><top/><bottom/></border>
    <border>
      <left style="thin"><color rgb="FFCBD5E1"/></left>
      <right style="thin"><color rgb="FFCBD5E1"/></right>
      <top style="thin"><color rgb="FFCBD5E1"/></top>
      <bottom style="thin"><color rgb="FFCBD5E1"/></bottom>
    </border>
  </borders>
  <cellStyleXfs count="1">
    <xf numFmtId="0" fontId="0" fillId="0" borderId="0"/>
  </cellStyleXfs>
  <cellXfs count="4">
    <xf numFmtId="0" fontId="0" fillId="0" borderId="1" xfId="0" applyBorder="1"/>
    <xf numFmtId="0" fontId="1" fillId="2" borderId="1" xfId="0" applyFont="1" applyFill="1" applyBorder="1"/>
    <xf numFmtId="1" fontId="0" fillId="0" borderId="1" xfId="0" applyNumberFormat="1" applyBorder="1"/>
    <xf numFmtId="44" fontId="0" fillId="0" borderId="1" xfId="0" applyNumberFormat="1" applyBorder="1"/>
  </cellXfs>
</styleSheet>"#).map_err(|e| ConvertError::IoError(e.to_string()))?;

    // 5. xl/workbook.xml
    zip.start_file("xl/workbook.xml", options)
        .map_err(|e| ConvertError::IoError(e.to_string()))?;
    zip.write_all(br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<workbook xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" 
          xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
  <sheets>
    <sheet name="Sheet1" sheetId="1" r:id="rId1"/>
  </sheets>
</workbook>"#).map_err(|e| ConvertError::IoError(e.to_string()))?;

    // 6. xl/worksheets/sheet1.xml
    zip.start_file("xl/worksheets/sheet1.xml", options)
        .map_err(|e| ConvertError::IoError(e.to_string()))?;

    let mut sheet_xml = String::from(r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
"#);

    // Compute column widths based on cell text lengths
    let mut col_max_chars: std::collections::HashMap<usize, usize> = std::collections::HashMap::new();
    for page in &doc.pages {
        for table in &page.tables {
            for cell in &table.cells {
                let text_len: usize = cell.text_runs.iter().map(|r| r.text.chars().count()).sum();
                let entry = col_max_chars.entry(cell.col).or_insert(10);
                *entry = (*entry).max(text_len + 4);
            }
        }
    }

    if !col_max_chars.is_empty() {
        sheet_xml.push_str("  <cols>\n");
        let mut sorted_cols: Vec<(usize, usize)> = col_max_chars.into_iter().collect();
        sorted_cols.sort_by_key(|k| k.0);
        for (col_idx, width_chars) in sorted_cols {
            let col_num = col_idx + 1;
            sheet_xml.push_str(&format!(
                "    <col min=\"{}\" max=\"{}\" width=\"{}\" customWidth=\"1\"/>\n",
                col_num, col_num, width_chars.max(12)
            ));
        }
        sheet_xml.push_str("  </cols>\n");
    }

    sheet_xml.push_str("  <sheetData>\n");

    let mut current_sheet_row = 1usize;
    let mut merge_cells: Vec<String> = Vec::new();

    for page in &doc.pages {
        // Output loose paragraphs as title/header rows if table is empty or above table
        if page.tables.is_empty() {
            for block in &page.blocks {
                let BlockIR::Paragraph(p) = block;
                let mut para_text = String::new();
                    for line in &p.lines {
                        for run in &line.runs {
                            if !para_text.is_empty() {
                                para_text.push(' ');
                            }
                            para_text.push_str(&run.text);
                        }
                    }
                    if !para_text.trim().is_empty() {
                        let cell_ref = format!("A{}", current_sheet_row);
                        sheet_xml.push_str(&format!(
                            "    <row r=\"{}\">\n      <c r=\"{}\" t=\"inlineStr\" s=\"0\"><is><t>{}</t></is></c>\n    </row>\n",
                            current_sheet_row, cell_ref, xml_escape(para_text.trim())
                        ));
                        current_sheet_row += 1;
                    }
                }
            }

        // Output Tables
        for table in &page.tables {
            let num_table_rows = table.cells.iter().map(|c| c.row).max().map(|m| m + 1).unwrap_or(0);

            for r in 0..num_table_rows {
                let row_num = current_sheet_row;
                sheet_xml.push_str(&format!("    <row r=\"{}\">\n", row_num));

                let mut row_cells: Vec<&TableCellIR> = table.cells.iter().filter(|c| c.row == r).collect();
                row_cells.sort_by_key(|c| c.col);

                for cell in row_cells {
                    let col_letter = col_to_letter(cell.col);
                    let cell_ref = format!("{}{}", col_letter, row_num);

                    if cell.row_span > 1 || cell.col_span > 1 {
                        let end_col = col_to_letter(cell.col + cell.col_span - 1);
                        let end_row = row_num + cell.row_span - 1;
                        merge_cells.push(format!("{}:{}{}", cell_ref, end_col, end_row));
                    }

                    let cell_text: String = cell.text_runs.iter().map(|r| r.text.as_str()).collect::<Vec<_>>().join(" ");
                    let trimmed = cell_text.trim();

                    let is_header = r == 0 || cell.text_runs.iter().any(|r| r.is_bold) || cell.background_rgb.is_some();
                    let style_id = if is_header { "1" } else { "0" };

                    if trimmed.is_empty() {
                        sheet_xml.push_str(&format!("      <c r=\"{}\" s=\"{}\"/>\n", cell_ref, style_id));
                    } else if let Ok(val) = trimmed.parse::<i64>() {
                        // Integer
                        sheet_xml.push_str(&format!("      <c r=\"{}\" t=\"n\" s=\"2\"><v>{}</v></c>\n", cell_ref, val));
                    } else if let Ok(val) = trimmed.parse::<f64>() {
                        // Float
                        sheet_xml.push_str(&format!("      <c r=\"{}\" t=\"n\" s=\"2\"><v>{}</v></c>\n", cell_ref, val));
                    } else if trimmed.starts_with('$') || trimmed.starts_with('€') || trimmed.starts_with('£') {
                        // Currency
                        let num_part = trimmed.trim_start_matches(['$', '€', '£', ' ']).replace(',', "");
                        if let Ok(val) = num_part.parse::<f64>() {
                            sheet_xml.push_str(&format!("      <c r=\"{}\" t=\"n\" s=\"3\"><v>{}</v></c>\n", cell_ref, val));
                        } else {
                            sheet_xml.push_str(&format!("      <c r=\"{}\" t=\"inlineStr\" s=\"{}\"><is><t>{}</t></is></c>\n", cell_ref, style_id, xml_escape(trimmed)));
                        }
                    } else {
                        // String
                        sheet_xml.push_str(&format!("      <c r=\"{}\" t=\"inlineStr\" s=\"{}\"><is><t>{}</t></is></c>\n", cell_ref, style_id, xml_escape(trimmed)));
                    }
                }

                sheet_xml.push_str("    </row>\n");
                current_sheet_row += 1;
            }

            // Gap between tables
            current_sheet_row += 1;
        }
    }

    sheet_xml.push_str("  </sheetData>\n");

    if !merge_cells.is_empty() {
        sheet_xml.push_str(&format!("  <mergeCells count=\"{}\">\n", merge_cells.len()));
        for m in merge_cells {
            sheet_xml.push_str(&format!("    <mergeCell ref=\"{}\"/>\n", m));
        }
        sheet_xml.push_str("  </mergeCells>\n");
    }

    sheet_xml.push_str("</worksheet>");

    zip.write_all(sheet_xml.as_bytes()).map_err(|e| ConvertError::IoError(e.to_string()))?;
    zip.finish().map_err(|e| ConvertError::IoError(e.to_string()))?;

    Ok(())
}
