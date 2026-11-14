use fileuc_types::ConvertError;
use std::path::Path;

static HTML_STYLE: &str = r#"
  :root {
    --bg-color: #ffffff;
    --text-color: #1f2328;
    --border-color: #d1d9e0;
    --code-bg: #f6f8fa;
    --link-color: #0969da;
    --quote-color: #59636e;
  }
  @media (prefers-color-scheme: dark) {
    :root {
      --bg-color: #0d1117;
      --text-color: #e6edf3;
      --border-color: #30363d;
      --code-bg: #161b22;
      --link-color: #4493f8;
      --quote-color: #8b949e;
    }
  }
  body {
    font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", "Noto Sans", Helvetica, Arial, sans-serif;
    line-height: 1.6;
    color: var(--text-color);
    background-color: var(--bg-color);
    max-width: 820px;
    margin: 40px auto;
    padding: 0 24px;
    box-sizing: border-box;
    word-wrap: break-word;
  }
  h1, h2, h3, h4, h5, h6 {
    margin-top: 24px;
    margin-bottom: 16px;
    font-weight: 600;
    line-height: 1.25;
  }
  h1 { font-size: 2em; border-bottom: 1px solid var(--border-color); padding-bottom: 0.3em; }
  h2 { font-size: 1.5em; border-bottom: 1px solid var(--border-color); padding-bottom: 0.3em; }
  p { margin-top: 0; margin-bottom: 16px; }
  a { color: var(--link-color); text-decoration: none; }
  a:hover { text-decoration: underline; }
  blockquote {
    padding: 0 1em;
    color: var(--quote-color);
    border-left: 0.25em solid var(--border-color);
    margin: 0 0 16px 0;
  }
  pre {
    background-color: var(--code-bg);
    padding: 16px;
    border-radius: 6px;
    overflow: auto;
    font-family: ui-monospace, SFMono-Regular, "SF Mono", Menlo, Consolas, "Liberation Mono", monospace;
    font-size: 85%;
    line-height: 1.45;
  }
  code {
    background-color: var(--code-bg);
    padding: 0.2em 0.4em;
    border-radius: 6px;
    font-family: ui-monospace, SFMono-Regular, "SF Mono", Menlo, Consolas, "Liberation Mono", monospace;
    font-size: 85%;
  }
  pre code {
    padding: 0;
    background: transparent;
  }
  table {
    border-collapse: collapse;
    width: 100%;
    margin-bottom: 16px;
  }
  th, td {
    padding: 6px 13px;
    border: 1px solid var(--border-color);
  }
  tr:nth-child(2n) {
    background-color: var(--code-bg);
  }
  hr {
    height: 0.25em;
    padding: 0;
    margin: 24px 0;
    background-color: var(--border-color);
    border: 0;
  }
  ul, ol {
    padding-left: 2em;
    margin-top: 0;
    margin-bottom: 16px;
  }
"#;

pub fn md_to_html(src: &Path, output: &Path) -> Result<(), ConvertError> {
    if !src.exists() {
        return Err(ConvertError::SourceNotFound(src.to_string_lossy().to_string()));
    }

    let markdown_text = std::fs::read_to_string(src)
        .map_err(|e| ConvertError::IoError(format!("Failed to read markdown file: {}", e)))?;

    let title = src
        .file_stem()
        .map(|s| s.to_string_lossy())
        .unwrap_or_default();

    use pulldown_cmark::{html, Options, Parser};
    let mut options = Options::empty();
    options.insert(Options::ENABLE_TABLES);
    options.insert(Options::ENABLE_FOOTNOTES);
    options.insert(Options::ENABLE_STRIKETHROUGH);
    options.insert(Options::ENABLE_TASKLISTS);

    let parser = Parser::new_ext(&markdown_text, options);
    let mut html_body = String::new();
    html::push_html(&mut html_body, parser);

    let full_html = format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>{}</title>
  <style>{}</style>
</head>
<body>
{}
</body>
</html>"#,
        escape_html(&title),
        HTML_STYLE,
        html_body
    );

    std::fs::write(output, full_html)
        .map_err(|e| ConvertError::IoError(format!("Failed to write HTML file: {}", e)))?;

    Ok(())
}

