use std::cmp::Ordering;
use std::collections::HashSet;
use std::path::{Path, PathBuf};

/// Compares two strings using natural numerical ordering.
/// Numbers within strings are compared by their numerical value rather than ASCII code.
/// (e.g. "image_2.png" < "image_10.png" < "image_16.png").
pub fn natural_cmp(a: &str, b: &str) -> Ordering {
    let mut a_chars = a.chars().peekable();
    let mut b_chars = b.chars().peekable();

    loop {
        match (a_chars.peek(), b_chars.peek()) {
            (None, None) => return Ordering::Equal,
            (None, Some(_)) => return Ordering::Less,
            (Some(_), None) => return Ordering::Greater,
            (Some(ca), Some(cb)) if ca.is_ascii_digit() && cb.is_ascii_digit() => {
                let mut num_a: u64 = 0;
                while let Some(c) = a_chars.peek() {
                    if let Some(digit) = c.to_digit(10) {
                        num_a = num_a.saturating_mul(10).saturating_add(digit as u64);
                        a_chars.next();
                    } else {
                        break;
                    }
                }
                let mut num_b: u64 = 0;
                while let Some(c) = b_chars.peek() {
                    if let Some(digit) = c.to_digit(10) {
                        num_b = num_b.saturating_mul(10).saturating_add(digit as u64);
                        b_chars.next();
                    } else {
                        break;
                    }
                }
                match num_a.cmp(&num_b) {
                    Ordering::Equal => continue,
                    other => return other,
                }
            }
            (Some(_), Some(_)) => {
                let ca = a_chars.next().unwrap();
                let cb = b_chars.next().unwrap();
                let ca_lower = ca.to_lowercase().next().unwrap_or(ca);
                let cb_lower = cb.to_lowercase().next().unwrap_or(cb);
                match ca_lower.cmp(&cb_lower) {
                    Ordering::Equal => continue,
                    other => return other,
                }
            }
        }
    }
}

/// Compares two file paths naturally based on their file names.
pub fn natural_path_cmp(a: &Path, b: &Path) -> Ordering {
    let a_name = a.file_name().unwrap_or_default().to_string_lossy();
    let b_name = b.file_name().unwrap_or_default().to_string_lossy();
    natural_cmp(&a_name, &b_name)
}

/// Tracks the user's chronological selection order in Windows Explorer
/// and remedies Windows Shell's drag-anchor reordering.
#[derive(Debug, Clone, Default)]
pub struct SelectionOrderTracker {
    ordered_files: Vec<PathBuf>,
}

impl SelectionOrderTracker {
    pub fn new() -> Self {
        Self {
            ordered_files: Vec::new(),
        }
    }

    /// Updates the tracker with the current selection snapshot from Windows Explorer.
    ///
    /// - If current selection is empty, clears the tracker.
    /// - Preserves the user's existing click sequence for files that remain selected.
    /// - When a single file is added (e.g. Ctrl+click on 5, then 8, then 1), it is appended
    ///   at the end of the order, capturing the exact user selection sequence.
    /// - When multiple files are added at once (e.g. Ctrl+A / Alt+A or box select),
    ///   they are sorted in natural alphanumeric order ("left to right in order") and appended.
    pub fn update_selection(&mut self, current_selection: &[PathBuf]) {
        if current_selection.is_empty() {
            self.ordered_files.clear();
            return;
        }

        let current_set: HashSet<&PathBuf> = current_selection.iter().collect();

        // 1. Keep already ordered files that remain in the current selection
        let mut new_ordered: Vec<PathBuf> = self
            .ordered_files
            .iter()
            .filter(|p| current_set.contains(p))
            .cloned()
            .collect();

        // 2. Extract newly selected files
        let ordered_set: HashSet<&PathBuf> = new_ordered.iter().collect();
        let mut newly_added: Vec<PathBuf> = current_selection
            .iter()
            .filter(|p| !ordered_set.contains(p))
            .cloned()
            .collect();

        if newly_added.len() == 1 {
            // User individually clicked a new file -> preserve chronological selection sequence
            new_ordered.push(newly_added.remove(0));
            self.ordered_files = new_ordered;
        } else if newly_added.len() > 1 {
            // Bulk selection (e.g. Ctrl+A / Alt+A, Shift-click, or box select)
            // The user intends all selected files in natural visual reading order (1, 2, ... 16)
            let mut all_sorted = current_selection.to_vec();
            all_sorted.sort_by(|a, b| natural_path_cmp(a, b));
            self.ordered_files = all_sorted;
        } else {
            // Deselection occurred
            self.ordered_files = new_ordered;
        }
    }

