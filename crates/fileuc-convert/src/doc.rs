use fileuc_types::ConvertError;
use pdf_writer::{Content, Finish, Name, Pdf, Rect, Ref, Str};
use std::io::Read;
use std::path::{Path, PathBuf};

pub fn find_pdf2docx() -> Option<PathBuf> {
    // 1. Check %LOCALAPPDATA%\FileUC\plugins\pdf2docx\Scripts\python.exe
    if let Ok(local_app_data) = std::env::var("LOCALAPPDATA") {
        let p = PathBuf::from(local_app_data)
            .join("FileUC")
            .join("plugins")
            .join("pdf2docx")
            .join("Scripts")
            .join("python.exe");
        if p.exists() {
            return Some(p);
        }
    }

    // 2. Check next to executable: <exe_dir>\plugins\pdf2docx\Scripts\python.exe
    if let Ok(exe) = std::env::current_exe() {
        if let Some(parent) = exe.parent() {
            let next_to_exe = parent
                .join("plugins")
                .join("pdf2docx")
                .join("Scripts")
                .join("python.exe");
            if next_to_exe.exists() {
                return Some(next_to_exe);
            }
            let engines_p = parent
                .join("engines")
                .join("pdf2docx")
                .join("python.exe");
            if engines_p.exists() {
                return Some(engines_p);
            }
        }
    }

    // 3. Check system / uv python environment if pdf2docx is installed on disk
    if let Ok(user_profile) = std::env::var("USERPROFILE") {
        let uv_dir = PathBuf::from(&user_profile)
            .join("AppData")
            .join("Roaming")
            .join("uv")
            .join("python")
            .join("cpython-3.13-windows-x86_64-none");
        let uv_python = uv_dir.join("python.exe");
        let site_pkg = uv_dir.join("Lib").join("site-packages").join("pdf2docx");
        if uv_python.exists() && site_pkg.exists() {
            return Some(uv_python);
        }
    }

    None
}

pub fn pdf_to_docx(src: &Path, output: &Path) -> Result<(), ConvertError> {
    if !src.exists() {
        return Err(ConvertError::SourceNotFound(src.to_string_lossy().to_string()));
    }
    let guard = crate::output_guard::OutputGuard::new(output);

    // 1. Try pdf2docx plugin engine
    if let Some(python_exe) = find_pdf2docx() {
        if run_pdf2docx_converter(&python_exe, src, output).is_ok() && output.exists() {
            if let Ok(meta) = std::fs::metadata(output) {
                if meta.len() > 0 {
                    guard.disarm();
                    return Ok(());
                }
            }
        }
        return Err(ConvertError::ConverterError(
            "PDF to Word conversion failed.".to_string(),
        ));
    }

    Err(ConvertError::EngineMissing(
        "PDF to Word engine is not installed. Please open FileUC Settings -> Plugins to install it.".to_string(),
    ))
}

fn run_pdf2docx_converter(python_exe: &Path, src: &Path, output: &Path) -> Result<(), ConvertError> {
    let mut cmd = std::process::Command::new(python_exe);
    let script = r#"
import sys
import os
from pdf2docx import Converter
import docx

src = sys.argv[1]
dst = sys.argv[2]

try:
    cv = Converter(src)
    cv.convert(dst)
    cv.close()
except Exception as e:
    sys.stderr.write(f"pdf2docx conversion error: {e}\n")
    sys.exit(1)

# Post-processing: Correct Z-ordering without deleting any element.
# Background drawings (cover templates, header designs, borders) remain intact at behindDoc="1" & relativeHeight="0".
# Real content images (photos, certificates, diagrams) are elevated to behindDoc="0" and higher relativeHeight
# so Word renders them in front of the background vector sheet instead of masking them beneath it.
try:
    if os.path.exists(dst):
        doc = docx.Document(dst)
        modified = False

        content_idx = 0
        for d in doc._element.xpath('.//w:drawing'):
            for a in d.xpath('./wp:anchor'):
                extent = a.xpath('./wp:extent')
                if extent:
                    cx = int(extent[0].get('cx', '0'))
                    cy = int(extent[0].get('cy', '0'))
                    # Full-page vector backgrounds span >5.0 x >7.8 inches (>4,500,000 x >7,100,000 EMUs)
                    # in either portrait or landscape orientation
                    is_full_page = (cx > 4_500_000 and cy > 7_100_000) or (cx > 7_100_000 and cy > 4_500_000)
                    if is_full_page:
                        a.set('behindDoc', '1')
                        a.set('relativeHeight', '0')
                        modified = True
                    else:
                        content_idx += 1
                        a.set('behindDoc', '0')
                        a.set('relativeHeight', str(1000 + content_idx))
                        modified = True

        # Remove dummy blank white thumbnail bundled by python-docx's default template
        # so Windows Explorer displays the standard Word icon (Big 'W' with lines)
        pkg = doc.part.package
        thumb_rids = [rid for rid, rel in pkg.rels.items() if 'thumbnail' in rel.reltype.lower()]
        for rid in thumb_rids:
            del pkg.rels[rid]
            modified = True

        if modified:
            doc.save(dst)
except Exception as e:
    sys.stderr.write(f"Warning: post-process error: {e}\n")

# Explicit clean exit to immediately release all OS handles and memory
sys.exit(0)
"#;
    cmd.args(["-c", script]);
    cmd.arg(src);
    cmd.arg(output);
    cmd.stdout(std::process::Stdio::null());
    cmd.stderr(std::process::Stdio::null());

    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }

    let mut child = cmd
        .spawn()
        .map_err(|e| ConvertError::ConverterError(format!("Failed to execute python: {}", e)))?;

    // Wait for the converter to finish with a safety timeout (180 seconds).
    // If the process exceeds the timeout, forcefully kill it to guarantee 100% memory reclamation.
    let start = std::time::Instant::now();
    let timeout = std::time::Duration::from_secs(180);
    let mut finished = false;
    let mut exit_status = None;

    while start.elapsed() < timeout {
        match child.try_wait() {
            Ok(Some(status)) => {
                finished = true;
                exit_status = Some(status);
                break;
            }
            Ok(None) => {
                std::thread::sleep(std::time::Duration::from_millis(50));
            }
            Err(e) => {
                let _ = child.kill();
                return Err(ConvertError::ConverterError(format!("Error waiting on python process: {}", e)));
            }
        }
    }

    if !finished {
        let _ = child.kill();
        let _ = child.wait();
        return Err(ConvertError::ConverterError("pdf2docx conversion timed out after 180 seconds".to_string()));
    }

    let status = exit_status.ok_or_else(|| {
        ConvertError::ConverterError("pdf2docx process exited without status".to_string())
    })?;
    if status.success() && output.exists() {
        Ok(())
    } else {
        Err(ConvertError::ConverterError(
            "pdf2docx conversion exited with an error".to_string(),
        ))
    }
}

