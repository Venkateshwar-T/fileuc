use fileuc_convert::indic::{convert_legacy_text, is_mojibake_or_legacy_font, LegacyScript, detect_legacy_script};

#[test]
fn test_indic_kannada_pdf_extraction() {
    let path = std::path::Path::new(r"C:\Users\Sharad Chand\Downloads\StudentbuspassApplication.pdf");
    if path.exists() {
        let paras = fileuc_convert::pdf::extract_pdf_paragraphs(path).expect("Failed to extract");
        println!("Total paragraphs extracted: {}", paras.len());
        for (i, p) in paras.iter().enumerate().skip(25).take(15) {
            println!("Converted Para {}: {}", i, p);
        }

        // Verify that Bangalore Metropolitan Transport Corporation is properly transliterated into Kannada
        let full_text = paras.join("\n");
        assert!(
            full_text.contains("ಬೆಂಗಳೂರು") || full_text.contains("ಮಹಾನಗರ") || full_text.contains("ಸಾರಿಗೆ"),
            "Expected Kannada text not found in extracted output!"
        );
    }
}

#[test]
fn test_european_and_unicode_languages_are_never_modified() {
    // 1. French with accents
    let french = "Élève à l'école d'ingénieurs et français";
    assert_eq!(detect_legacy_script(french, None), LegacyScript::None);
    assert!(!is_mojibake_or_legacy_font(french, None));
    assert_eq!(convert_legacy_text(french, None), french);

    // 2. German with umlauts
    let german = "Schöne Grüße aus München, Köln und Österreich!";
    assert_eq!(detect_legacy_script(german, None), LegacyScript::None);
    assert!(!is_mojibake_or_legacy_font(german, None));
    assert_eq!(convert_legacy_text(german, None), german);

    // 3. Spanish with tildes and inverted punctuation
    let spanish = "El niño nació en el año 2000 en España";
    assert_eq!(detect_legacy_script(spanish, None), LegacyScript::None);
    assert!(!is_mojibake_or_legacy_font(spanish, None));
    assert_eq!(convert_legacy_text(spanish, None), spanish);

    // 4. Modern Unicode Hindi (already Unicode)
    let hindi_unicode = "यह एक छात्र आवेदन पत्र है।";
    assert_eq!(detect_legacy_script(hindi_unicode, None), LegacyScript::None);
    assert!(!is_mojibake_or_legacy_font(hindi_unicode, None));
    assert_eq!(convert_legacy_text(hindi_unicode, None), hindi_unicode);

    // 5. Modern Unicode Kannada (already Unicode)
    let kannada_unicode = "ಇದು ವಿದ್ಯಾರ್ಥಿ ಬಸ್ ಪಾಸ್ ಅರ್ಜಿ ನಮೂನೆ.";
    assert_eq!(detect_legacy_script(kannada_unicode, None), LegacyScript::None);
    assert!(!is_mojibake_or_legacy_font(kannada_unicode, None));
    assert_eq!(convert_legacy_text(kannada_unicode, None), kannada_unicode);

    // 6. English
    let english = "Application Form for Student Concession Pass 2026-27";
    assert_eq!(detect_legacy_script(english, None), LegacyScript::None);
    assert!(!is_mojibake_or_legacy_font(english, None));
    assert_eq!(convert_legacy_text(english, None), english);
}

#[test]
fn test_legacy_scripts_detected_and_converted() {
    // Legacy Kannada Nudi
    let nudi = "¨ÉAUÀ¼ÀÆgÀÄ ªÀÄºÁ£ÀUÀgÀ ¸ÁjUÉ ¸ÀA¸ÉÜ";
    assert_eq!(detect_legacy_script(nudi, None), LegacyScript::KannadaNudi);
    assert!(is_mojibake_or_legacy_font(nudi, None));
    let converted_knd = convert_legacy_text(nudi, None);
    assert_eq!(converted_knd, "ಬೆಂಗಳೂರು ಮಹಾನಗರ ಸಾರಿಗೆ ಸಂಸ್ಥೆ");

    // Legacy Hindi Kruti Dev
    let kruti = "Hkkjr ljdkj";
    assert_eq!(detect_legacy_script(kruti, Some("KrutiDev010")), LegacyScript::HindiKrutiDev);
    assert!(is_mojibake_or_legacy_font(kruti, Some("KrutiDev010")));
    let converted_hi = convert_legacy_text(kruti, Some("KrutiDev010"));
    assert_eq!(converted_hi, "भारत सरकार");
}
