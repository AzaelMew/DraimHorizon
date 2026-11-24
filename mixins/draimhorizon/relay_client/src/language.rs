//! The interface language, Russian or English. Pure, so the code that never touches the
//! network can share it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Language {
    #[default]
    Russian,
    English,
}
impl Language {
    pub fn from_code(code: &str) -> Self {
        if code == "en" {
            Self::English
        } else {
            Self::Russian
        }
    }
    pub fn code(self) -> &'static str {
        self.text("ru", "en")
    }
    pub fn other(self) -> Self {
        match self {
            Self::Russian => Self::English,
            Self::English => Self::Russian,
        }
    }
    pub fn text<'a>(self, russian: &'a str, english: &'a str) -> &'a str {
        match self {
            Self::Russian => russian,
            Self::English => english,
        }
    }
}
