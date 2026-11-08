// Translation languages are deliberately separate from the two supported UI languages.
use super::{Language, needs_translation};

macro_rules! languages {
    ($(($variant:ident, $code:literal, $deepl:literal, $ru:literal, $en:literal)),+ $(,)?) => {
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
            pub fn name(self, ui: Language) -> &'static str {
                match self { Self::Russian => ui.text("Русский", "Russian"), $(Self::$variant => ui.text($ru, $en)),+ }
            }
        }
    };
}
languages! {
    (English, "en", "EN-US", "Английский", "English"),
    (Finnish, "fi", "FI", "Финский", "Finnish"),
    (Polish, "pl", "PL", "Польский", "Polish"),
    (German, "de", "DE", "Немецкий", "German"),
    (French, "fr", "FR", "Французский", "French"),
    (Spanish, "es", "ES", "Испанский", "Spanish"),
    (Italian, "it", "IT", "Итальянский", "Italian"),
    (Portuguese, "pt", "PT-BR", "Португальский", "Portuguese"),
    (Ukrainian, "uk", "UK", "Украинский", "Ukrainian"),
    (Dutch, "nl", "NL", "Нидерландский", "Dutch"),
    (Swedish, "sv", "SV", "Шведский", "Swedish"),
    (Danish, "da", "DA", "Датский", "Danish"),
    (Czech, "cs", "CS", "Чешский", "Czech"),
    (Turkish, "tr", "TR", "Турецкий", "Turkish"),
    (Arabic, "ar", "AR", "Арабский", "Arabic"),
    (Hindi, "hi", "HI", "Хинди", "Hindi"),
    (Japanese, "ja", "JA", "Японский", "Japanese"),
    (Korean, "ko", "KO", "Корейский", "Korean"),
    (Chinese, "zh-CN", "ZH-HANS", "Китайский", "Chinese"),
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
    /// Whether `text` contains letters of this language's script. Latin-script languages
    /// cannot be told apart by script, so they always answer true.
    pub fn has_own_script_in(self, text: &str) -> bool {
        let ranges: &[(char, char)] = match self {
            Self::Russian | Self::Ukrainian => &[('\u{0400}', '\u{052f}')],
            Self::Arabic => &[('\u{0600}', '\u{06ff}'), ('\u{0750}', '\u{077f}')],
            Self::Hindi => &[('\u{0900}', '\u{097f}')],
            Self::Japanese => &[
                ('\u{3040}', '\u{30ff}'),
                ('\u{4e00}', '\u{9fff}'),
                ('\u{ff66}', '\u{ff9f}'),
            ],
            Self::Korean => &[('\u{ac00}', '\u{d7af}'), ('\u{1100}', '\u{11ff}')],
            Self::Chinese => &[('\u{4e00}', '\u{9fff}')],
            _ => return true,
        };
        text.chars()
            .any(|c| ranges.iter().any(|(from, to)| (*from..=*to).contains(&c)))
    }
    /// Whether `text` is worth sending to a provider for translation into `self`.
    ///
    /// A line already in the target language must never go out: providers rewrite it slightly
    /// ("im at spawn" becomes "I'm at spawn") and the chat would show that as a translation.
    /// An explicit source language is trusted. With automatic detection, a line the local
    /// detector identifies as the target is skipped, and plain-ASCII text it cannot identify
    /// is treated as English when translating into English (the behaviour before language
    /// selection existed: only text with non-English markers was sent).
    pub fn should_translate(self, source: Option<TargetLanguage>, text: &str) -> bool {
        if !needs_translation(text) {
            return false;
        }
        if let Some(source) = source {
            // An explicit source is trusted, but a provider forced to read English as Russian
            // still "corrects" it ("yea i know" -> "yeah i know"). Text with none of the
            // source language's letters is not in that language, so it is left alone.
            return source != self && source.has_own_script_in(text);
        }
        match super::detect_source_language(text) {
            Some(code) => code != self.source_code(),
            None => !(self == Self::English && text.is_ascii()),
        }
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
    // Runs on the translation worker, never in a draw/input hook. It reuses the detector that
    // already labels chat lines (`detect_source_language`), so what is skipped matches the
    // badge the player sees. The filter only skips a line the detector positively identifies
    // as an uncommon language; anything it cannot identify (tiny words, typos, slang, mixed
    // lines) is translated rather than silently dropped.
    pub fn allows(self, text: &str) -> bool {
        if !needs_translation(text) {
            return false;
        }
        if !self.common_only || self.source.is_some() {
            return true;
        }
        match super::detect_source_language(text) {
            None => true,
            // BE is a Cyrillic neighbour of Russian/Ukrainian, easily mistaken for them.
            Some(code) => {
                code == "BE"
                    || TargetLanguage::COMMON
                        .iter()
                        .any(|l| l.source_code() == code)
            }
        }
    }
}
