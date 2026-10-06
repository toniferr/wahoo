//! Wahoo's types. All three are a single 32-bit integer at run time:
//! `coins` is the number itself, `switch` is 0 or 1, and `text` is the address of its bytes in linear memory.

use crate::diagnostics::Lang;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ty {
    Coins,
    Switch,
    Text,
    /// The type of an expression that already produced an error; compatible with everything so that one mistake
    /// is reported once instead of cascading.
    Error,
}

pub const TYPE_NAMES: [&str; 3] = ["coins", "switch", "text"];

impl Ty {
    pub fn from_name(name: &str) -> Option<Ty> {
        match name {
            "coins" => Some(Ty::Coins),
            "switch" => Some(Ty::Switch),
            "text" => Some(Ty::Text),
            _ => None,
        }
    }

    pub fn name(self, lang: Lang) -> String {
        match (self, lang) {
            (Ty::Coins, _) => "`coins`".into(),
            (Ty::Switch, _) => "`switch`".into(),
            (Ty::Text, _) => "`text`".into(),
            (Ty::Error, Lang::En) => "an unknown type".into(),
            (Ty::Error, Lang::Es) => "un tipo desconocido".into(),
        }
    }

    pub fn keyword(self) -> &'static str {
        match self {
            Ty::Coins => "coins",
            Ty::Switch => "switch",
            Ty::Text => "text",
            Ty::Error => "?",
        }
    }

    /// Whether a value of type `other` can go where `self` is expected.
    pub fn accepts(self, other: Ty) -> bool {
        self == other || self == Ty::Error || other == Ty::Error
    }
}