pub fn txt_to_html(src: &Path, output: &Path) -> Result<(), ConvertError> {
    if !src.exists() {
        return Err(ConvertError::SourceNotFound(src.to_string_lossy().to_string()));
    }

    let raw_text = std::fs::read_to_string(src)
        .map_err(|e| ConvertError::IoError(format!("Failed to read text file: {}", e)))?;

    let title = src
        .file_stem()
        .map(|s| s.to_string_lossy())
        .unwrap_or_default();

    let normalized = raw_text.replace("\r\n", "\n").replace('\r', "\n");
    let mut body_html = String::with_capacity(raw_text.len() + 256);

    for para in normalized.split("\n\n") {
        let trimmed = para.trim();
        if trimmed.is_empty() {
            continue;
        }
        let escaped = escape_html(trimmed);
        let with_br = escaped.replace('\n', "<br>\n");
        body_html.push_str("  <p>");
        body_html.push_str(&with_br);
        body_html.push_str("</p>\n");
    }

    if body_html.is_empty() {
        body_html.push_str("  <p></p>\n");
    }

    let full_html = format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>{}</title>
  <style>{}</style>
</head>
<body>
{}
</body>
</html>"#,
        escape_html(&title),
        HTML_STYLE,
        body_html
    );

    std::fs::write(output, full_html)
        .map_err(|e| ConvertError::IoError(format!("Failed to write HTML file: {}", e)))?;

    Ok(())
}

pub fn escape_html(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(c),
        }
    }
    out
}

// =========================================================================
// PURE RUST PDF DOCUMENT GENERATION (Markdown, Text, HTML) VIA IRONPAGES
// =========================================================================

pub fn txt_to_pdf(src: &Path, output: &Path) -> Result<(), ConvertError> {
    if !src.exists() {
        return Err(ConvertError::SourceNotFound(src.to_string_lossy().to_string()));
    }

    let raw_text = std::fs::read_to_string(src)
        .map_err(|e| ConvertError::IoError(format!("Failed to read text file: {}", e)))?;

    let title = src
        .file_stem()
        .map(|s| s.to_string_lossy())
        .unwrap_or_else(|| "Document".into());

    let normalized = raw_text.replace("\r\n", "\n").replace('\r', "\n");
    let mut body_html = String::with_capacity(raw_text.len() + 256);

    for para in normalized.split("\n\n") {
        let trimmed = para.trim();
        if trimmed.is_empty() {
            continue;
        }
        let escaped = escape_html(trimmed);
        let with_br = escaped.replace('\n', "<br>\n");
        body_html.push_str("  <p>");
        body_html.push_str(&with_br);
        body_html.push_str("</p>\n");
    }

    if body_html.is_empty() {
        body_html.push_str("  <p></p>\n");
    }

    let full_html = format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <title>{}</title>
  <style>{}</style>
</head>
<body>
{}
</body>
</html>"#,
        escape_html(&title),
        HTML_STYLE,
        body_html
    );

    let pdf_bytes = ironpages::html_to_pdf(&full_html)
        .map_err(|e| ConvertError::ConverterError(format!("Ironpages TXT to PDF error: {}", e)))?;

    std::fs::write(output, pdf_bytes)
        .map_err(|e| ConvertError::IoError(format!("Failed to write PDF file: {}", e)))?;

    Ok(())
}

