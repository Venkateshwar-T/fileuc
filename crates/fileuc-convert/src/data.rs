use calamine::{open_workbook_auto, Data, Reader};
use fileuc_types::ConvertError;
use rust_xlsxwriter::{Format, Workbook};
use std::path::Path;

/// Convert XLSX/XLS workbook (first sheet) to standard CSV format.
pub fn xlsx_to_csv(src: &Path, dst: &Path) -> Result<(), ConvertError> {
    let mut workbook = open_workbook_auto(src)
        .map_err(|e| ConvertError::ConverterError(format!("Failed to open spreadsheet: {}", e)))?;
    let sheet_names = workbook.sheet_names();
    if sheet_names.is_empty() {
        return Err(ConvertError::ConverterError("Spreadsheet has no sheets".into()));
    }
    let sheet_name = sheet_names[0].clone();
    let range = workbook
        .worksheet_range(&sheet_name)
        .map_err(|e| ConvertError::ConverterError(format!("Failed to read sheet '{}': {}", sheet_name, e)))?;

    let mut wtr = csv::WriterBuilder::new()
        .from_path(dst)
        .map_err(|e| ConvertError::IoError(e.to_string()))?;

    for row in range.rows() {
        let mut record = Vec::with_capacity(row.len());
        for cell in row {
            record.push(format_cell(cell));
        }
        wtr.write_record(&record)
            .map_err(|e| ConvertError::IoError(e.to_string()))?;
    }
    wtr.flush().map_err(|e| ConvertError::IoError(e.to_string()))?;
    Ok(())
}

/// Convert XLSX/XLS workbook (first sheet) to JSON.
pub fn xlsx_to_json(src: &Path, dst: &Path) -> Result<(), ConvertError> {
    let mut workbook = open_workbook_auto(src)
        .map_err(|e| ConvertError::ConverterError(format!("Failed to open spreadsheet: {}", e)))?;
    let sheet_names = workbook.sheet_names();
    if sheet_names.is_empty() {
        return Err(ConvertError::ConverterError("Spreadsheet has no sheets".into()));
    }
    let sheet_name = sheet_names[0].clone();
    let range = workbook
        .worksheet_range(&sheet_name)
        .map_err(|e| ConvertError::ConverterError(format!("Failed to read sheet '{}': {}", sheet_name, e)))?;

    let rows: Vec<&[Data]> = range.rows().collect();
    if rows.is_empty() {
        std::fs::write(dst, "[]").map_err(|e| ConvertError::IoError(e.to_string()))?;
        return Ok(());
    }

    let file = std::fs::File::create(dst).map_err(|e| ConvertError::IoError(e.to_string()))?;
    let mut writer = std::io::BufWriter::new(file);

    let first_row = rows[0];
    let has_headers = first_row.iter().any(|c| matches!(c, Data::String(_))) && rows.len() > 1;

    if has_headers {
        let mut headers: Vec<String> = Vec::with_capacity(first_row.len());
        for (i, c) in first_row.iter().enumerate() {
            let h = format_cell(c);
            let h = if h.trim().is_empty() {
                format!("column_{}", i + 1)
            } else {
                h.trim().to_string()
            };
            headers.push(h);
        }

        let mut records = Vec::with_capacity(rows.len() - 1);
        for row in &rows[1..] {
            let mut map = serde_json::Map::with_capacity(headers.len());
            for (i, h) in headers.iter().enumerate() {
                let val = row.get(i).map(cell_to_json_value).unwrap_or(serde_json::Value::Null);
                map.insert(h.clone(), val);
            }
            records.push(serde_json::Value::Object(map));
        }

        serde_json::to_writer_pretty(&mut writer, &records)
            .map_err(|e| ConvertError::ConverterError(e.to_string()))?;
    } else {
        let mut records = Vec::with_capacity(rows.len());
        for row in &rows {
            let record: Vec<serde_json::Value> = row.iter().map(cell_to_json_value).collect();
            records.push(serde_json::Value::Array(record));
        }

        serde_json::to_writer_pretty(&mut writer, &records)
            .map_err(|e| ConvertError::ConverterError(e.to_string()))?;
    }

    use std::io::Write;
    writer.flush().map_err(|e| ConvertError::IoError(e.to_string()))?;
    Ok(())
}

