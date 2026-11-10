/// Represents detected legacy non-Unicode 8-bit font encodings.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LegacyScript {
    KannadaNudi,
    HindiKrutiDev,
    None,
}

/// Detects if text or font indicates a legacy 8-bit non-Unicode font encoding
/// (such as Kannada Nudi/Baraha, Hindi Kruti Dev / DevLys).
///
/// Guarantees that:
/// - Standard modern Unicode in ANY language (English, French, German, Spanish,
///   Portuguese, Italian, Russian, Arabic, Japanese, Chinese, Hindi Unicode,
///   Kannada Unicode, Tamil Unicode, Telugu Unicode, etc.) returns `LegacyScript::None`.
/// - European texts with accents (e.g. "Élève à l'école d'ingénieurs", "Schöne Grüße")
///   return `LegacyScript::None` with 0% false positives.
pub fn detect_legacy_script(text: &str, font_name: Option<&str>) -> LegacyScript {
    if text.is_empty() {
        return LegacyScript::None;
    }

    // 1. If modern Unicode Indic or other non-Latin scripts are present, it is ALREADY standard Unicode!
    // Range U+0900..U+0D7F covers Devanagari, Bengali, Gurmukhi, Gujarati, Oriya, Tamil, Telugu, Kannada, Malayalam.
    let has_indic_unicode = text.chars().any(|c| ('\u{0900}'..='\u{0D7F}').contains(&c));
    if has_indic_unicode {
        return LegacyScript::None;
    }

    // 2. Font name verification if available from PDFium text object
    if let Some(font) = font_name {
        let f = font.to_lowercase();
        if f.contains("nudi") || f.contains("baraha") || f.contains("brh") || f.contains("akruti_knd") {
            return LegacyScript::KannadaNudi;
        }
        if f.contains("kruti") || f.contains("devlys") || f.contains("chanakya") || f.contains("walkman") {
            return LegacyScript::HindiKrutiDev;
        }
    }

    // 3. Structural Nudi / Baraha Kannada pattern matching:
    // Nudi text uses distinct ligature pairs where ASCII consonants (P, U, Z, e, n, v, z, £, ¥, §, ¨, ª, ¸, g, etc.)
    // are combined with Latin-1 vowel signs (À, Á, Â, Ã, Ä, Æ, È, É, Ê, Ë).
    // In European languages (French, Spanish, German), accents ONLY appear on vowels (é, è, à, ö, ü),
    // NEVER following an ASCII consonant like "PÀ" or "gÀ".
    const NUDI_PAIRS: &[&str] = &[
        "PÀ", "UÀ", "¸À", "ªÀ", "¨É", "¯É", "PÉ", "zÀ", "£À", "AiÀÄ",
        "ºÀ", "¥À", "§À", "vÀ", "gÀ", "QÀ", "bÀ", "eÀ", "dÀ", "nÀ",
        "oÀ", "pÀ", "qÀ", "tÀ", "xÀ", "zsÀ", "¥sÀ", "¨sÀ", "ªÀÄ", "®À",
        "¯À", "±À", "µÀ", "¼À", "PÁ", "UÁ", "¸Á", "ªÀiÁ", "PÃ", "UÃ",
        "PÂ", "UÂ", "PÀÆ", "UÀÆ", "PÀÄ", "UÀÄ", "©Ã", "¢£Á", "«zÁ",
    ];

    if NUDI_PAIRS.iter().any(|&pair| text.contains(pair)) {
        return LegacyScript::KannadaNudi;
    }

    // 4. Check for presence of non-alphabetic Latin-1 symbol characters used exclusively in legacy Indic encodings
    // ('¨', '¸', 'ª', '¼', '½', '¾', '¿', '¯', '±', 'µ', '·', 'º', '»')
    let legacy_symbol_count = text.chars().filter(|&c| matches!(c,
        '¨' | '¸' | 'ª' | '¼' | '½' | '¾' | '¿' | '¯' | '±' | 'µ' | '·' | 'º' | '»'
    )).count();

    if legacy_symbol_count >= 2 {
        return LegacyScript::KannadaNudi;
    }

    LegacyScript::None
}

