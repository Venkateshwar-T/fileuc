use fileuc_types::ConvertError;
use pdf_writer::{Content, Filter, Finish, Name, Pdf, Rect, Ref, Str};
use std::io::Read;
use std::path::Path;

pub fn excel_to_pdf(src: &Path, output: &Path) -> Result<(), ConvertError> {
    if !src.exists() {
        return Err(ConvertError::SourceNotFound(src.to_string_lossy().to_string()));
    }

    // 1. Try Excel COM automation first if available on Windows
    if try_excel_com_convert(src, output).is_ok() && output.exists() {
        if let Ok(meta) = std::fs::metadata(output) {
            if meta.len() > 0 {
                return Ok(());
            }
        }
    }

    // 2. Pure Rust smart fallback: parse sheet data and render table with column splitting
    let table = extract_spreadsheet_table(src)?;
    render_table_to_pdf(&table, output)
}

fn try_excel_com_convert(src: &Path, output: &Path) -> Result<(), ConvertError> {
    let abs_src = crate::pptx::clean_windows_path(src);
    let abs_dst = crate::pptx::clean_windows_path(output);

    // xlTypePDF = 0
    let script = format!(
        "$excel = New-Object -ComObject Excel.Application; $excel.Visible = $false; $excel.DisplayAlerts = $false; \
try {{ \
  $wb = $excel.Workbooks.Open('{}', $false, $true); \
  if ($wb -ne $null) {{ \
    $wb.ExportAsFixedFormat(0, '{}'); \
    $wb.Close($false); \
    [System.Runtime.InteropServices.Marshal]::ReleaseComObject($wb) | Out-Null; \
  }} \
}} finally {{ \
  $excel.Quit(); \
  [System.Runtime.InteropServices.Marshal]::ReleaseComObject($excel) | Out-Null; \
  [System.GC]::Collect(); \
  [System.GC]::WaitForPendingFinalizers(); \
}}",
        abs_src.replace('\'', "''"),
        abs_dst.replace('\'', "''")
    );

    let mut cmd = std::process::Command::new("powershell.exe");
    cmd.args(["-NoProfile", "-NonInteractive", "-WindowStyle", "Hidden", "-Command", &script]);
    cmd.stdout(std::process::Stdio::null());
    cmd.stderr(std::process::Stdio::null());

    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }

    let status = cmd.status().map_err(|e| ConvertError::ConverterError(e.to_string()))?;
    if status.success() && output.exists() {
        if let Ok(meta) = std::fs::metadata(output) {
            if meta.len() > 0 {
                return Ok(());
            }
        }
    }
    if output.exists() {
        let _ = std::fs::remove_file(output);
    }
    Err(ConvertError::ConverterError("Excel COM conversion failed".to_string()))
}

pub fn extract_spreadsheet_table(src: &Path) -> Result<Vec<Vec<String>>, ConvertError> {
    let ext = src
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();

    match ext.as_str() {
        "xlsx" => extract_xlsx_table(src),
        "csv" => extract_csv_table(src, b','),
        "tsv" => extract_csv_table(src, b'\t'),
        _ => {
            // Try xlsx first, then csv fallback
            if let Ok(table) = extract_xlsx_table(src) {
                if !table.is_empty() {
                    return Ok(table);
                }
            }
            extract_csv_table(src, b',')
        }
    }
}

fn extract_csv_table(src: &Path, delimiter: u8) -> Result<Vec<Vec<String>>, ConvertError> {
    let content = match std::fs::read_to_string(src) {
        Ok(s) => s,
        Err(_) => {
            let bytes = std::fs::read(src).map_err(|e| ConvertError::IoError(e.to_string()))?;
            String::from_utf8_lossy(&bytes).to_string()
        }
    };

    let mut rows = Vec::new();
    let delim_char = delimiter as char;

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        let mut row = Vec::new();
        let mut cur_field = String::new();
        let mut in_quotes = false;
        let mut chars = trimmed.chars().peekable();

        while let Some(ch) = chars.next() {
            if ch == '"' {
                if in_quotes && chars.peek() == Some(&'"') {
                    cur_field.push('"');
                    chars.next();
                } else {
                    in_quotes = !in_quotes;
                }
            } else if ch == delim_char && !in_quotes {
                row.push(cur_field.trim().to_string());
                cur_field.clear();
            } else {
                cur_field.push(ch);
            }
        }
        row.push(cur_field.trim().to_string());
        rows.push(row);
    }

    Ok(rows)
}

