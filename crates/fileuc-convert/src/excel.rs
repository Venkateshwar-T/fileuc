use fileuc_types::ConvertError;
use std::path::Path;

pub fn excel_to_pdf(src: &Path, output: &Path) -> Result<(), ConvertError> {
    if !src.exists() {
        return Err(ConvertError::SourceNotFound(src.to_string_lossy().to_string()));
    }
    let guard = crate::output_guard::OutputGuard::new(output);

    // 1. Try Excel COM first if Excel is installed on Windows
    if try_excel_com_convert(src, output).is_ok() && output.exists() {
        if let Ok(meta) = std::fs::metadata(output) {
            if meta.len() > 0 {
                guard.disarm();
                return Ok(());
            }
        }
    }

    // 2. Try Python plugin engine (openpyxl + reportlab)
    if let Some(python_exe) = crate::doc::find_pdf2docx() {
        run_excel_to_pdf_converter(&python_exe, src, output)?;
        if output.exists() {
            if let Ok(meta) = std::fs::metadata(output) {
                if meta.len() > 0 {
                    guard.disarm();
                    return Ok(());
                }
            }
        }
        return Err(ConvertError::ConverterError(
            "Excel to PDF conversion failed.".to_string(),
        ));
    }

    Err(ConvertError::EngineMissing(
        "Office & Document Processing Engine is not installed. Please open FileUC Settings -> Plugins to install it.".to_string(),
    ))
}

pub fn pdf_to_excel(src: &Path, output: &Path) -> Result<(), ConvertError> {
    if !src.exists() {
        return Err(ConvertError::SourceNotFound(src.to_string_lossy().to_string()));
    }
    let guard = crate::output_guard::OutputGuard::new(output);

    if let Some(python_exe) = crate::doc::find_pdf2docx() {
        run_pdf_to_excel_converter(&python_exe, src, output)?;
        if output.exists() {
            if let Ok(meta) = std::fs::metadata(output) {
                if meta.len() > 0 {
                    guard.disarm();
                    return Ok(());
                }
            }
        }
        return Err(ConvertError::ConverterError(
            "PDF to Excel conversion failed.".to_string(),
        ));
    }

    Err(ConvertError::EngineMissing(
        "Office & Document Processing Engine is not installed. Please open FileUC Settings -> Plugins to install it.".to_string(),
    ))
}