pub fn md_to_pdf(src: &Path, output: &Path) -> Result<(), ConvertError> {
    if !src.exists() {
        return Err(ConvertError::SourceNotFound(src.to_string_lossy().to_string()));
    }

    let markdown_text = std::fs::read_to_string(src)
        .map_err(|e| ConvertError::IoError(format!("Failed to read markdown file: {}", e)))?;

    let pdf_bytes = ironpages::markdown_to_pdf(&markdown_text)
        .map_err(|e| ConvertError::ConverterError(format!("Ironpages Markdown to PDF error: {}", e)))?;

    std::fs::write(output, pdf_bytes)
        .map_err(|e| ConvertError::IoError(format!("Failed to write PDF file: {}", e)))?;

    Ok(())
}

pub fn html_to_pdf(src: &Path, output: &Path) -> Result<(), ConvertError> {
    if !src.exists() {
        return Err(ConvertError::SourceNotFound(src.to_string_lossy().to_string()));
    }

    let html_content = std::fs::read_to_string(src)
        .map_err(|e| ConvertError::IoError(format!("Failed to read HTML file: {}", e)))?;

    let pdf_bytes = ironpages::html_to_pdf(&html_content)
        .map_err(|e| ConvertError::ConverterError(format!("Ironpages HTML to PDF error: {}", e)))?;

    std::fs::write(output, pdf_bytes)
        .map_err(|e| ConvertError::IoError(format!("Failed to write PDF file: {}", e)))?;

    Ok(())
}

/// Convert Markdown text to clean plain text, stripping markdown formatting while preserving readability.
pub fn markdown_to_text(src: &Path, dst: &Path) -> Result<(), ConvertError> {
    let content = std::fs::read_to_string(src)
        .map_err(|e| ConvertError::IoError(e.to_string()))?;

    use pulldown_cmark::{Event, Parser, Tag, TagEnd};

    let parser = Parser::new(&content);
    let mut text_output = String::with_capacity(content.len());

    for event in parser {
        match event {
            Event::Text(t) => {
                text_output.push_str(&t);
            }
            Event::Code(c) => {
                text_output.push_str(&c);
            }
            Event::Start(Tag::Paragraph) => {
                if !text_output.is_empty() && !text_output.ends_with("\n\n") {
                    if text_output.ends_with('\n') {
                        text_output.push('\n');
                    } else {
                        text_output.push_str("\r\n\r\n");
                    }
                }
            }
            Event::End(TagEnd::Paragraph) => {
                text_output.push_str("\r\n\r\n");
            }
            Event::Start(Tag::Heading { .. }) => {
                if !text_output.is_empty() && !text_output.ends_with('\n') {
                    text_output.push_str("\r\n\r\n");
                }
            }
            Event::End(TagEnd::Heading(_)) => {
                text_output.push_str("\r\n\r\n");
            }
            Event::Start(Tag::Item) => {
                text_output.push_str("• ");
            }
            Event::End(TagEnd::Item) => {
                text_output.push_str("\r\n");
            }
            Event::Start(Tag::CodeBlock(_)) => {
                if !text_output.is_empty() && !text_output.ends_with('\n') {
                    text_output.push_str("\r\n");
                }
            }
            Event::End(TagEnd::CodeBlock) => {
                text_output.push_str("\r\n");
            }
            Event::SoftBreak | Event::HardBreak => {
                text_output.push_str("\r\n");
            }
            _ => {}
        }
    }

    let clean = text_output.trim().to_string();
    std::fs::write(dst, clean).map_err(|e| ConvertError::IoError(e.to_string()))?;
    Ok(())
}

