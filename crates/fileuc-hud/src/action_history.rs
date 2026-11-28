//! Lightweight persistent action frequency tracker for FileUC.
//! Records usage frequency per file extension (e.g. "png" -> ToWebp: 15, "pdf" -> LockPdf: 8)
//! to intelligently order chips and provide user-personalized search results.

use fileuc_types::ConversionAction;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

#[derive(Debug, Clone, Default)]
pub struct ActionHistoryTracker {
    // Map: file extension (lowercase) -> Map: action -> frequency count
    history: HashMap<String, HashMap<ConversionAction, u32>>,
    storage_path: PathBuf,
}

impl ActionHistoryTracker {
    pub fn new() -> Self {
        let storage_path = get_default_history_path();
        let history = load_history_from_disk(&storage_path);
        Self {
            history,
            storage_path,
        }
    }

    /// Records that the user executed `action` on a file with `extension`.
    pub fn record(&mut self, extension: &str, action: ConversionAction) {
        let ext = extension.trim().trim_start_matches('.').to_lowercase();
        if ext.is_empty() {
            return;
        }

        let entry = self.history.entry(ext).or_default();
        let count = entry.entry(action).or_insert(0);
        *count = count.saturating_add(1);

        self.save_to_disk();
    }

    /// Returns the historical usage count for a given extension and action.
    pub fn get_count(&self, extension: &str, action: ConversionAction) -> u32 {
        let ext = extension.trim().trim_start_matches('.').to_lowercase();
        self.history
            .get(&ext)
            .and_then(|map| map.get(&action))
            .copied()
            .unwrap_or(0)
    }

    /// Sorts a slice of actions based on historical frequency for `extension` descending.
    /// Preserves original relative order for actions with equal frequency.
    pub fn sort_actions(&self, extension: &str, actions: &mut [ConversionAction]) {
        let ext = extension.trim().trim_start_matches('.').to_lowercase();
        if let Some(map) = self.history.get(&ext) {
            let mut with_meta: Vec<(usize, ConversionAction, u32)> = actions
                .iter()
                .enumerate()
                .map(|(idx, &a)| (idx, a, map.get(&a).copied().unwrap_or(0)))
                .collect();

            // Sort by count descending, then original index ascending
            with_meta.sort_by(|a, b| b.2.cmp(&a.2).then_with(|| a.0.cmp(&b.0)));

            for (i, item) in with_meta.into_iter().enumerate() {
                actions[i] = item.1;
            }
        }
    }

    fn save_to_disk(&self) {
        if let Some(parent) = self.storage_path.parent() {
            let _ = fs::create_dir_all(parent);
        }

        // Serialize: ext -> action_name -> count
        let mut serializable: HashMap<&str, HashMap<String, u32>> = HashMap::new();
        for (ext, actions) in &self.history {
            let mut action_map = HashMap::new();
            for (action, count) in actions {
                action_map.insert(format!("{:?}", action), *count);
            }
            serializable.insert(ext.as_str(), action_map);
        }

        if let Ok(json) = serde_json::to_string_pretty(&serializable) {
            let tmp_path = self.storage_path.with_extension(format!("tmp.{}", std::process::id()));
            if fs::write(&tmp_path, json.as_bytes()).is_ok() {
                if fs::rename(&tmp_path, &self.storage_path).is_err() {
                    let _ = fs::remove_file(&self.storage_path);
                    let _ = fs::rename(&tmp_path, &self.storage_path);
                }
            }
        }
    }
}

fn get_default_history_path() -> PathBuf {
    if let Ok(local_app_data) = std::env::var("LOCALAPPDATA") {
        PathBuf::from(local_app_data).join("FileUC").join("action_history.json")
    } else if let Ok(user_profile) = std::env::var("USERPROFILE") {
        PathBuf::from(user_profile).join(".fileuc").join("action_history.json")
    } else {
        PathBuf::from("action_history.json")
    }
}

fn load_history_from_disk(path: &PathBuf) -> HashMap<String, HashMap<ConversionAction, u32>> {
    let mut history = HashMap::new();
    if let Ok(data) = fs::read_to_string(path) {
        if let Ok(deserialized) = serde_json::from_str::<HashMap<String, HashMap<String, u32>>>(&data) {
            for (ext, action_map) in deserialized {
                let mut map = HashMap::new();
                for (name, count) in action_map {
                    if let Some(action) = parse_action_from_name(&name) {
                        map.insert(action, count);
                    }
                }
                history.insert(ext, map);
            }
        }
    }
    history
}