/// Convert CSV to Excel XLSX workbook.
pub fn csv_to_xlsx(src: &Path, dst: &Path) -> Result<(), ConvertError> {
    let mut rdr = csv::ReaderBuilder::new()
        .flexible(true)
        .has_headers(false)
        .from_path(src)
        .map_err(|e| ConvertError::IoError(e.to_string()))?;

    let mut workbook = Workbook::new();
    let worksheet = workbook.add_worksheet();
    let header_format = Format::new().set_bold();

    for (r, result) in rdr.records().enumerate() {
        let record = result.map_err(|e| ConvertError::ConverterError(e.to_string()))?;
        let row_idx = r as u32;

        for (c, field) in record.iter().enumerate() {
            let col_idx = c as u16;
            let trimmed = field.trim();

            let has_leading_zero = trimmed.len() > 1 && trimmed.starts_with('0') && !trimmed.starts_with("0.");
            if r == 0 {
                worksheet
                    .write_string_with_format(row_idx, col_idx, field, &header_format)
                    .map_err(|e| ConvertError::ConverterError(e.to_string()))?;
            } else if !has_leading_zero
                && trimmed.parse::<i64>().map_or(false, |v| v.abs() <= 9_007_199_254_740_992)
            {
                if let Ok(val) = trimmed.parse::<i64>() {
                    worksheet
                        .write_number(row_idx, col_idx, val as f64)
                        .map_err(|e| ConvertError::ConverterError(e.to_string()))?;
                }
            } else if !has_leading_zero && trimmed.parse::<f64>().is_ok() {
                if let Ok(val) = trimmed.parse::<f64>() {
                    worksheet
                        .write_number(row_idx, col_idx, val)
                        .map_err(|e| ConvertError::ConverterError(e.to_string()))?;
                }
            } else if trimmed.eq_ignore_ascii_case("true") {
                worksheet
                    .write_boolean(row_idx, col_idx, true)
                    .map_err(|e| ConvertError::ConverterError(e.to_string()))?;
            } else if trimmed.eq_ignore_ascii_case("false") {
                worksheet
                    .write_boolean(row_idx, col_idx, false)
                    .map_err(|e| ConvertError::ConverterError(e.to_string()))?;
            } else {
                worksheet
                    .write_string(row_idx, col_idx, field)
                    .map_err(|e| ConvertError::ConverterError(e.to_string()))?;
            }
        }
    }

    worksheet.autofit();
    workbook
        .save(dst)
        .map_err(|e| ConvertError::ConverterError(format!("Failed to save XLSX: {}", e)))?;
    Ok(())
}

/// Convert CSV to JSON format.
pub fn csv_to_json(src: &Path, dst: &Path) -> Result<(), ConvertError> {
    let mut rdr = csv::ReaderBuilder::new()
        .flexible(true)
        .has_headers(true)
        .from_path(src)
        .map_err(|e| ConvertError::IoError(e.to_string()))?;

    let headers = rdr
        .headers()
        .map_err(|e| ConvertError::ConverterError(e.to_string()))?
        .clone();

    let mut records = Vec::new();
    for result in rdr.records() {
        let record = result.map_err(|e| ConvertError::ConverterError(e.to_string()))?;
        let mut map = serde_json::Map::with_capacity(headers.len());
        for (i, h) in headers.iter().enumerate() {
            let field = record.get(i).unwrap_or("");
            let key = if h.trim().is_empty() {
                format!("column_{}", i + 1)
            } else {
                h.trim().to_string()
            };
            map.insert(key, parse_inferred_json_value(field));
        }
        records.push(serde_json::Value::Object(map));
    }

    let file = std::fs::File::create(dst).map_err(|e| ConvertError::IoError(e.to_string()))?;
    let mut writer = std::io::BufWriter::new(file);
    serde_json::to_writer_pretty(&mut writer, &records)
        .map_err(|e| ConvertError::ConverterError(e.to_string()))?;

    use std::io::Write;
    writer.flush().map_err(|e| ConvertError::IoError(e.to_string()))?;
    Ok(())
}

