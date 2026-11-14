use fileuc_convert::data::{csv_to_json, csv_to_xlsx, json_to_csv, json_to_xlsx, xlsx_to_csv, xlsx_to_json};
use fileuc_convert::text::{markdown_to_text, text_to_markdown};
use std::fs;
use tempfile::tempdir;

#[test]
fn test_csv_and_json_roundtrip() {
    let dir = tempdir().unwrap();
    let csv_file = dir.path().join("data.csv");
    let json_file = dir.path().join("data.json");
    let csv_out = dir.path().join("roundtrip.csv");

    let csv_data = "Name,Age,Active\nAlice,30,true\nBob,25,false\n";
    fs::write(&csv_file, csv_data).unwrap();

    // CSV -> JSON
    csv_to_json(&csv_file, &json_file).unwrap();
    assert!(json_file.exists());
    let json_content = fs::read_to_string(&json_file).unwrap();
    assert!(json_content.contains("\"Name\": \"Alice\""));
    assert!(json_content.contains("\"Age\": 30"));
    assert!(json_content.contains("\"Active\": true"));

    // JSON -> CSV
    json_to_csv(&json_file, &csv_out).unwrap();
    assert!(csv_out.exists());
    let roundtrip_csv = fs::read_to_string(&csv_out).unwrap();
    assert!(roundtrip_csv.contains("Alice"));
    assert!(roundtrip_csv.contains("30"));
}

#[test]
fn test_csv_to_xlsx_and_xlsx_to_csv() {
    let dir = tempdir().unwrap();
    let csv_file = dir.path().join("input.csv");
    let xlsx_file = dir.path().join("output.xlsx");
    let csv_out = dir.path().join("from_xlsx.csv");
    let json_out = dir.path().join("from_xlsx.json");

    let csv_data = "Item,Count,Price\nApple,10,2.5\nBanana,20,1.2\n";
    fs::write(&csv_file, csv_data).unwrap();

    // CSV -> XLSX
    csv_to_xlsx(&csv_file, &xlsx_file).unwrap();
    assert!(xlsx_file.exists());
    assert!(fs::metadata(&xlsx_file).unwrap().len() > 100);

    // XLSX -> CSV
    xlsx_to_csv(&xlsx_file, &csv_out).unwrap();
    assert!(csv_out.exists());
    let csv_content = fs::read_to_string(&csv_out).unwrap();
    assert!(csv_content.contains("Apple"));
    assert!(csv_content.contains("10"));

    // XLSX -> JSON
    xlsx_to_json(&xlsx_file, &json_out).unwrap();
    assert!(json_out.exists());
    let json_content = fs::read_to_string(&json_out).unwrap();
    assert!(json_content.contains("\"Item\": \"Apple\""));
    assert!(json_content.contains("\"Count\": 10"));
}

#[test]
fn test_json_to_xlsx() {
    let dir = tempdir().unwrap();
    let json_file = dir.path().join("users.json");
    let xlsx_file = dir.path().join("users.xlsx");

    let json_data = r#"[{"id": 1, "username": "sharad"}, {"id": 2, "username": "antigravity"}]"#;
    fs::write(&json_file, json_data).unwrap();

    json_to_xlsx(&json_file, &xlsx_file).unwrap();
    assert!(xlsx_file.exists());
    assert!(fs::metadata(&xlsx_file).unwrap().len() > 100);
}

#[test]
fn test_markdown_and_text_roundtrip() {
    let dir = tempdir().unwrap();
    let md_file = dir.path().join("doc.md");
    let txt_file = dir.path().join("doc.txt");
    let md_out = dir.path().join("doc_back.md");

    let md_content = "# Project FileUC\n\nHigh speed desktop file utility.\n\n- Feature 1\n- Feature 2\n\n```rust\nlet x = 42;\n```\n";
    fs::write(&md_file, md_content).unwrap();

    // MD -> TXT
    markdown_to_text(&md_file, &txt_file).unwrap();
    assert!(txt_file.exists());
    let txt = fs::read_to_string(&txt_file).unwrap();
    assert!(txt.contains("Project FileUC"));
    assert!(txt.contains("High speed desktop file utility."));
    assert!(txt.contains("• Feature 1"));
    assert!(txt.contains("let x = 42;"));
    // Ensure no markdown heading hashes
    assert!(!txt.starts_with('#'));

    // TXT -> MD
    text_to_markdown(&txt_file, &md_out).unwrap();
    assert!(md_out.exists());
    let md = fs::read_to_string(&md_out).unwrap();
    assert!(md.contains("Project FileUC"));
}

#[test]
fn test_column_order_preservation() {
    let dir = tempdir().unwrap();
    let original_csv = dir.path().join("orig.csv");
    let xlsx_1 = dir.path().join("step1.xlsx");
    let json_file = dir.path().join("step2.json");
    let xlsx_2 = dir.path().join("step3.xlsx");
    let final_csv = dir.path().join("step4.csv");

    // Columns intentionally NOT alphabetical
    let csv_content = "Zebra,Apple,Monkey,Banana\nStripes,Red,Tail,Yellow\nWild,Fruit,Jungle,Plant\n";
    fs::write(&original_csv, csv_content).unwrap();

    // 1. CSV -> XLSX
    csv_to_xlsx(&original_csv, &xlsx_1).unwrap();

    // 2. XLSX -> JSON
    xlsx_to_json(&xlsx_1, &json_file).unwrap();
    let json_str = fs::read_to_string(&json_file).unwrap();
    let zebra_pos = json_str.find("\"Zebra\"").unwrap();
    let apple_pos = json_str.find("\"Apple\"").unwrap();
    let monkey_pos = json_str.find("\"Monkey\"").unwrap();
    let banana_pos = json_str.find("\"Banana\"").unwrap();
    assert!(zebra_pos < apple_pos, "Zebra must come before Apple in JSON");
    assert!(apple_pos < monkey_pos, "Apple must come before Monkey in JSON");
    assert!(monkey_pos < banana_pos, "Monkey must come before Banana in JSON");

    // 3. JSON -> XLSX
    json_to_xlsx(&json_file, &xlsx_2).unwrap();

    // 4. XLSX -> CSV
    xlsx_to_csv(&xlsx_2, &final_csv).unwrap();
    let final_content = fs::read_to_string(&final_csv).unwrap();
    let first_line = final_content.lines().next().unwrap();
    assert_eq!(first_line, "Zebra,Apple,Monkey,Banana", "Original column order must be preserved across XLSX <-> JSON roundtrip");
}
