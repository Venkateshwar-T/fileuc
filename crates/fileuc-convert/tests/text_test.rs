#[test]
fn test_md_to_html_conversion() {
    let temp_dir = tempfile::tempdir().unwrap();
    let md_path = temp_dir.path().join("test_doc.md");
    let html_path = temp_dir.path().join("test_doc.html");

    let markdown_content = r#"# Hello World
This is a **bold** and *italic* paragraph with `inline code`.

- Item 1
- Item 2
- [x] Done task

| Header A | Header B |
| --- | --- |
| Val 1 | Val 2 |
"#;
    std::fs::write(&md_path, markdown_content).unwrap();

    let res = fileuc_convert::text::md_to_html(&md_path, &html_path);
    assert!(res.is_ok());
    assert!(html_path.exists());

    let content = std::fs::read_to_string(&html_path).unwrap();
    assert!(content.contains("<!DOCTYPE html>"));
    assert!(content.contains("<title>test_doc</title>"));
    assert!(content.contains("<h1>Hello World</h1>"));
    assert!(content.contains("<strong>bold</strong>"));
    assert!(content.contains("<em>italic</em>"));
    assert!(content.contains("<table>"));
}

#[test]
fn test_txt_to_html_conversion() {
    let temp_dir = tempfile::tempdir().unwrap();
    let txt_path = temp_dir.path().join("plain_notes.txt");
    let html_path = temp_dir.path().join("plain_notes.html");

    let text_content = "Line 1: Hello <World> & Friends\nLine 2: 100% Rust converter";
    std::fs::write(&txt_path, text_content).unwrap();

    let res = fileuc_convert::text::txt_to_html(&txt_path, &html_path);
    assert!(res.is_ok());
    assert!(html_path.exists());

    let content = std::fs::read_to_string(&html_path).unwrap();
    assert!(content.contains("<!DOCTYPE html>"));
    assert!(content.contains("<title>plain_notes</title>"));
    assert!(content.contains("&lt;World&gt; &amp; Friends"));
    assert!(content.contains("100% Rust converter"));
}