/// Convert plain text to normalized Markdown.
pub fn text_to_markdown(src: &Path, dst: &Path) -> Result<(), ConvertError> {
    let content = std::fs::read_to_string(src)
        .map_err(|e| ConvertError::IoError(e.to_string()))?;
    let trimmed = content.trim_end();
    let mut out = String::with_capacity(trimmed.len() + 2);
    out.push_str(trimmed);
    out.push_str("\r\n");
    std::fs::write(dst, out).map_err(|e| ConvertError::IoError(e.to_string()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_txt_to_pdf() {
        let temp_dir = tempfile::tempdir().unwrap();
        let src = temp_dir.path().join("sample.txt");
        let dst = temp_dir.path().join("sample.pdf");
        std::fs::write(&src, "Hello FileUC!\n\nThis is a second paragraph with some text.").unwrap();

        txt_to_pdf(&src, &dst).unwrap();
        assert!(dst.exists());
        assert!(std::fs::metadata(&dst).unwrap().len() > 100);
    }

    #[test]
    fn test_md_to_pdf() {
        let temp_dir = tempfile::tempdir().unwrap();
        let src = temp_dir.path().join("sample.md");
        let dst = temp_dir.path().join("sample.pdf");
        let md = "# Title Heading\n\nThis is a paragraph with **bold** text.\n\n- Item 1\n- Item 2\n\n```rust\nfn main() {}\n```\n";
        std::fs::write(&src, md).unwrap();

        md_to_pdf(&src, &dst).unwrap();
        assert!(dst.exists());
        assert!(std::fs::metadata(&dst).unwrap().len() > 100);
    }

    #[test]
    fn test_html_to_pdf() {
        let temp_dir = tempfile::tempdir().unwrap();
        let src = temp_dir.path().join("sample.html");
        let dst = temp_dir.path().join("sample.pdf");
        let html = "<html><head><title>Test</title></head><body><h1>Web Heading</h1><p>Paragraph inside HTML.</p><ul><li>List Item A</li><li>List Item B</li></ul></body></html>";
        std::fs::write(&src, html).unwrap();

        html_to_pdf(&src, &dst).unwrap();
        assert!(dst.exists());
        assert!(std::fs::metadata(&dst).unwrap().len() > 100);
    }

    #[test]
    fn test_txt_to_html_to_pdf_flow() {
        let temp_dir = tempfile::tempdir().unwrap();
        let txt_src = temp_dir.path().join("input.txt");
        let html_dst = temp_dir.path().join("output.html");
        let pdf_dst = temp_dir.path().join("final.pdf");

        let long_line = "This is a very long line of text that exceeds eighty characters in length to ensure that text wrapping works cleanly and beautifully without any clipping or right-side overflow when converted to HTML and PDF.";
        let text_content = format!("First Paragraph.\n\n{}\n\nSecond Paragraph with a break:\nLine A\nLine B", long_line);
        std::fs::write(&txt_src, &text_content).unwrap();

        // 1. Convert TXT -> HTML
        txt_to_html(&txt_src, &html_dst).unwrap();
        assert!(html_dst.exists());
        let html_str = std::fs::read_to_string(&html_dst).unwrap();
        assert!(html_str.contains("<p>First Paragraph.</p>"));

        // 2. Convert HTML -> PDF
        html_to_pdf(&html_dst, &pdf_dst).unwrap();
        assert!(pdf_dst.exists());
        assert!(std::fs::metadata(&pdf_dst).unwrap().len() > 200);
    }

    #[test]
    fn test_code_block_wrapping_and_pagination() {
        let temp_dir = tempfile::tempdir().unwrap();
        let html_src = temp_dir.path().join("code.html");
        let pdf_dst = temp_dir.path().join("code.pdf");

        // Code block with 120 lines and lines up to 150 characters
        let mut code_content = String::new();
        code_content.push_str("<pre><code>");
        for i in 0..120 {
            code_content.push_str(&format!("let very_long_variable_name_number_{} = \"This is an extraordinarily long string literal that must be wrapped so it does not spill past the right border of the code block box\";\n", i));
        }
        code_content.push_str("</code></pre>");

        std::fs::write(&html_src, &code_content).unwrap();

        html_to_pdf(&html_src, &pdf_dst).unwrap();
        assert!(pdf_dst.exists());
        assert!(std::fs::metadata(&pdf_dst).unwrap().len() > 1000);
    }
}