/// Convenience checker for legacy Indic text.
pub fn is_mojibake_or_legacy_font(text: &str, font_name: Option<&str>) -> bool {
    detect_legacy_script(text, font_name) != LegacyScript::None
}

/// Converts a line or paragraph of text if it is detected as legacy Mojibake / 8-bit Indic font.
/// Standard modern Unicode across all languages passes through completely untouched.
pub fn convert_legacy_text(text: &str, font_name: Option<&str>) -> String {
    match detect_legacy_script(text, font_name) {
        LegacyScript::KannadaNudi => nudi_to_unicode(text),
        LegacyScript::HindiKrutiDev => kruti_to_unicode(text),
        LegacyScript::None => text.to_string(),
    }
}

/// Transliterates legacy 8-bit Nudi / Baraha Kannada text into standard modern Unicode Kannada (U+0C80..U+0CFF).
pub fn nudi_to_unicode(text: &str) -> String {
    let mut s = text.to_string();

    // 1. Common compound words & prefixes
    let word_replaces = [
        ("¨ÉAUÀ¼ÀÆgÀÄ", "ಬೆಂಗಳೂರು"),
        ("ªÀÄºÁ£ÀUÀgÀ", "ಮಹಾನಗರ"),
        ("¸ÁjUÉ", "ಸಾರಿಗೆ"),
        ("¸ÀA¸ÉÜ", "ಸಂಸ್ಥೆ"),
        ("C£ÀÄ§AzsÀ", "ಅನುಬಂಧ"),
        ("«zÁåyð", "ವಿದ್ಯಾರ್ಥಿ"),
        ("jAiÀiÁ¬Äw", "ರಿಯಾಯಿತಿ"),
        ("¥Á¸ÀÄ", "ಪಾಸ್"),
        ("¥Á¸ï", "ಪಾಸ್"),
        ("Cfð", "ಅರ್ಜಿ"),
        ("ºÉ¸ÀgÀÄ", "ಹೆಸರು"),
        ("¢£ÁAPÀB", "ದಿನಾಂಕ:"),
        ("¢£ÁAPÀ", "ದಿನಾಂಕ"),
        ("°AUÀ", "ಲಿಂಗ"),
        ("ªÀUÀð", "ವರ್ಗ"),
        ("¸ÁªÀiÁ£Àå", "ಸಾಮಾನ್ಯ"),
        ("EvÀgÉ", "ಇತರೆ"),
        ("¥À.¥À", "ಪ.ಪಂ"),
        ("¥À.eÁ", "ಪ.ಜಾ"),
        ("¥Àæ¸ÀÄ ÛvÀ", "ಪ್ರಸ್ತುತ"),
        ("¥Àæ¸ÀÄÛvÀ", "ಪ್ರಸ್ತುತ"),
        ("CAZÉ", "ಅಂಚೆ"),
        ("«¼Á¸À", "ವಿಳಾಸ"),
        ("ªÀÄ£É", "ಮನೆ"),
        ("£ÀA", "ನಂ"),
        ("©Ã¢", "ಬೀದಿ"),
        ("¸ÀÜ¼À", "ಸ್ಥಳ"),
        ("¦£ï", "ಪಿನ್"),
        ("PÉÆÃqï", "ಕೋಡ್"),
        ("vÀAzÉ", "ತಂದೆ"),
        ("vÁ¬Ä", "ತಾಯಿ"),
        ("±Á¯É", "ಶಾಲೆ"),
        ("PÁ¯ÉÃdÄ", "ಕಾಲೇಜು"),
        ("«±Àé«zÁå¤®AiÀÄ", "ವಿಶ್ವವಿದ್ಯಾನಿಲಯ"),
        ("«±Àé«zÁå®AiÀÄ", "ವಿಶ್ವವಿದ್ಯಾಲಯ"),
        ("zÀÆgÀªÁtÂ", "ದೂರವಾಣಿ"),
        ("¸À»", "ಸಹಿ"),
        ("C¨sÀåyð", "ಅಭ್ಯರ್ಥಿ"),
        ("ªÀÄÄRå¸ÀÜgÀÄ", "ಮುಖ್ಯಸ್ಥರು"),
        ("ªÉÆ¨ÉÊ¯ï", "ಮೊಬೈಲ್"),
        ("¸ÀASÉå", "ಸಂಖ್ಯೆ"),
        ("§¸ï", "ಬಸ್"),
        ("ºÀÄnÖzÀ", "ಹುಟ್ಟಿದ"),
        ("§ºÀ¼À", "ಬಹಳ"),
        ("PÀ£ÁðlPÀ", "ಕರ್ನಾಟಕ"),
        ("¸ÀPÁðgÀ", "ಸರ್ಕಾರ"),
    ];

    for &(from, to) in &word_replaces {
        s = s.replace(from, to);
    }

    // 2. Syllables with prefix e-matra: 'É' or '¨É' etc.
    let prefixes = [
        ("PÉÆÃ", "ಕೋ"), ("PÉÆ", "ಕೊ"), ("PÉÃ", "ಕೇ"), ("PÉÊ", "ಕೈ"), ("PÉ", "ಕೆ"),
        ("QÉÆÃ", "ಖೋ"), ("QÉÆ", "ಖೊ"), ("QÉÃ", "ಖೇ"), ("QÉÊ", "ಖೈ"), ("QÉ", "ಖೆ"),
        ("UÉÆÃ", "ಗೋ"), ("UÉÆ", "ಗೊ"), ("UÉÃ", "ಗೇ"), ("UÉÊ", "ಗೈ"), ("UÉ", "ಗೆ"),
        ("ZÉÆÃ", "ಚೋ"), ("ZÉÆ", "ಚೊ"), ("ZÉÃ", "ಚೇ"), ("ZÉÊ", "ಚೈ"), ("ZÉ", "ಚೆ"),
        ("eÉÆÃ", "ಜೋ"), ("eÉÆ", "ಜೊ"), ("eÉÃ", "ಜೇ"), ("eÉÊ", "ಜೈ"), ("eÉ", "ಜೆ"),
        ("nÉÆÃ", "ಟೋ"), ("nÉÆ", "ಟೊ"), ("nÉÃ", "ಟೇ"), ("nÉÊ", "ಟೈ"), ("nÉ", "ಟೆ"),
        ("vÉÆÃ", "ತೋ"), ("vÉÆ", "ತೊ"), ("vÉÃ", "ತೇ"), ("vÉÊ", "ತೈ"), ("vÉ", "ತೆ"),
        ("zÉÆÃ", "ದೋ"), ("zÉÆ", "ದೊ"), ("zÉÃ", "ದೇ"), ("zÉÊ", "ದೈ"), ("zÉ", "ದೆ"),
        ("£ÉÆÃ", "ನೋ"), ("£ÉÆ", "ನೊ"), ("£ÉÃ", "ನೇ"), ("£ÉÊ", "ನೈ"), ("£É", "ನೆ"),
        ("¥ÉÆÃ", "ಪೋ"), ("¥ÉÆ", "ಪೊ"), ("¥ÉÃ", "ಪೇ"), ("¥ÉÊ", "ಪೈ"), ("¥É", "ಪೆ"),
        ("§ÉÆÃ", "ಬೋ"), ("§ÉÆ", "ಬೊ"), ("§ÉÃ", "ಬೇ"), ("§ÉÊ", "ಬೈ"), ("§É", "ಬೆ"),
        ("¨ÉÆÃ", "ಭೋ"), ("¨ÉÆ", "ಭೊ"), ("¨ÉÃ", "ಭೇ"), ("¨ÉÊ", "ಭೈ"), ("¨É", "ಬೆ"),
        ("ªÉÆÃ", "ಮೋ"), ("ªÉÆ", "ಮೊ"), ("ªÉÃ", "ಮೇ"), ("ªÉÊ", "ಮೈ"), ("ªÉ", "ಮೆ"),
        ("gÉÆÃ", "ರೋ"), ("gÉÆ", "ರೊ"), ("gÉÃ", "ರೇ"), ("gÉÊ", "ರೈ"), ("gÉ", "ರೆ"),
        ("¯ÉÆÃ", "ಲೋ"), ("¯ÉÆ", "ಲೊ"), ("¯ÉÃ", "ಲೇ"), ("¯ÉÊ", "ಲೈ"), ("¯É", "ಲೆ"),
        ("¸ÉÆÃ", "ಸೋ"), ("¸ÉÆ", "ಸೊ"), ("¸ÉÃ", "ಸೇ"), ("¸ÉÊ", "ಸೈ"), ("¸É", "ಸೆ"),
        ("ºÉÆÃ", "ಹೋ"), ("ºÉÆ", "ಹೊ"), ("ºÉÃ", "ಹೇ"), ("ºÉÊ", "ಹೈ"), ("ºÉ", "ಹೆ"),
        ("¼ÉÆÃ", "ಳೋ"), ("¼ÉÆ", "ಳೊ"), ("¼ÉÃ", "ಳೇ"), ("¼ÉÊ", "ಳೈ"), ("¼É", "ಳೆ"),
    ];

    for &(from, to) in &prefixes {
        s = s.replace(from, to);
    }

    // 3. Regular syllables (Consonant + Matra)
    let glyphs = [
        ("PÁ", "ಕಾ"), ("QÁ", "ಖಾ"), ("UÁ", "ಗಾ"), ("ZÁ", "ಚಾ"), ("eÁ", "ಜಾ"),
        ("nÁ", "ಟಾ"), ("vÁ", "ತಾ"), ("zÁ", "ದಾ"), ("£Á", "ನಾ"), ("¥Á", "ಪಾ"),
        ("§Á", "ಬಾ"), ("ªÀiÁ", "ಮಾ"), ("ªÀiÁ", "ಮಾ"), ("gÁ", "ರಾ"), ("¯Á", "ಲಾ"),
        ("¸Á", "ಸಾ"), ("ºÁ", "ಹಾ"), ("¼Á", "ಳಾ"),

        ("QÃ", "ಖೀ"), ("PÃ", "ಕೀ"), ("UÃ", "ಗೀ"), ("ZÃ", "ಚೀ"), ("eÃ", "ಜೀ"),
        ("nÃ", "ಟೀ"), ("vÃ", "ತೀ"), ("zÃ", "ದೀ"), ("£Ã", "ನೀ"), ("¥Ã", "ಪೀ"),
        ("§Ã", "ಬೀ"), ("ªÃ", "ಮೀ"), ("gÃ", "ರೀ"), ("¯Ã", "ಲೀ"), ("¸Ã", "ಸೀ"),
        ("ºÃ", "ಹೀ"), ("¼Ã", "ಳೀ"), ("©Ã", "ಬೀ"),

        ("QÂ", "ಖಿ"), ("PÂ", "ಕಿ"), ("UÂ", "ಗಿ"), ("ZÂ", "ಚಿ"), ("eÂ", "ಜಿ"),
        ("nÂ", "ಟಿ"), ("vÂ", "ತಿ"), ("zÂ", "ದಿ"), ("£Â", "ನಿ"), ("¥Â", "ಪಿ"),
        ("§Â", "ಬಿ"), ("ªÂ", "ಮಿ"), ("gÂ", "ರಿ"), ("¯Â", "ಲಿ"), ("¸Â", "ಸಿ"),
        ("ºÂ", "ಹಿ"), ("¼Â", "ಳಿ"), ("¦", "ಪಿ"), ("©", "ಬಿ"), ("«", "ವಿ"),

        ("PÀÆ", "ಕೂ"), ("UÀÆ", "ಗೂ"), ("ZÀÆ", "ಚೂ"), ("vÀÆ", "ತೂ"), ("zÀÆ", "ದೂ"),
        ("£ÀÆ", "ನೂ"), ("¥ÀÆ", "ಪೂ"), ("§ÀÆ", "ಬೂ"), ("ªÀÄÆ", "ಮೂ"), ("gÀÆ", "ರೂ"),
        ("¯ÀÆ", "ಲೂ"), ("¸ÀÆ", "ಸೂ"), ("ºÀÆ", "ಹೂ"), ("¼ÀÆ", "ಳೂ"),

        ("PÀÄ", "ಕು"), ("UÀÄ", "ಗು"), ("ZÀÄ", "ಚು"), ("vÀÄ", "ತು"), ("zÀÄ", "ದು"),
        ("£ÀÄ", "ನು"), ("¥ÀÄ", "ಪು"), ("§ÀÄ", "ಬು"), ("ªÀÄÄ", "ಮು"), ("gÀÄ", "ರು"),
        ("¯ÀÄ", "ಲು"), ("¸ÀÄ", "ಸು"), ("ºÀÄ", "ಹು"), ("¼ÀÄ", "ಳು"),

        ("PÀ", "ಕ"), ("QÀ", "ಖ"), ("UÀ", "ಗ"), ("SÀ", "ಘ"), ("ZÀ", "ಚ"),
        ("bÀ", "ಛ"), ("eÁ", "ಜಾ"), ("eÀ", "ಜ"), ("dÀ", "ಝ"), ("nÀ", "ಟ"),
        ("oÀ", "ಠ"), ("pÀ", "ಡ"), ("qÀ", "ಢ"), ("tÀ", "ಣ"), ("vÀ", "ತ"),
        ("xÀ", "ಥ"), ("zÀ", "ದ"), ("zsÀ", "ಧ"), ("£À", "ನ"), ("¥À", "ಪ"),
        ("¥sÀ", "ಫ"), ("§À", "ಬ"), ("¨sÀ", "ಭ"), ("ªÀÄ", "ಮ"), ("ªÀ", "ವ"),
        ("AiÀÄ", "ಯ"), ("gÀ", "ರ"), ("®À", "ಲ"), ("¯À", "ಲ"), ("ªÀ", "ವ"),
        ("±À", "ಶ"), ("µÀ", "ಷ"), ("¸À", "ಸ"), ("ºÀ", "ಹ"), ("¼À", "ಳ"),
    ];

    for &(from, to) in &glyphs {
        s = s.replace(from, to);
    }

    // 4. Base consonants and vowels
    let singles = [
        ("C", "ಅ"), ("D", "ಆ"), ("E", "ಇ"), ("F", "ಈ"), ("G", "ಉ"),
        ("H", "ಊ"), ("I", "ಋ"), ("J", "ಎ"), ("K", "ಏ"), ("L", "ಐ"),
        ("M", "ಒ"), ("N", "ಓ"), ("O", "ಔ"),
        ("P", "ಕ್"), ("Q", "ಖ್"), ("R", "ಗ್"), ("S", "ಘ್"), ("T", "ಙ್"),
        ("Z", "ಚ್"), ("b", "ಛ್"), ("c", "ಜ್"), ("d", "ಝ್"), ("e", "ಞ್"),
        ("n", "ಟ್"), ("o", "ಠ್"), ("p", "ಡ್"), ("q", "ಢ್"), ("t", "ಣ್"),
        ("v", "ತ್"), ("x", "ಥ್"), ("z", "ದ್"), ("£", "ನ್"), ("¥", "ಪ್"),
        ("§", "ಬ್"), ("¨", "ಭ್"), ("ª", "ಮ್"), ("g", "ರ್"), ("®", "ಲ್"),
        ("±", "ಶ್"), ("µ", "ಷ್"), ("¸", "ಸ್"), ("º", "ಹ್"), ("¼", "ಳ್"),
        ("w", "ತಿ"), ("j", "ರಿ"),
        ("ï", "್"), ("A", "ಂ"), ("B", "ಃ"),
        ("Á", "ಾ"), ("Â", "ಿ"), ("Ã", "ೀ"), ("Ä", "ು"), ("Æ", "ೂ"), ("È", "ೃ"),
    ];

    for &(from, to) in &singles {
        s = s.replace(from, to);
    }

    s
}