fn extract_xlsx_table(src: &Path) -> Result<Vec<Vec<String>>, ConvertError> {
    let file = std::fs::File::open(src).map_err(|e| ConvertError::IoError(e.to_string()))?;
    let mut zip = zip::ZipArchive::new(file).map_err(|e| ConvertError::ConverterError(e.to_string()))?;

    // 1. Parse shared strings table if present
    let mut shared_strings: Vec<String> = Vec::new();
    if let Ok(mut ss_file) = zip.by_name("xl/sharedStrings.xml") {
        let mut xml = String::new();
        if ss_file.read_to_string(&mut xml).is_ok() {
            shared_strings = parse_shared_strings(&xml);
        }
    }

    // 2. Locate first sheet
    let mut sheet_xml = String::new();
    let sheet_names = ["xl/worksheets/sheet1.xml", "xl/worksheets/Sheet1.xml", "xl/worksheets/sheet.xml"];
    for name in &sheet_names {
        if let Ok(mut sf) = zip.by_name(name) {
            let _ = sf.read_to_string(&mut sheet_xml);
            break;
        }
    }

    if sheet_xml.is_empty() {
        // Find any file in xl/worksheets/
        for i in 0..zip.len() {
            if let Ok(mut file) = zip.by_index(i) {
                let name = file.name().to_string();
                if name.starts_with("xl/worksheets/sheet") && name.ends_with(".xml") {
                    let _ = file.read_to_string(&mut sheet_xml);
                    break;
                }
            }
        }
    }

    if sheet_xml.is_empty() {
        return Err(ConvertError::ConverterError("No sheet found in XLSX".to_string()));
    }

    parse_sheet_xml(&sheet_xml, &shared_strings)
}

fn parse_shared_strings(xml: &str) -> Vec<String> {
    let mut strings = Vec::new();
    let mut pos = 0;

    while let Some(si_start) = xml[pos..].find("<si>") {
        let abs_start = pos + si_start + 4;
        let abs_end = match xml[abs_start..].find("</si>") {
            Some(e) => abs_start + e,
            None => break,
        };

        let si_content = &xml[abs_start..abs_end];
        let mut text = String::new();
        let mut t_pos = 0;

        while let Some(t_open) = si_content[t_pos..].find("<t") {
            let tag_content_start = t_pos + t_open;
            let tag_close = match si_content[tag_content_start..].find('>') {
                Some(c) => tag_content_start + c + 1,
                None => break,
            };
            let t_end = match si_content[tag_close..].find("</t>") {
                Some(e) => tag_close + e,
                None => break,
            };

            text.push_str(&si_content[tag_close..t_end]);
            t_pos = t_end + 4;
        }

        strings.push(xml_unescape(&text));
        pos = abs_end + 5;
    }

    strings
}

fn parse_sheet_xml(xml: &str, shared_strings: &[String]) -> Result<Vec<Vec<String>>, ConvertError> {
    let mut rows: Vec<Vec<String>> = Vec::new();
    let mut row_pos = 0;

    while let Some(r_start) = xml[row_pos..].find("<row ") {
        let abs_r_start = row_pos + r_start;
        let abs_r_end = match xml[abs_r_start..].find("</row>") {
            Some(e) => abs_r_start + e,
            None => break,
        };

        let row_content = &xml[abs_r_start..abs_r_end];
        let mut row_cells: Vec<(usize, String)> = Vec::new();

        // Parse cells: <c r="A1" t="s"><v>0</v></c>
        let mut c_pos = 0;
        while let Some(c_start) = row_content[c_pos..].find("<c ") {
            let cell_sub = &row_content[c_pos + c_start..];
            let tag_end = match cell_sub.find('>') {
                Some(i) => i,
                None => break,
            };
            let tag_attrs = &cell_sub[..tag_end];

            // Extract cell reference r="A1"
            let col_idx = if let Some(r_idx) = tag_attrs.find("r=\"") {
                let rest = &tag_attrs[r_idx + 3..];
                let ref_str = rest.split('"').next().unwrap_or("");
                column_ref_to_index(ref_str)
            } else {
                row_cells.len()
            };

            let is_shared_string = tag_attrs.contains("t=\"s\"");
            let is_inline_string = tag_attrs.contains("t=\"inlineStr\"");

            // Extract cell value
            let cell_full_end = match cell_sub.find("</c>") {
                Some(i) => i + 4,
                None => tag_end + 1,
            };
            let cell_body = &cell_sub[tag_end + 1..cell_full_end];

            let cell_val = if is_shared_string {
                if let Some(v_start) = cell_body.find("<v>") {
                    let v_end = cell_body[v_start + 3..].find("</v>").unwrap_or(0);
                    let idx_str = &cell_body[v_start + 3..v_start + 3 + v_end];
                    if let Ok(idx) = idx_str.parse::<usize>() {
                        shared_strings.get(idx).cloned().unwrap_or_default()
                    } else {
                        String::new()
                    }
                } else {
                    String::new()
                }
            } else if is_inline_string {
                if let Some(t_start) = cell_body.find("<t>") {
                    let t_end = cell_body[t_start + 3..].find("</t>").unwrap_or(0);
                    xml_unescape(&cell_body[t_start + 3..t_start + 3 + t_end])
                } else {
                    String::new()
                }
            } else if let Some(v_start) = cell_body.find("<v>") {
                let v_end = cell_body[v_start + 3..].find("</v>").unwrap_or(0);
                xml_unescape(&cell_body[v_start + 3..v_start + 3 + v_end])
            } else {
                String::new()
            };

            row_cells.push((col_idx, cell_val));
            c_pos += c_start + cell_full_end;
        }

        if !row_cells.is_empty() {
            let max_col = row_cells.iter().map(|(idx, _)| *idx).max().unwrap_or(0);
            let mut row = vec![String::new(); max_col + 1];
            for (idx, val) in row_cells {
                if idx < row.len() {
                    row[idx] = val;
                }
            }
            rows.push(row);
        }

        row_pos = abs_r_end + 6;
    }

    Ok(rows)
}