pub fn doc_to_pdf(src: &Path, output: &Path) -> Result<(), ConvertError> {
    if !src.exists() {
        return Err(ConvertError::SourceNotFound(src.to_string_lossy().to_string()));
    }
    let guard = crate::output_guard::OutputGuard::new(output);

    // Try Word COM first if available on Windows
    if try_word_com_convert(src, output).is_ok() && output.exists() {
        if let Ok(meta) = std::fs::metadata(output) {
            if meta.len() > 0 {
                guard.disarm();
                return Ok(());
            }
        }
    }

    // Pure Rust fallback: extract text and render via pdf-writer
    let paragraphs = extract_doc_paragraphs(src)?;
    render_text_to_pdf(&paragraphs, output)?;
    guard.disarm();
    Ok(())
}

fn try_word_com_convert(src: &Path, output: &Path) -> Result<(), ConvertError> {
    let abs_src = crate::output::clean_windows_path(src);
    let abs_dst = crate::output::clean_windows_path(output);

    let script = format!(
        "$word = New-Object -ComObject Word.Application; $word.Visible = $false; $word.DisplayAlerts = 0; try {{ $doc = $word.Documents.Open('{}', $false, $true); if ($doc -ne $null) {{ $doc.SaveAs([ref]'{}', [ref]17); $doc.Close([ref]0); [System.Runtime.InteropServices.Marshal]::ReleaseComObject($doc) | Out-Null; }} }} finally {{ $word.Quit(); [System.Runtime.InteropServices.Marshal]::ReleaseComObject($word) | Out-Null; [System.GC]::Collect(); [System.GC]::WaitForPendingFinalizers(); }}",
        abs_src.replace('\'', "''"),
        abs_dst.replace('\'', "''")
    );

    let mut cmd = std::process::Command::new("powershell.exe");
    cmd.args(["-NoProfile", "-NonInteractive", "-WindowStyle", "Hidden", "-Command", &script]);
    cmd.stdin(std::process::Stdio::null());
    cmd.stdout(std::process::Stdio::null());
    cmd.stderr(std::process::Stdio::null());

    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }

    let mut child = cmd.spawn().map_err(|e| ConvertError::ConverterError(e.to_string()))?;
    let start = std::time::Instant::now();
    let timeout = std::time::Duration::from_secs(45);
    let mut finished = false;
    let mut exit_status = None;

    while start.elapsed() < timeout {
        match child.try_wait() {
            Ok(Some(status)) => {
                finished = true;
                exit_status = Some(status);
                break;
            }
            Ok(None) => {
                std::thread::sleep(std::time::Duration::from_millis(50));
            }
            Err(e) => {
                let _ = child.kill();
                return Err(ConvertError::ConverterError(format!("Word COM error: {}", e)));
            }
        }
    }

    if !finished {
        let _ = child.kill();
        let _ = child.wait();
        return Err(ConvertError::ConverterError("Word COM conversion timed out".to_string()));
    }

    let status = exit_status.ok_or_else(|| {
        ConvertError::ConverterError("Word COM process exited without status".to_string())
    })?;
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
        let mut cur_char_count = 0;
        for w in words {
            let w_char_count = w.chars().count();
            if cur_line.is_empty() {
                if w_char_count > 75 {
                    let mut chunk = String::with_capacity(75);
                    let mut count = 0;
                    for ch in w.chars() {
                        if count >= 75 {
                            wrapped_lines.push(chunk);
                            chunk = String::with_capacity(75);
                            count = 0;
                        }
                        chunk.push(ch);
                        count += 1;
                    }
                    if !chunk.is_empty() {
                        cur_line = chunk;
                        cur_char_count = count;
                    }
                } else {
                    cur_line = w.to_string();
                    cur_char_count = w_char_count;
                }
            } else if cur_char_count + w_char_count + 1 > 75 {
                wrapped_lines.push(cur_line);
                cur_line = String::new();
                cur_char_count = 0;
                if w_char_count > 75 {
                    let mut chunk = String::with_capacity(75);
                    let mut count = 0;
                    for ch in w.chars() {
                        if count >= 75 {
                            wrapped_lines.push(chunk);
                            chunk = String::with_capacity(75);
                            count = 0;
                        }
                        chunk.push(ch);
                        count += 1;
                    }
                    if !chunk.is_empty() {
                        cur_line = chunk;
                        cur_char_count = count;
                    }
                } else {
                    cur_line = w.to_string();
                    cur_char_count = w_char_count;
                }
            } else {
                cur_line.push(' ');
                cur_line.push_str(w);
                cur_char_count += 1 + w_char_count;
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