fn parse_action_from_name(name: &str) -> Option<ConversionAction> {
    use ConversionAction::*;
    match name {
        "ToPng" => Some(ToPng),
        "ToJpg" => Some(ToJpg),
        "ToJpeg" => Some(ToJpeg),
        "ToWebp" => Some(ToWebp),
        "ToSvg" => Some(ToSvg),
        "ToIco" => Some(ToIco),
        "CreateIconPack" => Some(CreateIconPack),
        "ToBmp" => Some(ToBmp),
        "ToTiff" => Some(ToTiff),
        "ToPdf" => Some(ToPdf),
        "DocToPdf" => Some(DocToPdf),
        "PptxToPdf" => Some(PptxToPdf),
        "MdToPdf" => Some(MdToPdf),
        "TxtToPdf" => Some(TxtToPdf),
        "HtmlToPdf" => Some(HtmlToPdf),
        "ExcelToPdf" => Some(ExcelToPdf),
        "PdfToDocx" => Some(PdfToDocx),
        "PdfToExcel" => Some(PdfToExcel),
        "PdfOcr" => Some(PdfOcr),
        "SplitPdf" => Some(SplitPdf),
        "MergePdf" => Some(MergePdf),
        "CompressPdf" => Some(CompressPdf),
        "CompressVideo" => Some(CompressVideo),
        "LockPdf" => Some(LockPdf),
        "UnlockPdf" => Some(UnlockPdf),
        "PdfToImages" => Some(PdfToImages),
        "ReduceSize" => Some(ReduceSize),
        "BulkRename" => Some(BulkRename),
        "Share" => Some(Share),
        "CreateZip" => Some(CreateZip),
        "Create7z" => Some(Create7z),
        "OneZip" => Some(OneZip),
        "ExtractArchive" => Some(ExtractArchive),
        "VideoToGif" => Some(VideoToGif),
        "VideoToMp3" => Some(VideoToMp3),
        "ToMp3" => Some(ToMp3),
        "ToWav" => Some(ToWav),
        "ToOgg" => Some(ToOgg),
        "ToAac" => Some(ToAac),
        "ToFlac" => Some(ToFlac),
        "ToHtml" => Some(ToHtml),
        "MdToTxt" => Some(MdToTxt),
        "TxtToMd" => Some(TxtToMd),
        "XlsxToCsv" => Some(XlsxToCsv),
        "XlsxToJson" => Some(XlsxToJson),
        "CsvToXlsx" => Some(CsvToXlsx),
        "CsvToJson" => Some(CsvToJson),
        "JsonToCsv" => Some(JsonToCsv),
        "JsonToXlsx" => Some(JsonToXlsx),
        "StripMetadata" => Some(StripMetadata),
        "Resize" => Some(Resize),
        "ToGrayscale" => Some(ToGrayscale),
        _ => None,
    }
}

static GLOBAL_TRACKER: OnceLock<Mutex<ActionHistoryTracker>> = OnceLock::new();

pub fn get_global_history() -> &'static Mutex<ActionHistoryTracker> {
    GLOBAL_TRACKER.get_or_init(|| Mutex::new(ActionHistoryTracker::new()))
}

pub fn record_action_usage(extension: &str, action: ConversionAction) {
    if matches!(
        action,
        ConversionAction::ReceiveFiles
            | ConversionAction::SendFiles
            | ConversionAction::QuickQr
    ) {
        return;
    }
    if let Ok(mut tracker) = get_global_history().lock() {
        tracker.record(extension, action);
    }
}

pub fn sort_actions_by_frequency(extension: &str, actions: &mut [ConversionAction]) {
    if let Ok(tracker) = get_global_history().lock() {
        tracker.sort_actions(extension, actions);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_record_and_sort() {
        let mut tracker = ActionHistoryTracker {
            history: HashMap::new(),
            storage_path: PathBuf::from("target/test_history.json"),
        };

        tracker.record("png", ConversionAction::ToWebp);
        tracker.record("png", ConversionAction::ToWebp);
        tracker.record("png", ConversionAction::ToJpg);

        assert_eq!(tracker.get_count("png", ConversionAction::ToWebp), 2);
        assert_eq!(tracker.get_count("png", ConversionAction::ToJpg), 1);
        assert_eq!(tracker.get_count("png", ConversionAction::ToSvg), 0);

        let mut actions = vec![
            ConversionAction::ToSvg,
            ConversionAction::ToJpg,
            ConversionAction::ToWebp,
        ];
        tracker.sort_actions("png", &mut actions);

        assert_eq!(actions[0], ConversionAction::ToWebp);
        assert_eq!(actions[1], ConversionAction::ToJpg);
        assert_eq!(actions[2], ConversionAction::ToSvg);

        let _ = fs::remove_file("target/test_history.json");
    }
}