fn column_ref_to_index(cell_ref: &str) -> usize {
    let mut col = 0usize;
    for ch in cell_ref.chars() {
        if ch.is_ascii_alphabetic() {
            let val = (ch.to_ascii_uppercase() as u8 - b'A' + 1) as usize;
            col = col * 26 + val;
        } else {
            break;
        }
    }
    if col > 0 {
        col - 1
    } else {
        0
    }
}

fn xml_unescape(s: &str) -> String {
    s.replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
}

pub fn render_table_to_pdf(table: &[Vec<String>], output: &Path) -> Result<(), ConvertError> {
    if table.is_empty() {
        return render_empty_pdf(output);
    }

    let col_count = table.iter().map(|r| r.len()).max().unwrap_or(0);
    if col_count == 0 {
        return render_empty_pdf(output);
    }

    // A4 Landscape dimensions (points)
    let page_w = 841.89;
    let page_h = 595.28;
    let margin = 36.0;
    let usable_w = page_w - (2.0 * margin); // 769.89 pt
    let usable_h = page_h - (2.0 * margin); // 523.28 pt

    let header_h = 24.0;
    let row_h = 20.0;
    let font_size = 9.0;
    let header_font_size = 9.5;

    // 1. Calculate natural column widths based on content length
    let mut natural_col_widths = vec![50.0f32; col_count];
    for col in 0..col_count {
        let mut max_chars = 3;
        for row in table {
            if let Some(cell) = row.get(col) {
                max_chars = max_chars.max(cell.chars().count());
            }
        }
        // Approximate width: 6.2 pt per character + 14 pt padding
        natural_col_widths[col] = (max_chars as f32 * 6.2 + 14.0).clamp(50.0, 240.0);
    }

    // 2. Partition columns into horizontal page bands (smart column splitting!)
    let mut col_bands: Vec<Vec<usize>> = Vec::new();
    let mut current_band: Vec<usize> = Vec::new();
    let mut current_band_w = 0.0;

    for col in 0..col_count {
        let w = natural_col_widths[col];
        if !current_band.is_empty() && current_band_w + w > usable_w {
            col_bands.push(current_band);
            current_band = Vec::new();
            current_band_w = 0.0;
        }
        current_band.push(col);
        current_band_w += w;
    }
    if !current_band.is_empty() {
        col_bands.push(current_band);
    }

    // 3. Vertical chunking of rows
    let max_data_rows_per_page = ((usable_h - header_h - 25.0) / row_h).floor() as usize;
    let max_data_rows_per_page = max_data_rows_per_page.max(5);

    let header_row = &table[0];
    let data_rows = if table.len() > 1 { &table[1..] } else { &[][..] };
    let row_chunks: Vec<&[Vec<String>]> = if data_rows.is_empty() {
        vec![&[][..]]
    } else {
        data_rows.chunks(max_data_rows_per_page).collect()
    };

    // Total pages = vertical chunks * horizontal bands
    let total_pages = row_chunks.len() * col_bands.len();

    // 4. Build PDF with pdf-writer
    let mut pdf = Pdf::new();
    let catalog_ref = Ref::new(1);
    let page_tree_ref = Ref::new(2);
    let font_ref = Ref::new(3);
    let bold_font_ref = Ref::new(4);
    let font_name = Name(b"F1");
    let bold_font_name = Name(b"F2");

    pdf.catalog(catalog_ref).pages(page_tree_ref);
    pdf.type1_font(font_ref).base_font(Name(b"Helvetica"));
    pdf.type1_font(bold_font_ref).base_font(Name(b"Helvetica-Bold"));

    let mut page_refs = Vec::new();
    let mut next_ref = 5;

    for (chunk_idx, row_chunk) in row_chunks.iter().enumerate() {
        for (band_idx, col_band) in col_bands.iter().enumerate() {
            let page_ref = Ref::new(next_ref);
            next_ref += 1;
            let content_ref = Ref::new(next_ref);
            next_ref += 1;
            page_refs.push(page_ref);

            // Compute scaled widths for columns in this band to fit usable_w cleanly
            let band_natural_w: f32 = col_band.iter().map(|&c| natural_col_widths[c]).sum();
            let scale_factor = if band_natural_w < usable_w {
                (usable_w / band_natural_w).min(1.4)
            } else {
                usable_w / band_natural_w
            };

            let scaled_col_widths: Vec<f32> = col_band
                .iter()
                .map(|&c| natural_col_widths[c] * scale_factor)
                .collect();
            let total_table_w: f32 = scaled_col_widths.iter().sum();

            let mut content = Content::new();

            // Top banner info: Sheet Title & Horizontal Part Indicator
            let top_y = page_h - margin;
            content.begin_text();
            content.set_font(bold_font_name, 11.0);
            content.next_line(margin, top_y - 12.0);
            if col_bands.len() > 1 {
                let col_start = col_band.first().copied().unwrap_or(0) + 1;
                let col_end = col_band.last().copied().unwrap_or(0) + 1;
                let title = format!("Spreadsheet Data (Columns {} - {})", col_start, col_end);
                content.show(Str(title.as_bytes()));
            } else {
                content.show(Str(b"Spreadsheet Data"));
            }
            content.end_text();

            // Table starting Y
            let table_top_y = top_y - 20.0;

            // 1. Draw Table Header Background (#EEF2F6)
            content.save_state();
            content.set_fill_gray(0.93);
            content.rect(margin, table_top_y - header_h, total_table_w, header_h);
            content.fill_nonzero();
            content.restore_state();

            // 2. Draw Table Header Borders & Text
            content.save_state();
            content.set_stroke_gray(0.70);
            content.set_line_width(0.75);
            content.rect(margin, table_top_y - header_h, total_table_w, header_h);
            content.stroke();

            let mut cur_x = margin;
            for (i, &col_idx) in col_band.iter().enumerate() {
                let col_w = scaled_col_widths[i];
                if i > 0 {
                    content.move_to(cur_x, table_top_y);
                    content.line_to(cur_x, table_top_y - header_h);
                    content.stroke();
                }

                let text = header_row.get(col_idx).map(|s| s.as_str()).unwrap_or("");
                let display_text = truncate_text_for_width(text, col_w - 8.0, header_font_size);

                content.begin_text();
                content.set_font(bold_font_name, header_font_size);
                content.next_line(cur_x + 5.0, table_top_y - header_h + 7.0);
                content.show(Str(display_text.as_bytes()));
                content.end_text();

                cur_x += col_w;
            }
            content.restore_state();

            // 3. Draw Data Rows
            let mut cur_y = table_top_y - header_h;
            for (row_idx, row) in row_chunk.iter().enumerate() {
                let row_bottom = cur_y - row_h;

                // Alternating row background (#F8FAFC on odd rows)
                if row_idx % 2 == 1 {
                    content.save_state();
                    content.set_fill_gray(0.975);
                    content.rect(margin, row_bottom, total_table_w, row_h);
                    content.fill_nonzero();
                    content.restore_state();
                }

                // Row borders
                content.save_state();
                content.set_stroke_gray(0.85);
                content.set_line_width(0.5);
                content.rect(margin, row_bottom, total_table_w, row_h);
                content.stroke();

                let mut cell_x = margin;
                for (i, &col_idx) in col_band.iter().enumerate() {
                    let col_w = scaled_col_widths[i];
                    if i > 0 {
                        content.move_to(cell_x, cur_y);
                        content.line_to(cell_x, row_bottom);
                        content.stroke();
                    }

                    let cell_text = row.get(col_idx).map(|s| s.as_str()).unwrap_or("");
                    let display_text = truncate_text_for_width(cell_text, col_w - 8.0, font_size);

                    content.begin_text();
                    content.set_font(font_name, font_size);
                    content.next_line(cell_x + 5.0, row_bottom + 6.0);
                    content.show(Str(display_text.as_bytes()));
                    content.end_text();

                    cell_x += col_w;
                }
                content.restore_state();

                cur_y = row_bottom;
            }

            // Footer: Page Number
            let current_page_num = chunk_idx * col_bands.len() + band_idx + 1;
            let footer_text = format!("Page {} of {}", current_page_num, total_pages);
            content.begin_text();
            content.set_font(font_name, 8.5);
            content.next_line(page_w - margin - 70.0, margin - 15.0);
            content.show(Str(footer_text.as_bytes()));
            content.end_text();

            let content_bytes = content.finish();
            let mut stream = pdf.stream(content_ref, &content_bytes);
            stream.filter(Filter::FlateDecode);
            stream.finish();

            let mut page = pdf.page(page_ref);
            page.media_box(Rect::new(0.0, 0.0, page_w, page_h));
            page.parent(page_tree_ref);
            page.contents(content_ref);

            let mut resources = page.resources();
            let mut fonts = resources.fonts();
            fonts.pair(font_name, font_ref);
            fonts.pair(bold_font_name, bold_font_ref);
            fonts.finish();
            resources.finish();
            page.finish();
        }
    }

    let mut pages = pdf.pages(page_tree_ref);
    pages.count(page_refs.len() as i32);
    pages.kids(page_refs.iter().copied());
    pages.finish();

    let pdf_bytes = pdf.finish();
    std::fs::write(output, &pdf_bytes).map_err(|e| ConvertError::IoError(e.to_string()))?;
    Ok(())
}