    /// Reorders a list of files (such as from OLE drag-drop CF_HDROP) according to:
    /// 1. The tracked user click sequence (if the files were selected individually).
    /// 2. The natural visual order (if selected via Ctrl+A or dragged without prior history).
    ///
    /// This fixes the Windows OLE issue where the file under the mouse cursor is
    /// arbitrarily forced to index 0.
    pub fn order_files(&self, files: &[PathBuf]) -> Vec<PathBuf> {
        if files.len() <= 1 {
            return files.to_vec();
        }

        let input_set: HashSet<&PathBuf> = files.iter().collect();

        // Check if our tracked order has these files
        let mut ordered: Vec<PathBuf> = self
            .ordered_files
            .iter()
            .filter(|p| input_set.contains(p))
            .cloned()
            .collect();

        let ordered_set: HashSet<&PathBuf> = ordered.iter().collect();
        let mut remaining: Vec<PathBuf> = files
            .iter()
            .filter(|p| !ordered_set.contains(p))
            .cloned()
            .collect();

        if !ordered.is_empty() {
            // If any files were not in the tracked history, sort them naturally and append
            remaining.sort_by(|a, b| natural_path_cmp(a, b));
            ordered.extend(remaining);
            ordered
        } else {
            // Fallback: sort all files naturally so the dragged anchor doesn't corrupt position 0
            let mut result = files.to_vec();
            result.sort_by(|a, b| natural_path_cmp(a, b));
            result
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_natural_sorting_order() {
        let mut files = vec![
            PathBuf::from("Student Portfolio_image_16.png"),
            PathBuf::from("Student Portfolio_image_1.png"),
            PathBuf::from("Student Portfolio_image_2.png"),
            PathBuf::from("Student Portfolio_image_10.png"),
            PathBuf::from("Student Portfolio_image_9.png"),
        ];

        files.sort_by(|a, b| natural_path_cmp(a, b));

        let names: Vec<String> = files
            .iter()
            .map(|f| f.file_name().unwrap().to_string_lossy().to_string())
            .collect();

        assert_eq!(
            names,
            vec![
                "Student Portfolio_image_1.png",
                "Student Portfolio_image_2.png",
                "Student Portfolio_image_9.png",
                "Student Portfolio_image_10.png",
                "Student Portfolio_image_16.png",
            ]
        );
    }

    #[test]
    fn test_ctrl_a_select_all_preserves_left_to_right_order() {
        let mut tracker = SelectionOrderTracker::new();

        // Windows Explorer snapshot when user presses Alt+A / Ctrl+A:
        let all_files: Vec<PathBuf> = (1..=16)
            .map(|i| PathBuf::from(format!("Student Portfolio_image_{}.png", i)))
            .collect();

        tracker.update_selection(&all_files);

        // Windows OLE Drag places the clicked anchor (image 16) at index 0:
        let mut dropped_files = vec![PathBuf::from("Student Portfolio_image_16.png")];
        for i in 1..=15 {
            dropped_files.push(PathBuf::from(format!("Student Portfolio_image_{}.png", i)));
        }

        let ordered = tracker.order_files(&dropped_files);

        // Verify image 16 is at the LAST position, and image 1 is first:
        assert_eq!(
            ordered.first().unwrap().file_name().unwrap(),
            "Student Portfolio_image_1.png"
        );
        assert_eq!(
            ordered.last().unwrap().file_name().unwrap(),
            "Student Portfolio_image_16.png"
        );
        assert_eq!(ordered.len(), 16);
    }

    #[test]
    fn test_ctrl_a_after_single_selection_resets_to_natural_order() {
        let mut tracker = SelectionOrderTracker::new();

        // Previously, image 16 was selected
        tracker.update_selection(&[PathBuf::from("Student Portfolio_image_16.png")]);

        // User hits Ctrl+A -> all 16 files are now selected:
        let all_files: Vec<PathBuf> = (1..=16)
            .map(|i| PathBuf::from(format!("Student Portfolio_image_{}.png", i)))
            .collect();
        tracker.update_selection(&all_files);

        // Dropped files with anchor on 16:
        let mut dropped_files = vec![PathBuf::from("Student Portfolio_image_16.png")];
        for i in 1..=15 {
            dropped_files.push(PathBuf::from(format!("Student Portfolio_image_{}.png", i)));
        }

        let ordered = tracker.order_files(&dropped_files);

        // Verify image 16 is at the LAST position (index 15), not kept at index 0:
        assert_eq!(
            ordered.first().unwrap().file_name().unwrap(),
            "Student Portfolio_image_1.png"
        );
        assert_eq!(
            ordered.last().unwrap().file_name().unwrap(),
            "Student Portfolio_image_16.png"
        );
        assert_eq!(ordered[15].file_name().unwrap(), "Student Portfolio_image_16.png");
    }

    #[test]
    fn test_individual_selection_order_preserved() {
        let mut tracker = SelectionOrderTracker::new();

        let f5 = PathBuf::from("Student Portfolio_image_5.png");
        let f8 = PathBuf::from("Student Portfolio_image_8.png");
        let f1 = PathBuf::from("Student Portfolio_image_1.png");

        // 1. User clicks image 5
        tracker.update_selection(&[f5.clone()]);

        // 2. User Ctrl+clicks image 8
        tracker.update_selection(&[f5.clone(), f8.clone()]);

        // 3. User Ctrl+clicks image 1
        tracker.update_selection(&[f1.clone(), f5.clone(), f8.clone()]);

        // User drags from image 1. Windows Explorer puts image 1 first, then 5, 8:
        let dropped_from_explorer = vec![f1.clone(), f5.clone(), f8.clone()];

        let ordered = tracker.order_files(&dropped_from_explorer);

        // Verify order is exactly 5 -> 8 -> 1:
        assert_eq!(ordered, vec![f5, f8, f1]);
    }
}