fn try_excel_com_convert(src: &Path, output: &Path) -> Result<(), ConvertError> {
    let abs_src = crate::output::clean_windows_path(src);
    let abs_dst = crate::output::clean_windows_path(output);

    let script = format!(
        "$excel = New-Object -ComObject Excel.Application; $excel.Visible = $false; $excel.DisplayAlerts = $false; try {{ $wb = $excel.Workbooks.Open('{}', $false, $true); if ($wb -ne $null) {{ $wb.ExportAsFixedFormat(0, '{}'); $wb.Close($false); [System.Runtime.InteropServices.Marshal]::ReleaseComObject($wb) | Out-Null; }} }} finally {{ $excel.Quit(); [System.Runtime.InteropServices.Marshal]::ReleaseComObject($excel) | Out-Null; [System.GC]::Collect(); [System.GC]::WaitForPendingFinalizers(); }}",
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
                return Err(ConvertError::ConverterError(format!("Excel COM error: {}", e)));
            }
        }
    }

    if !finished {
        let _ = child.kill();
        let _ = child.wait();
        return Err(ConvertError::ConverterError("Excel COM conversion timed out".to_string()));
    }

    let status = exit_status.ok_or_else(|| {
        ConvertError::ConverterError("Excel COM process exited without status".to_string())
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
    Err(ConvertError::ConverterError("Excel COM conversion failed".to_string()))
}

fn run_excel_to_pdf_converter(python_exe: &Path, src: &Path, output: &Path) -> Result<(), ConvertError> {
    let mut cmd = std::process::Command::new(python_exe);
    let script = r#"
import sys
import os
import openpyxl
from reportlab.lib.pagesizes import A4, landscape
from reportlab.lib import colors
from reportlab.platypus import SimpleDocTemplate, Table, TableStyle, Paragraph, Spacer, PageBreak
from reportlab.lib.styles import getSampleStyleSheet, ParagraphStyle

src = sys.argv[1]
dst = sys.argv[2]

try:
    wb = openpyxl.load_workbook(src, data_only=True)
    
    max_c_overall = max((sheet.max_column for sheet in wb.worksheets if sheet.max_column is not None), default=1)
    page_format = landscape(A4) if max_c_overall > 6 else A4

    doc = SimpleDocTemplate(
        dst,
        pagesize=page_format,
        rightMargin=25,
        leftMargin=25,
        topMargin=25,
        bottomMargin=25
    )

    styles = getSampleStyleSheet()
    title_style = ParagraphStyle(
        'SheetTitle',
        parent=styles["Heading2"],
        fontName='Helvetica-Bold',
        fontSize=13,
        leading=16,
        textColor=colors.HexColor('#1E293B')
    )
    cell_style = ParagraphStyle(
        'CellStyle',
        parent=styles["Normal"],
        fontName='Helvetica',
        fontSize=8,
        leading=10,
        textColor=colors.HexColor('#0F172A')
    )
    header_style = ParagraphStyle(
        'HeaderStyle',
        parent=cell_style,
        fontName='Helvetica-Bold',
        textColor=colors.whitesmoke
    )

    elements = []

    for sheet_idx, sheet in enumerate(wb.worksheets):
        if sheet_idx > 0:
            elements.append(PageBreak())

        elements.append(Paragraph(f"<b>Sheet: {sheet.title}</b>", title_style))
        elements.append(Spacer(1, 8))

        data = []
        max_r = sheet.max_row or 0
        max_c = sheet.max_column or 0

        if max_r == 0 or max_c == 0:
            elements.append(Paragraph("<i>(Empty Sheet)</i>", cell_style))
            continue

        for r in range(1, max_r + 1):
            row_data = []
            has_content = False
            for c in range(1, max_c + 1):
                val = sheet.cell(row=r, column=c).value
                val_str = "" if val is None else str(val)
                if val_str.strip():
                    has_content = True
                if r == 1:
                    row_data.append(Paragraph(f"<b>{val_str}</b>", header_style))
                else:
                    row_data.append(Paragraph(val_str, cell_style))
            if has_content or r == 1:
                data.append(row_data)

        if data:
            t = Table(data, repeatRows=1)
            t.setStyle(TableStyle([
                ('BACKGROUND', (0, 0), (-1, 0), colors.HexColor('#2563EB')),
                ('ALIGN', (0, 0), (-1, -1), 'LEFT'),
                ('VALIGN', (0, 0), (-1, -1), 'MIDDLE'),
                ('BOTTOMPADDING', (0, 0), (-1, -1), 4),
                ('TOPPADDING', (0, 0), (-1, -1), 4),
                ('LEFTPADDING', (0, 0), (-1, -1), 5),
                ('RIGHTPADDING', (0, 0), (-1, -1), 5),
                ('GRID', (0, 0), (-1, -1), 0.5, colors.HexColor('#CBD5E1')),
                ('ROWBACKGROUNDS', (0, 1), (-1, -1), [colors.white, colors.HexColor('#F8FAFC')]),
            ]))
            elements.append(t)

    doc.build(elements)
except Exception as e:
    sys.stderr.write(f"excel_to_pdf error: {e}\n")
    sys.exit(1)
"#;

    let abs_src = crate::pptx::clean_windows_path(src);
    let abs_dst = crate::pptx::clean_windows_path(output);

    cmd.args(["-c", script, &abs_src, &abs_dst]);
    cmd.stdout(std::process::Stdio::null());
    cmd.stderr(std::process::Stdio::null());

    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }

    let status = cmd.status().map_err(|e| ConvertError::ConverterError(e.to_string()))?;
    if !status.success() {
        return Err(ConvertError::ConverterError("excel_to_pdf failed".to_string()));
    }
    Ok(())
}

fn run_pdf_to_excel_converter(python_exe: &Path, src: &Path, output: &Path) -> Result<(), ConvertError> {
    let mut cmd = std::process::Command::new(python_exe);
    let script = r#"
import sys
import pdfplumber
import openpyxl

src = sys.argv[1]
dst = sys.argv[2]

try:
    wb = openpyxl.Workbook()
    wb.remove(wb.active)

    with pdfplumber.open(src) as pdf:
        for idx, page in enumerate(pdf.pages):
            sheet = wb.create_sheet(title=f"Page {idx + 1}")
            tables = page.extract_tables()

            if tables:
                for table in tables:
                    for row in table:
                        clean_row = ["" if cell is None else str(cell).strip() for cell in row]
                        sheet.append(clean_row)
                    sheet.append([])
            else:
                text = page.extract_text()
                if text:
                    for line in text.split("\n"):
                        parts = [p.strip() for p in line.split("  ") if p.strip()]
                        if parts:
                            sheet.append(parts)
                        else:
                            sheet.append([line.strip()])

    if not wb.worksheets:
        ws = wb.create_sheet(title="Sheet1")
        ws.append(["(No extractable content found)"])

    wb.save(dst)
except Exception as e:
    sys.stderr.write(f"pdf_to_excel error: {e}\n")
    sys.exit(1)
"#;

    let abs_src = crate::pptx::clean_windows_path(src);
    let abs_dst = crate::pptx::clean_windows_path(output);

    cmd.args(["-c", script, &abs_src, &abs_dst]);
    cmd.stdout(std::process::Stdio::null());
    cmd.stderr(std::process::Stdio::null());

    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }

    let status = cmd.status().map_err(|e| ConvertError::ConverterError(e.to_string()))?;
    if !status.success() {
        return Err(ConvertError::ConverterError("pdf_to_excel failed".to_string()));
    }
    Ok(())
}