fn truncate_text_for_width(text: &str, max_width_pt: f32, font_size: f32) -> String {
    // Helvetica character width average is ~0.55 * font_size
    let char_w = font_size * 0.52;
    let max_chars = (max_width_pt / char_w).floor() as usize;

    // Filter out non-ASCII or unprintable control characters for standard Type1 Helvetica
    let cleaned: String = text
        .chars()
        .map(|c| if c.is_ascii() && !c.is_ascii_control() { c } else { ' ' })
        .collect();

    let trimmed = cleaned.trim();
    if trimmed.chars().count() <= max_chars {
        trimmed.to_string()
    } else if max_chars > 3 {
        let truncated: String = trimmed.chars().take(max_chars - 2).collect();
        format!("{}..", truncated)
    } else {
        trimmed.chars().take(max_chars).collect()
    }
}

fn render_empty_pdf(output: &Path) -> Result<(), ConvertError> {
    let mut pdf = Pdf::new();
    let catalog_ref = Ref::new(1);
    let page_tree_ref = Ref::new(2);
    let page_ref = Ref::new(3);
    let font_ref = Ref::new(4);
    let content_ref = Ref::new(5);

    pdf.catalog(catalog_ref).pages(page_tree_ref);
    pdf.type1_font(font_ref).base_font(Name(b"Helvetica"));

    let mut content = Content::new();
    content.begin_text();
    content.set_font(Name(b"F1"), 12.0);
    content.next_line(72.0, 500.0);
    content.show(Str(b"Empty Spreadsheet"));
    content.end_text();

    let bytes = content.finish();
    let stream = pdf.stream(content_ref, &bytes);
    stream.finish();

    let mut page = pdf.page(page_ref);
    page.media_box(Rect::new(0.0, 0.0, 595.0, 842.0));
    page.parent(page_tree_ref);
    page.contents(content_ref);
    let mut resources = page.resources();
    resources.fonts().pair(Name(b"F1"), font_ref);
    resources.finish();
    page.finish();

    let mut pages = pdf.pages(page_tree_ref);
    pages.count(1);
    pages.kids([page_ref]);
    pages.finish();

    let pdf_bytes = pdf.finish();
    std::fs::write(output, &pdf_bytes).map_err(|e| ConvertError::IoError(e.to_string()))?;
    Ok(())
}