/// Convert JSON to Excel XLSX workbook.
pub fn json_to_xlsx(src: &Path, dst: &Path) -> Result<(), ConvertError> {
    let file = std::fs::File::open(src)
        .map_err(|e| ConvertError::IoError(e.to_string()))?;
    let reader = std::io::BufReader::new(file);
    let json_val: serde_json::Value = serde_json::from_reader(reader)
        .map_err(|e| ConvertError::ConverterError(format!("Invalid JSON: {}", e)))?;

    let mut workbook = Workbook::new();
    let worksheet = workbook.add_worksheet();
    let header_format = Format::new().set_bold();

    match json_val {
        serde_json::Value::Array(arr) => {
            if arr.is_empty() {
                workbook
                    .save(dst)
                    .map_err(|e| ConvertError::ConverterError(e.to_string()))?;
                return Ok(());
            }

            if arr.iter().any(|v| v.is_object()) {
                let mut keys: Vec<String> = Vec::new();
                for item in &arr {
                    if let serde_json::Value::Object(map) = item {
                        for k in map.keys() {
                            if !keys.contains(k) {
                                keys.push(k.clone());
                            }
                        }
                    }
                }

                for (c, key) in keys.iter().enumerate() {
                    worksheet
                        .write_string_with_format(0, c as u16, key, &header_format)
                        .map_err(|e| ConvertError::ConverterError(e.to_string()))?;
                }

                for (r, item) in arr.iter().enumerate() {
                    let row_idx = (r + 1) as u32;
                    if let serde_json::Value::Object(map) = item {
                        for (c, key) in keys.iter().enumerate() {
                            if let Some(val) = map.get(key) {
                                write_json_val_to_cell(worksheet, row_idx, c as u16, val)?;
                            }
                        }
                    }
                }
            } else {
                for (r, row_val) in arr.iter().enumerate() {
                    let row_idx = r as u32;
                    if let serde_json::Value::Array(sub_arr) = row_val {
                        for (c, cell_val) in sub_arr.iter().enumerate() {
                            write_json_val_to_cell(worksheet, row_idx, c as u16, cell_val)?;
                        }
                    } else {
                        write_json_val_to_cell(worksheet, row_idx, 0, row_val)?;
                    }
                }
            }
        }
        serde_json::Value::Object(map) => {
            worksheet
                .write_string_with_format(0, 0, "Key", &header_format)
                .map_err(|e| ConvertError::ConverterError(e.to_string()))?;
            worksheet
                .write_string_with_format(0, 1, "Value", &header_format)
                .map_err(|e| ConvertError::ConverterError(e.to_string()))?;

            for (r, (k, v)) in map.iter().enumerate() {
                let row_idx = (r + 1) as u32;
                worksheet
                    .write_string(row_idx, 0, k)
                    .map_err(|e| ConvertError::ConverterError(e.to_string()))?;
                write_json_val_to_cell(worksheet, row_idx, 1, v)?;
            }
        }
        _ => {
            return Err(ConvertError::ConverterError(
                "JSON root must be an array or object".into(),
            ));
        }
    }

    worksheet.autofit();
    workbook
        .save(dst)
        .map_err(|e| ConvertError::ConverterError(format!("Failed to save XLSX: {}", e)))?;
    Ok(())
}

/// Convert JSON to CSV format.
pub fn json_to_csv(src: &Path, dst: &Path) -> Result<(), ConvertError> {
    let file = std::fs::File::open(src)
        .map_err(|e| ConvertError::IoError(e.to_string()))?;
    let reader = std::io::BufReader::new(file);
    let json_val: serde_json::Value = serde_json::from_reader(reader)
        .map_err(|e| ConvertError::ConverterError(format!("Invalid JSON: {}", e)))?;

    let mut wtr = csv::WriterBuilder::new()
        .from_path(dst)
        .map_err(|e| ConvertError::IoError(e.to_string()))?;

    match json_val {
        serde_json::Value::Array(arr) => {
            if arr.is_empty() {
                wtr.flush().map_err(|e| ConvertError::IoError(e.to_string()))?;
                return Ok(());
            }

            if arr.iter().any(|v| v.is_object()) {
                let mut keys: Vec<String> = Vec::new();
                for item in &arr {
                    if let serde_json::Value::Object(map) = item {
                        for k in map.keys() {
                            if !keys.contains(k) {
                                keys.push(k.clone());
                            }
                        }
                    }
                }

                wtr.write_record(&keys)
                    .map_err(|e| ConvertError::IoError(e.to_string()))?;

                for item in &arr {
                    let mut record = Vec::with_capacity(keys.len());
                    if let serde_json::Value::Object(map) = item {
                        for k in &keys {
                            let s = match map.get(k) {
                                None | Some(serde_json::Value::Null) => String::new(),
                                Some(serde_json::Value::String(s)) => s.clone(),
                                Some(other) => other.to_string(),
                            };
                            record.push(s);
                        }
                    }
                    wtr.write_record(&record)
                        .map_err(|e| ConvertError::IoError(e.to_string()))?;
                }
            } else {
                for row_val in &arr {
                    if let serde_json::Value::Array(sub_arr) = row_val {
                        let record: Vec<String> = sub_arr
                            .iter()
                            .map(|v| match v {
                                serde_json::Value::Null => String::new(),
                                serde_json::Value::String(s) => s.clone(),
                                other => other.to_string(),
                            })
                            .collect();
                        wtr.write_record(&record)
                            .map_err(|e| ConvertError::IoError(e.to_string()))?;
                    } else {
                        wtr.write_record(&[row_val.to_string()])
                            .map_err(|e| ConvertError::IoError(e.to_string()))?;
                    }
                }
            }
        }
        serde_json::Value::Object(map) => {
            wtr.write_record(&["Key", "Value"])
                .map_err(|e| ConvertError::IoError(e.to_string()))?;
            for (k, v) in &map {
                let s = match v {
                    serde_json::Value::Null => String::new(),
                    serde_json::Value::String(s) => s.clone(),
                    other => other.to_string(),
                };
                wtr.write_record(&[k.as_str(), &s])
                    .map_err(|e| ConvertError::IoError(e.to_string()))?;
            }
        }
        _ => {
            return Err(ConvertError::ConverterError(
                "JSON root must be an array or object".into(),
            ));
        }
    }

    wtr.flush().map_err(|e| ConvertError::IoError(e.to_string()))?;
    Ok(())
}