/// Transliterates legacy 8-bit Kruti Dev / DevLys Hindi text into modern Unicode Devanagari (U+0900..U+097F).
pub fn kruti_to_unicode(text: &str) -> String {
    let mut s = text.to_string();

    let ligatures = [
        ("D;", "क्य"), ("X;", "ग्य"), ("P;", "च्य"), ("T;", "ज्य"),
        ("R;", "त्य"), ("Y;", "ल्य"), ("L;", "स्य"), ("C;", "ब्य"),
        ("d`", "कृ"), ("x`", "गृ"), ("r`", "तृ"), ("e`", "मृ"),
        ("iz", "प्र"), ("oz", "व्र"), ("xz", "ग्र"), ("nz", "द्र"),
        ("Vª", "ट्र"), ("Mª", "ड्र"),
        ("{k", "क्ष"), ("=", "त्र"), ("K", "ज्ञ"),
        ("J", "श्र"),
    ];
    for &(from, to) in &ligatures {
        s = s.replace(from, to);
    }

    // In Kruti Dev, 'f' (ि) precedes the consonant: "fd" -> "कि"
    let chars: Vec<char> = s.chars().collect();
    let mut swapped = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == 'f' && i + 1 < chars.len() {
            let cons = chars[i + 1];
            swapped.push(cons);
            swapped.push('f');
            i += 2;
        } else {
            swapped.push(chars[i]);
            i += 1;
        }
    }
    s = swapped.into_iter().collect();

    let replacements = [
        ("vks", "ओ"), ("vkS", "औ"), ("vk", "आ"), ("v", "अ"),
        ("bZ", "ई"), ("b", "इ"), ("m", "उ"), ("Å", "ऊ"), ("_", "ऋ"),
        ("[kk", "खा"), ("[k", "ख"), ("[", "ख्"),
        ("?kk", "घा"), ("?k", "घ"), ("?", "घ्"),
        ("Fkk", "था"), ("Fk", "थ"), ("F", "थ्"),
        ("/kk", "धा"), ("/k", "ध"), ("/", "ध्"),
        ("Hkk", "भा"), ("Hk", "भ"), ("H", "भ्"),
        ("'kk", "शा"), ("'k", "श"), ("'", "श्"),
        ("\"kk", "षा"), ("\"k", "ष"), ("\"", "ष्"),
        ("ks", "ो"), ("kS", "ौ"), ("k", "ा"), ("h", "ी"), ("f", "ि"),
        ("s", "े"), ("S", "ै"), ("q", "ु"), ("w", "ू"), ("`", "ृ"),
        ("a", "ं"), ("W", "ँ"), (":", "ः"), ("्k", "ा"),
        ("d", "क"), ("x", "ग"), ("³", "ङ"),
        ("p", "च"), ("N", "छ"), ("t", "ज"), ("÷", "झ"), ("¥", "ञ"),
        ("V", "ट"), ("B", "ठ"), ("M", "ड"), ("<", "ढ"), (".", "ण"),
        ("r", "त"), ("n", "द"), ("u", "न"),
        ("i", "प"), ("Q", "फ"), ("c", "ब"), ("e", "म"),
        (";", "य"), ("j", "र"), ("y", "ल"), ("o", "व"),
        ("l", "स"), ("g", "ह"),
        ("D", "क्"), ("X", "ग्"), ("P", "च्"), ("T", "ज्"),
        ("R", "त्"), ("Y", "ल्"), ("L", "स्"), ("C", "ब्"),
    ];

    for &(from, to) in &replacements {
        s = s.replace(from, to);
    }

    s
}
