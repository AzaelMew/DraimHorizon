use claim_borders_relay_client::translation::{Language, SourcePolicy, TargetLanguage};

#[test]
fn full_screen_controls_cover_each_language_and_migrate_old_targets() {
    use claim_borders::outgoing::{
        TranslationControl, TranslationField, TranslationScreen, TranslationTargets,
        translation_button,
    };
    let mut prefs = TranslationTargets::load(Some("fi"), Some("pl"), Language::English);
    assert_eq!(prefs.incoming, TargetLanguage::Finnish);
    assert_eq!(prefs.outgoing, TargetLanguage::Polish);
    assert_eq!(prefs.input, Some(TargetLanguage::English));
    assert!(prefs.common_only);
    prefs.input = Some(TargetLanguage::German);
    prefs.chat_source = Some(TargetLanguage::Finnish);
    assert_eq!(prefs.policy(false).google_source(), "fi");
    assert_eq!(prefs.policy(true).google_source(), "de");
    assert_eq!(TranslationTargets::decode(&prefs.encode()).unwrap(), prefs);
    assert!(TranslationTargets::decode(r#"{"schema":9}"#).is_err());
    assert!(translation_button([640.0, 360.0])[2] <= 39.0);
    for size in [[320.0, 180.0], [640.0, 360.0]] {
        for ui in [Language::English, Language::Russian] {
            let mut menu = TranslationScreen::new(prefs);
            for field in [
                TranslationField::Input,
                TranslationField::Output,
                TranslationField::Received,
                TranslationField::ChatSource,
            ] {
                menu.field = field;
                let mut languages = std::collections::BTreeSet::new();
                let mut auto = false;
                for page in 0..menu.pages(size) {
                    menu.page = page;
                    for w in menu.layout(size, ui) {
                        assert!(
                            w.rect[0] >= 0.0
                                && w.rect[1] >= 0.0
                                && w.rect[0] + w.rect[2] <= size[0]
                                && w.rect[1] + w.rect[3] <= size[1],
                            "{:?}",
                            w.rect
                        );
                        if let TranslationControl::Choose(language) = w.control {
                            match language {
                                Some(lang) => {
                                    languages.insert(lang);
                                }
                                None => auto = true,
                            }
                            let center = [w.rect[0] + w.rect[2] / 2.0, w.rect[1] + w.rect[3] / 2.0];
                            assert_eq!(menu.hit(size, ui, center), Some(w.control));
                        }
                    }
                }
                assert_eq!(languages.len(), 20);
                assert_eq!(auto, field.allows_auto());
            }
        }
    }
}

#[test]
fn first_run_after_update_reproduces_the_old_direction_not_auto() {
    use claim_borders::outgoing::TranslationTargets;
    for (ui, label, input, output) in [
        (
            Language::English,
            "EN>RU",
            TargetLanguage::English,
            TargetLanguage::Russian,
        ),
        (
            Language::Russian,
            "RU>EN",
            TargetLanguage::Russian,
            TargetLanguage::English,
        ),
    ] {
        let t = TranslationTargets::load(None, None, ui);
        assert_eq!(
            (t.label().as_str(), t.input, t.outgoing, t.incoming),
            (label, Some(input), output, input)
        );
        assert!(t.common_only);
    }
}

#[test]
fn atomic_preferences_preserve_invalid_files_and_previous_committed_values() {
    use claim_borders::outgoing::TranslationTargets;
    let dir = std::env::temp_dir().join(format!("dh-translation-prefs-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("preferences");
    let mut prefs = TranslationTargets::load(None, None, Language::English);
    prefs.save(&path).unwrap();
    let original = std::fs::read_to_string(&path).unwrap();
    prefs.input = Some(TargetLanguage::Finnish);
    prefs.outgoing = TargetLanguage::Polish;
    prefs.save(&path).unwrap();
    assert_eq!(
        TranslationTargets::decode(&std::fs::read_to_string(&path).unwrap()).unwrap(),
        prefs
    );
    let pending = path.with_extension(format!("{}.pending", std::process::id()));
    std::fs::write(&pending, "existing pending data").unwrap();
    assert!(prefs.save(&path).is_err());
    assert_eq!(
        std::fs::read_to_string(&pending).unwrap(),
        "existing pending data"
    );
    std::fs::remove_file(pending).unwrap();
    std::fs::write(&path, "invalid existing configuration").unwrap();
    assert!(prefs.save(&path).is_err());
    assert_eq!(
        std::fs::read_to_string(&path).unwrap(),
        "invalid existing configuration"
    );
    std::fs::write(&path, original).unwrap();
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn archive_retains_non_ui_translation_language() {
    use claim_borders::{chat_feed::Feed, chat_preferences::Preferences};
    let mut feed = Feed::default();
    let id = feed
        .push_with("<Alex> Hello", 1, Preferences::default())
        .unwrap();
    feed.translate(id, "Hei".into(), TargetLanguage::Finnish);
    let mut restored = Feed::default();
    restored
        .restore(&feed.archive(), Preferences::default())
        .unwrap();
    assert!(restored.entries[0].has_translation(TargetLanguage::Finnish));
    assert!(!restored.entries[0].has_translation(TargetLanguage::English));
}

#[test]
fn common_language_catalog_and_source_filter_are_independent_of_ui() {
    assert_eq!(TargetLanguage::COMMON.len(), 20);
    for code in [
        "en", "ru", "fi", "pl", "de", "fr", "es", "it", "pt", "uk", "nl", "sv", "da", "cs", "tr",
        "ar", "hi", "ja", "ko", "zh-CN",
    ] {
        let target = TargetLanguage::from_code(code).unwrap();
        assert_eq!(target.code(), code);
        assert!(!target.name(Language::English).is_empty());
        assert!(!target.name(Language::Russian).is_empty());
    }
    assert!(TargetLanguage::from_code("xx").is_none());
    let common = SourcePolicy {
        source: None,
        common_only: true,
    };
    assert!(
        common.allows("This is a complete English sentence about building a house with friends.")
    );
    assert!(
        common.allows("Tämä on suomenkielinen viesti ystäville ja haluan rakentaa suuren talon.")
    );
    assert!(!common.allows("ეს არის ქართული ტექსტი და ჩვენ ვსაუბრობთ ერთმანეთთან ქართულ ენაზე."));
    assert!(
        !common.allows("Γεια σας φίλοι, αυτή είναι μια ολόκληρη πρόταση στην ελληνική γλώσσα.")
    );
    assert!(!common.allows("123 :) !!!"));
    assert!(!common.allows("/msg Alex hello"));
    let unrestricted = SourcePolicy {
        source: None,
        common_only: false,
    };
    assert!(unrestricted.allows("Γεια σας φίλοι"));
    let explicit = SourcePolicy {
        source: Some(TargetLanguage::Finnish),
        common_only: true,
    };
    assert!(explicit.allows("hei"));
    assert_eq!(explicit.google_source(), "fi");
    assert_eq!(common.google_source(), "auto");
}

#[test]
fn chat_source_is_optional_in_saved_preferences_and_unlocks_short_words() {
    use claim_borders::outgoing::TranslationTargets;
    // A file written before the chat-source choice existed must still load as auto-detect.
    let old = r#"{"schema":1,"input":"en","output":"ru","received":"en","common_only":true}"#;
    let prefs = TranslationTargets::decode(old).unwrap();
    assert_eq!(prefs.chat_source, None);
    assert_eq!(prefs.policy(false).google_source(), "auto");
    // An explicit chat language bypasses unreliable detection of tiny messages.
    let mut prefs = prefs;
    prefs.chat_source = Some(TargetLanguage::Finnish);
    let policy = prefs.policy(false);
    assert!(policy.allows("moi"));
    assert_eq!(policy.google_source(), "fi");
    assert!(TranslationTargets::decode(&prefs.encode()).unwrap() == prefs);
    assert!(TranslationTargets::decode(r#"{"schema":1,"input":null,"output":"ru","received":"en","chat_source":"xx","common_only":true}"#).is_err());
}

#[test]
fn common_filter_never_skips_real_chat_lines_it_cannot_identify_reliably() {
    let common = SourcePolicy {
        source: None,
        common_only: true,
    };
    // Lines seen in a live chat that were left untranslated.
    for text in [
        "куда на моей территорке",
        "claim команда",
        "./claim на нужном чанке напиши в чат",
        "hi",
        "ok",
        "moi",
        "lol xd",
        "где спавн?",
    ] {
        assert!(common.allows(text), "{text}");
    }
    // A long, clearly uncommon language is still skipped.
    assert!(!common.allows("ეს არის ქართული ტექსტი და ჩვენ ვსაუბრობთ ერთმანეთთან ქართულ ენაზე."));
}