fn write_json_val_to_cell(
    worksheet: &mut rust_xlsxwriter::Worksheet,
    row: u32,
    col: u16,
    val: &serde_json::Value,
) -> Result<(), ConvertError> {
    match val {
        serde_json::Value::Null => Ok(()),
        serde_json::Value::Bool(b) => {
            worksheet
                .write_boolean(row, col, *b)
                .map_err(|e| ConvertError::ConverterError(e.to_string()))?;
            Ok(())
        }
        serde_json::Value::Number(n) => {
            if let Some(f) = n.as_f64() {
                worksheet
                    .write_number(row, col, f)
                    .map_err(|e| ConvertError::ConverterError(e.to_string()))?;
            } else {
                worksheet
                    .write_string(row, col, n.to_string())
                    .map_err(|e| ConvertError::ConverterError(e.to_string()))?;
            }
            Ok(())
        }
        serde_json::Value::String(s) => {
            worksheet
                .write_string(row, col, s)
                .map_err(|e| ConvertError::ConverterError(e.to_string()))?;
            Ok(())
        }
        _ => {
            worksheet
                .write_string(row, col, val.to_string())
                .map_err(|e| ConvertError::ConverterError(e.to_string()))?;
            Ok(())
        }
    }
}

fn format_cell(cell: &Data) -> String {
    match cell {
        Data::Empty => String::new(),
        Data::String(s) => s.clone(),
        Data::Float(f) => {
            if f.fract() == 0.0 && *f >= i64::MIN as f64 && *f <= i64::MAX as f64 {
                format!("{}", *f as i64)
            } else {
                format!("{}", f)
            }
        }
        Data::Int(i) => format!("{}", i),
        Data::Bool(b) => format!("{}", b),
        Data::DateTime(d) => format!("{}", d),
        Data::DateTimeIso(d) => d.clone(),
        Data::DurationIso(d) => d.clone(),
        Data::Error(e) => format!("{:?}", e),
    }
}

fn cell_to_json_value(cell: &Data) -> serde_json::Value {
    match cell {
        Data::Empty => serde_json::Value::Null,
        Data::String(s) => serde_json::Value::String(s.clone()),
        Data::Float(f) => {
            if f.fract() == 0.0 && *f >= i64::MIN as f64 && *f <= i64::MAX as f64 {
                serde_json::Value::Number((*f as i64).into())
            } else if let Some(n) = serde_json::Number::from_f64(*f) {
                serde_json::Value::Number(n)
            } else {
                serde_json::Value::String(f.to_string())
            }
        }
        Data::Int(i) => serde_json::Value::Number((*i).into()),
        Data::Bool(b) => serde_json::Value::Bool(*b),
        Data::DateTime(d) => serde_json::Value::String(d.to_string()),
        Data::DateTimeIso(d) => serde_json::Value::String(d.clone()),
        Data::DurationIso(d) => serde_json::Value::String(d.clone()),
        Data::Error(e) => serde_json::Value::String(format!("{:?}", e)),
    }
}

fn parse_inferred_json_value(text: &str) -> serde_json::Value {
    let trimmed = text.trim();
    let has_leading_zero = trimmed.len() > 1 && trimmed.starts_with('0') && !trimmed.starts_with("0.");
    if trimmed.is_empty() {
        serde_json::Value::Null
    } else if !has_leading_zero && trimmed.parse::<i64>().is_ok() {
        if let Ok(val) = trimmed.parse::<i64>() {
            serde_json::Value::Number(val.into())
        } else {
            serde_json::Value::String(text.to_string())
        }
    } else if !has_leading_zero && trimmed.parse::<f64>().is_ok() {
        if let Ok(val) = trimmed.parse::<f64>() {
            if let Some(n) = serde_json::Number::from_f64(val) {
                serde_json::Value::Number(n)
            } else {
                serde_json::Value::String(text.to_string())
            }
        } else {
            serde_json::Value::String(text.to_string())
        }
    } else if trimmed.eq_ignore_ascii_case("true") {
        serde_json::Value::Bool(true)
    } else if trimmed.eq_ignore_ascii_case("false") {
        serde_json::Value::Bool(false)
    } else {
        serde_json::Value::String(text.to_string())
    }
}
