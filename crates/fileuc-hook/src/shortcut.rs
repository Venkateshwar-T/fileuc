use crate::gesture::Modifiers;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ShortcutCombo {
    pub mods: Modifiers,
    pub vk: u32,
}

/// Parses a shortcut string such as "Ctrl+Shift+Q", "Alt+Q", "Ctrl+Alt+K", etc.
pub fn parse_shortcut(s: &str) -> Option<ShortcutCombo> {
    let mut mods = 0u8;
    let mut vk: Option<u32> = None;

    for part in s.split('+') {
        let trimmed = part.trim();
        if trimmed.is_empty() {
            continue;
        }
        let lower = trimmed.to_lowercase();
        match lower.as_str() {
            "ctrl" | "control" => mods |= Modifiers::CTRL.0,
            "alt" | "menu" => mods |= Modifiers::ALT.0,
            "shift" => mods |= Modifiers::SHIFT.0,
            "space" => vk = Some(0x20),
            "enter" | "return" => vk = Some(0x0D),
            "tab" => vk = Some(0x09),
            "esc" | "escape" => vk = Some(0x1B),
            s if s.starts_with('f') && s.len() > 1 => {
                if let Ok(num) = s[1..].parse::<u32>() {
                    if (1..=12).contains(&num) {
                        vk = Some(0x70 + (num - 1));
                    }
                }
            }
            _ => {
                if trimmed.len() == 1 {
                    let ch = trimmed.chars().next().unwrap().to_ascii_uppercase();
                    if ch.is_ascii_alphanumeric() {
                        vk = Some(ch as u32);
                    }
                }
            }
        }
    }

    vk.and_then(|v| {
        // Safety guard: plain alphanumeric keys MUST have at least one modifier (Ctrl, Alt, or Shift).
        // Only standalone Function keys (F1-F12) are allowed without modifiers.
        let is_f_key = (0x70..=0x7B).contains(&v);
        if mods == 0 && !is_f_key {
            None
        } else {
            Some(ShortcutCombo {
                mods: Modifiers(mods),
                vk: v,
            })
        }
    })
}
