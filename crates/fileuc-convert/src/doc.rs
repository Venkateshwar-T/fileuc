use fileuc_types::ConvertError;
use pdf_writer::{Content, Finish, Name, Pdf, Rect, Ref, Str};
use std::io::Read;
use std::path::Path;

pub fn doc_to_pdf(src: &Path, output: &Path) -> Result<(), ConvertError> {
    if !src.exists() {
        return Err(ConvertError::SourceNotFound(src.to_string_lossy().to_string()));
    }

    // Try Word COM first if available on Windows
    if try_word_com_convert(src, output).is_ok() && output.exists() {
        if let Ok(meta) = std::fs::metadata(output) {
            if meta.len() > 0 {
                return Ok(());
            }
        }
    }

    // Pure Rust fallback: extract text and render via pdf-writer
    let paragraphs = extract_doc_paragraphs(src)?;
    render_text_to_pdf(&paragraphs, output)
}

fn try_word_com_convert(src: &Path, output: &Path) -> Result<(), ConvertError> {
    let abs_src = crate::pptx::clean_windows_path(src);
    let abs_dst = crate::pptx::clean_windows_path(output);

    let script = format!(
        "$word = New-Object -ComObject Word.Application; $word.Visible = $false; $word.DisplayAlerts = 0; try {{ $doc = $word.Documents.Open('{}', $false, $true); if ($doc -ne $null) {{ $doc.SaveAs([ref]'{}', [ref]17); $doc.Close([ref]0); [System.Runtime.InteropServices.Marshal]::ReleaseComObject($doc) | Out-Null; }} }} finally {{ $word.Quit(); [System.Runtime.InteropServices.Marshal]::ReleaseComObject($word) | Out-Null; [System.GC]::Collect(); [System.GC]::WaitForPendingFinalizers(); }}",
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
    Err(ConvertError::ConverterError("Word COM conversion failed".to_string()))
}

fn extract_doc_paragraphs(src: &Path) -> Result<Vec<String>, ConvertError> {
    let ext = src
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();

    if ext == "docx" {
        extract_docx_paragraphs(src)
    } else {
        extract_rtf_or_binary_paragraphs(src)
    }
}

fn extract_docx_paragraphs(src: &Path) -> Result<Vec<String>, ConvertError> {
    let file = std::fs::File::open(src).map_err(|e| ConvertError::IoError(e.to_string()))?;
    let mut archive = zip::ZipArchive::new(file)
        .map_err(|e| ConvertError::ConverterError(format!("Failed to read DOCX zip: {}", e)))?;

    let mut doc_xml = String::new();
    if let Ok(mut xml_file) = archive.by_name("word/document.xml") {
        xml_file
            .read_to_string(&mut doc_xml)
            .map_err(|e| ConvertError::IoError(e.to_string()))?;
    } else {
        return Err(ConvertError::ConverterError("Invalid DOCX: missing document.xml".to_string()));
    }

    let mut paragraphs = Vec::new();
    for p_chunk in doc_xml.split("<w:p") {
        if let Some(end_p) = p_chunk.find("</w:p>") {
            let p_content = &p_chunk[..end_p];
            let mut line = String::new();
            for t_chunk in p_content.split("<w:t") {
                if let Some(start_t) = t_chunk.find('>') {
                    if let Some(end_t) = t_chunk[start_t..].find("</w:t>") {
                        let text = &t_chunk[start_t + 1..start_t + end_t];
                        line.push_str(text);
                    }
                }
            }
            let trimmed = line.trim();
            if !trimmed.is_empty() {
                paragraphs.push(trimmed.to_string());
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

fn extract_rtf_or_binary_paragraphs(src: &Path) -> Result<Vec<String>, ConvertError> {
    let bytes = std::fs::read(src).map_err(|e| ConvertError::IoError(e.to_string()))?;
    let text = String::from_utf8_lossy(&bytes);

    if text.starts_with(r"{\rtf") {
        let mut paragraphs = Vec::new();
        let mut current_para = String::new();
        let mut chars = text.chars().peekable();
        while let Some(c) = chars.next() {
            if c == '\\' {
                let mut cmd = String::new();
                while let Some(&next) = chars.peek() {
                    if next.is_ascii_alphabetic() || next == '\'' {
                        cmd.push(chars.next().unwrap());
                    } else {
                        break;
                    }
                }
                if cmd == "par" {
                    let trimmed = current_para.trim();
                    if !trimmed.is_empty() {
                        paragraphs.push(trimmed.to_string());
                    }
                    current_para.clear();
                } else if cmd == "'20" {
                    current_para.push(' ');
                }
                if chars.peek() == Some(&' ') {
                    chars.next();
                }
            } else if c != '{' && c != '}' && c != '\r' && c != '\n' {
                current_para.push(c);
            }
        }
        let trimmed = current_para.trim();
        if !trimmed.is_empty() {
            paragraphs.push(trimmed.to_string());
        }
        if !paragraphs.is_empty() {
            return Ok(paragraphs);
        }
    }

    let mut paragraphs = Vec::new();
    let mut current = String::new();
    for &b in &bytes {
        if b.is_ascii_graphic() || b == b' ' {
            current.push(b as char);
        } else if b == b'\n' || b == b'\r' {
            if current.len() >= 3 {
                paragraphs.push(current.trim().to_string());
            }
            current.clear();
        }
    }
    if current.len() >= 3 {
        paragraphs.push(current.trim().to_string());
    }

    if paragraphs.is_empty() {
        paragraphs.push(format!(
            "Document: {}",
            src.file_name().unwrap_or_default().to_string_lossy()
        ));
    }

    Ok(paragraphs)
}

fn render_text_to_pdf(paragraphs: &[String], output: &Path) -> Result<(), ConvertError> {
    let mut pdf = Pdf::new();
    let catalog_ref = Ref::new(1);
    let page_tree_ref = Ref::new(2);
    let font_ref = Ref::new(3);
    let font_name = Name(b"F1");

    pdf.catalog(catalog_ref).pages(page_tree_ref);
    pdf.type1_font(font_ref).base_font(Name(b"Helvetica"));

    let page_w = 595.0; // A4 width
    let page_h = 842.0; // A4 height
    let margin = 50.0;
    let line_height = 16.0;
    let font_size = 11.0;
    let max_lines_per_page = ((page_h - 2.0 * margin) / line_height) as usize;

    let mut wrapped_lines = Vec::new();
    for p in paragraphs {
        let words: Vec<&str> = p.split_whitespace().collect();
        if words.is_empty() {
            continue;
        }
        let mut cur_line = String::new();
        for w in words {
            if cur_line.is_empty() {
                if w.len() > 75 {
                    let mut start = 0;
                    while start < w.len() {
                        let end = (start + 75).min(w.len());
                        wrapped_lines.push(w[start..end].to_string());
                        start = end;
                    }
                } else {
                    cur_line = w.to_string();
                }
            } else if cur_line.len() + w.len() + 1 > 75 {
                wrapped_lines.push(cur_line);
                if w.len() > 75 {
                    let mut start = 0;
                    while start < w.len() {
                        let end = (start + 75).min(w.len());
                        wrapped_lines.push(w[start..end].to_string());
                        start = end;
                    }
                    cur_line = String::new();
                } else {
                    cur_line = w.to_string();
                }
            } else {
                cur_line.push(' ');
                cur_line.push_str(w);
            }
        }
        if !cur_line.is_empty() {
            wrapped_lines.push(cur_line);
        }
        wrapped_lines.push(String::new());
    }

    let chunks: Vec<&[String]> = wrapped_lines.chunks(max_lines_per_page.max(1)).collect();
    let page_count = chunks.len().max(1);

    let mut page_refs = Vec::new();
    let mut next_ref = 4;

    for _ in 0..page_count {
        page_refs.push(Ref::new(next_ref));
        next_ref += 1;
    }

    pdf.pages(page_tree_ref)
        .kids(page_refs.iter().copied())
        .count(page_refs.len() as i32);

    for (idx, page_ref) in page_refs.iter().enumerate() {
        let content_ref = Ref::new(next_ref);
        next_ref += 1;

        let mut page = pdf.page(*page_ref);
        page.media_box(Rect::new(0.0, 0.0, page_w, page_h));
        page.parent(page_tree_ref);
        page.contents(content_ref);
        page.resources().fonts().pair(font_name, font_ref);
        page.finish();

        let mut content = Content::new();
        content.begin_text();
        content.set_font(font_name, font_size);
        content.next_line(margin, page_h - margin);

        if let Some(lines) = chunks.get(idx) {
            for line in lines.iter() {
                if line.is_empty() {
                    content.next_line(0.0, -line_height * 0.7);
                } else {
                    let sanitized: String = line
                        .chars()
                        .map(|c| if (c as u32) >= 0x20 && (c as u32) <= 0x7E { c } else if c == '\t' { ' ' } else { '?' })
                        .collect();
                    content.show(Str(sanitized.as_bytes()));
                    content.next_line(0.0, -line_height);
                }
            }
        }

        content.end_text();
        pdf.stream(content_ref, &content.finish());
    }

    std::fs::write(output, pdf.finish()).map_err(|e| ConvertError::IoError(e.to_string()))?;
    Ok(())
}
