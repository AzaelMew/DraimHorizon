// Translation languages are deliberately separate from the two supported UI languages.
use super::{Language, needs_translation};

macro_rules! languages {
    ($(($variant:ident, $code:literal, $deepl:literal, $detector:literal, $ru:literal, $en:literal)),+ $(,)?) => {
        #[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub enum TargetLanguage { #[default] Russian, $($variant),+ }
        impl TargetLanguage {
            pub const COMMON: [Self; 20] = [Self::English, Self::Russian, Self::Finnish, Self::Polish,
                Self::German, Self::French, Self::Spanish, Self::Italian, Self::Portuguese, Self::Ukrainian,
                Self::Dutch, Self::Swedish, Self::Danish, Self::Czech, Self::Turkish, Self::Arabic,
                Self::Hindi, Self::Japanese, Self::Korean, Self::Chinese];
            pub fn from_code(code: &str) -> Option<Self> {
                if code.eq_ignore_ascii_case("ru") { return Some(Self::Russian); }
                $(if code.eq_ignore_ascii_case($code) { return Some(Self::$variant); })+
                if code.eq_ignore_ascii_case("zh") { return Some(Self::Chinese); }
                None
            }
            pub fn code(self) -> &'static str {
                match self { Self::Russian => "ru", $(Self::$variant => $code),+ }
            }
            pub fn deepl_code(self) -> &'static str {
                match self { Self::Russian => "RU", $(Self::$variant => $deepl),+ }
            }
            pub fn detector_code(self) -> &'static str {
                match self { Self::Russian => "rus", $(Self::$variant => $detector),+ }
            }
            pub fn name(self, ui: Language) -> &'static str {
                match self { Self::Russian => ui.text("Русский", "Russian"), $(Self::$variant => ui.text($ru, $en)),+ }
            }
        }
    };
}
languages! {
    (English, "en", "EN-US", "eng", "Английский", "English"),
    (Finnish, "fi", "FI", "fin", "Финский", "Finnish"),
    (Polish, "pl", "PL", "pol", "Польский", "Polish"),
    (German, "de", "DE", "deu", "Немецкий", "German"),
    (French, "fr", "FR", "fra", "Французский", "French"),
    (Spanish, "es", "ES", "spa", "Испанский", "Spanish"),
    (Italian, "it", "IT", "ita", "Итальянский", "Italian"),
    (Portuguese, "pt", "PT-BR", "por", "Португальский", "Portuguese"),
    (Ukrainian, "uk", "UK", "ukr", "Украинский", "Ukrainian"),
    (Dutch, "nl", "NL", "nld", "Нидерландский", "Dutch"),
    (Swedish, "sv", "SV", "swe", "Шведский", "Swedish"),
    (Danish, "da", "DA", "dan", "Датский", "Danish"),
    (Czech, "cs", "CS", "ces", "Чешский", "Czech"),
    (Turkish, "tr", "TR", "tur", "Турецкий", "Turkish"),
    (Arabic, "ar", "AR", "ara", "Арабский", "Arabic"),
    (Hindi, "hi", "HI", "hin", "Хинди", "Hindi"),
    (Japanese, "ja", "JA", "jpn", "Японский", "Japanese"),
    (Korean, "ko", "KO", "kor", "Корейский", "Korean"),
    (Chinese, "zh-CN", "ZH-HANS", "cmn", "Китайский", "Chinese"),
}
impl From<Language> for TargetLanguage {
    fn from(language: Language) -> Self {
        match language {
            Language::Russian => Self::Russian,
            Language::English => Self::English,
        }
    }
}
impl TargetLanguage {
    pub fn source_code(self) -> String {
        self.code()
            .split('-')
            .next()
            .unwrap_or(self.code())
            .to_ascii_uppercase()
    }
    pub fn tag(self) -> String {
        self.code()
            .split('-')
            .next()
            .unwrap_or(self.code())
            .to_ascii_uppercase()
    }
    pub fn needs_translation(self, text: &str) -> bool {
        needs_translation(text)
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SourcePolicy {
    pub source: Option<TargetLanguage>,
    pub common_only: bool,
}
impl SourcePolicy {
    pub fn google_source(self) -> &'static str {
        self.source.map_or("auto", TargetLanguage::code)
    }
    // Runs on the translation worker, never in a draw/input hook. Unknown/unlisted
    // sources fail closed before any provider request. Identification of short/mixed text
    // is probabilistic; explicit input selection bypasses auto-identification.
    pub fn allows(self, text: &str) -> bool {
        needs_translation(text)
            && (!self.common_only
                || self.source.is_some()
                || super::detector::detect(text).is_some_and(|info| {
                    TargetLanguage::COMMON
                        .iter()
                        .any(|lang| lang.detector_code() == info.lang().code())
                }))
    }
}
