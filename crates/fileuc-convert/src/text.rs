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

    let escaped = escape_html(&raw_text);

    let full_html = format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>{}</title>
  <style>{}
  pre.text-content {{
    white-space: pre-wrap;
    word-break: break-word;
    background: transparent;
    padding: 0;
    font-family: inherit;
    font-size: 1rem;
    line-height: 1.6;
  }}
  </style>
</head>
<body>
  <pre class="text-content">{}</pre>
</body>
</html>"#,
        escape_html(&title),
        HTML_STYLE,
        escaped
    );

    std::fs::write(output, full_html)
        .map_err(|e| ConvertError::IoError(format!("Failed to write HTML file: {}", e)))?;

    Ok(())
}

fn escape_html(s: &str) -> String {
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
