//! Shared persistence envelope. Untagged v1/v2/v3 records are the historic Spider format.
use crate::{DealId, Game, Mode, klondike, tripeaks};
use serde::{Deserialize, Deserializer, Serialize, Serializer, de::Error};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Spider,
    Klondike,
    Tripeaks,
}
impl Kind {
    pub fn name(self) -> &'static str {
        match self {
            Self::Spider => "Паук",
            Self::Klondike => "Косынка",
            Self::Tripeaks => "Три пика",
        }
    }
    pub fn regular_slot(self) -> &'static str {
        match self {
            Self::Spider => "regular",
            Self::Klondike => "klondike_regular",
            Self::Tripeaks => "tripeaks_regular",
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Session {
    Spider(Game),
    Klondike(klondike::Game),
    Tripeaks(tripeaks::Game),
}
impl From<Game> for Session {
    fn from(g: Game) -> Self {
        Self::Spider(g)
    }
}
impl From<klondike::Game> for Session {
    fn from(g: klondike::Game) -> Self {
        Self::Klondike(g)
    }
}
impl Serialize for Session {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let mut value = match self {
            Self::Spider(g) => serde_json::to_value(g),
            Self::Klondike(g) => serde_json::to_value(g),
            Self::Tripeaks(g) => serde_json::to_value(g),
        }
        .map_err(serde::ser::Error::custom)?;
        value["kind"] = match self.kind() {
            Kind::Spider => "spider",
            Kind::Klondike => "klondike",
            Kind::Tripeaks => "tripeaks",
        }
        .into();
        if self.kind() == Kind::Spider {
            value["variant"] = "spider".into();
        }
        value.serialize(s)
    }
}
impl<'de> Deserialize<'de> for Session {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let v = serde_json::Value::deserialize(d)?;
        Self::from_value(v).map_err(D::Error::custom)
    }
}
impl Session {
    fn from_value(value: serde_json::Value) -> Result<Self, &'static str> {
        let text = value.to_string();
        match value.get("kind").and_then(|v| v.as_str()) {
            None if value.get("kind").is_none() && value.get("variant").is_none() => {
                Game::restore(&text).map(Self::Spider)
            }
            Some("spider") if value.get("variant").is_none_or(|v| v == "spider") => {
                Game::restore(&text).map(Self::Spider)
            }
            Some("klondike") => klondike::Game::restore(&text).map(Self::Klondike),
            Some("tripeaks") => tripeaks::Game::restore(&text).map(Self::Tripeaks),
            _ => Err("Неизвестный вид или вариант пасьянса"),
        }
    }
    pub fn restore(json: &str) -> Result<Self, &'static str> {
        Self::from_value(serde_json::from_str(json).map_err(|_| "Повреждённое сохранение")?)
    }
    pub fn new(kind: Kind, seed: u64, mode: Mode) -> Self {
        match kind {
            Kind::Spider => Game::new_with_mode(seed, mode).into(),
            Kind::Klondike => klondike::Game::new(seed).into(),
            Kind::Tripeaks => Self::Tripeaks(tripeaks::Game::new(seed)),
        }
    }
    pub fn kind(&self) -> Kind {
        match self {
            Self::Spider(_) => Kind::Spider,
            Self::Klondike(_) => Kind::Klondike,
            Self::Tripeaks(_) => Kind::Tripeaks,
        }
    }
    pub fn spider(&self) -> Option<&Game> {
        if let Self::Spider(g) = self {
            Some(g)
        } else {
            None
        }
    }
    pub fn into_spider(self) -> Option<Game> {
        if let Self::Spider(g) = self {
            Some(g)
        } else {
            None
        }
    }
    pub fn klondike(&self) -> Option<&klondike::Game> {
        if let Self::Klondike(g) = self {
            Some(g)
        } else {
            None
        }
    }
    pub fn tripeaks(&self) -> Option<&tripeaks::Game> {
        if let Self::Tripeaks(g) = self {
            Some(g)
        } else {
            None
        }
    }
    pub fn stuck(&self) -> bool {
        self.tripeaks()
            .is_some_and(|g| g.state.status == tripeaks::Status::Stuck)
    }
    pub fn finished(&self) -> bool {
        self.won() || self.stuck()
    }
    pub fn mode_label(&self) -> &'static str {
        match self {
            Self::Spider(g) => g.state.mode.label(),
            Self::Klondike(_) => "По одной карте",
            Self::Tripeaks(_) => "Один проход запаса",
        }
    }
    pub fn spider_mode(&self) -> Mode {
        self.spider().map_or(Mode::One, |g| g.state.mode)
    }
    pub fn moves(&self) -> u32 {
        match self {
            Self::Spider(g) => g.state.moves,
            Self::Klondike(g) => g.state.moves,
            Self::Tripeaks(g) => g.state.moves,
        }
    }
    pub fn won(&self) -> bool {
        match self {
            Self::Spider(g) => g.state.won(),
            Self::Klondike(g) => g.state.won(),
            Self::Tripeaks(g) => g.state.won(),
        }
    }
    pub fn history_len(&self) -> usize {
        match self {
            Self::Spider(g) => g.history.len(),
            Self::Klondike(g) => g.history.len(),
            Self::Tripeaks(g) => g.history.len(),
        }
    }
    pub fn undo(&mut self) -> bool {
        match self {
            Self::Spider(g) => g.undo(),
            Self::Klondike(g) => g.undo(),
            Self::Tripeaks(g) => g.undo(),
        }
    }
    pub fn validate(&self) -> Result<(), &'static str> {
        match self {
            Self::Spider(g) => g.validate(),
            Self::Klondike(g) => g.validate(),
            Self::Tripeaks(g) => g.validate(),
        }
    }
    pub fn deal_id(&self) -> Option<PublicDeal> {
        match self {
            Self::Spider(g) => g.deal.clone().map(PublicDeal::Spider),
            Self::Klondike(g) => Some(PublicDeal::Klondike(g.deal)),
            Self::Tripeaks(g) => Some(PublicDeal::Tripeaks(g.deal)),
        }
    }
    pub fn deal_code(&self) -> Option<String> {
        self.deal_id().map(|d| d.code())
    }
    pub fn fits_slot(&self, slot: &str) -> bool {
        match slot {
            "regular" => self.kind() == Kind::Spider,
            "klondike_regular" => self.kind() == Kind::Klondike,
            "tripeaks_regular" => self.kind() == Kind::Tripeaks,
            _ if slot.starts_with("daily_") => {
                self.kind() == Kind::Spider
                    && self.deal_code() == Some(format!("d1_{}", &slot[6..]))
            }
            _ => slot
                .strip_prefix("challenge_")
                .is_some_and(|code| self.deal_code().as_deref() == Some(code)),
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PublicDeal {
    Spider(DealId),
    Klondike(klondike::Deal),
    Tripeaks(tripeaks::Deal),
}
impl PublicDeal {
    pub fn parse(code: &str) -> Result<Self, &'static str> {
        if code.len() > 64 || !code.is_ascii() {
            return Err("Некорректный код расклада");
        }
        if code.starts_with('t') {
            tripeaks::Deal::parse(code).map(Self::Tripeaks)
        } else if code.starts_with('k') {
            klondike::Deal::parse(code).map(Self::Klondike)
        } else {
            DealId::parse(code).map(Self::Spider)
        }
    }
    pub fn code(&self) -> String {
        match self {
            Self::Spider(d) => d.code(),
            Self::Klondike(d) => d.code(),
            Self::Tripeaks(d) => d.code(),
        }
    }
    pub fn start(&self) -> Result<Session, &'static str> {
        match self {
            Self::Spider(d) => d.start().map(Session::Spider),
            Self::Klondike(d) => d.start().map(Session::Klondike),
            Self::Tripeaks(d) => d.start().map(Session::Tripeaks),
        }
    }
    pub fn kind(&self) -> Kind {
        match self {
            Self::Spider(_) => Kind::Spider,
            Self::Klondike(_) => Kind::Klondike,
            Self::Tripeaks(_) => Kind::Tripeaks,
        }
    }
    pub fn label(&self) -> Result<String, &'static str> {
        match self {
            Self::Spider(d) => d.mode().map(|m| format!("Паук · {}", m.label())),
            Self::Klondike(_) => Ok("Косынка · По одной карте".into()),
            Self::Tripeaks(_) => Ok("Три пика · Один проход запаса".into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn historic_spider_saves_in_all_modes_keep_cards_and_undo_without_fake_seed() {
        for mode in Mode::ALL {
            let mut g = Game::new_with_mode(765, mode);
            g.deal().unwrap();
            let mut old = serde_json::to_value(&g).unwrap();
            old["version"] = 2.into();
            old.as_object_mut().unwrap().remove("deal");
            let mut restored = Session::restore(&old.to_string()).unwrap();
            assert_eq!(restored.kind(), Kind::Spider);
            assert_eq!(restored.spider().unwrap().state, g.state);
            assert_eq!(restored.spider().unwrap().history, g.history);
            assert!(restored.deal_code().is_none());
            assert!(restored.undo());
            assert_eq!(restored.spider().unwrap().state, g.history[0]);
            if mode == Mode::One {
                old["version"] = 1.into();
                old["state"].as_object_mut().unwrap().remove("mode");
                for state in old["history"].as_array_mut().unwrap() {
                    state.as_object_mut().unwrap().remove("mode");
                }
                let v1 = Session::restore(&old.to_string()).unwrap();
                assert_eq!(v1.spider().unwrap().state, g.state);
                assert_eq!(v1.spider().unwrap().history, g.history);
            }
        }
    }
    #[test]
    fn both_games_roundtrip_with_explicit_kind_variant_and_exact_u64() {
        for kind in [Kind::Spider, Kind::Klondike, Kind::Tripeaks] {
            let g = Session::new(kind, u64::MAX, Mode::Four);
            let json = serde_json::to_string(&g).unwrap();
            assert!(json.contains("18446744073709551615"));
            assert!(json.contains("\"kind\""));
            assert!(json.contains("\"variant\""));
            assert_eq!(Session::restore(&json).unwrap(), g);
            assert_eq!(serde_json::from_str::<Session>(&json).unwrap(), g);
            let code = g.deal_code().unwrap();
            assert_eq!(PublicDeal::parse(&code).unwrap().start().unwrap(), g);
            assert!(g.fits_slot(kind.regular_slot()));
            assert!(!g.fits_slot(if kind == Kind::Spider {
                "klondike_regular"
            } else {
                "regular"
            }));
            assert!(g.fits_slot(&format!("challenge_{code}")));
        }
    }
    #[test]
    fn published_spider_links_are_unchanged_and_unknown_games_cannot_be_saved() {
        for code in ["s1_4_0123456789abcdef", "d1_20261115", "p1_0"] {
            let old = DealId::parse(code).unwrap().start().unwrap();
            let new = PublicDeal::parse(code).unwrap().start().unwrap();
            assert_eq!(new, Session::Spider(old));
            assert_eq!(new.deal_code().as_deref(), Some(code));
        }
        for code in [
            "k2_1_0123456789abcdef",
            "k1_3_0123456789abcdef",
            "k1_1_123",
            "k1_1_FFFFFFFFFFFFFFFF",
        ] {
            assert!(PublicDeal::parse(code).is_err());
        }
        for text in [
            r#"{"kind":"unknown","version":3}"#,
            r#"{"kind":"klondike","variant":"draw_three"}"#,
        ] {
            assert!(Session::restore(text).is_err());
        }
        assert!(crate::deal::valid_slot("klondike_regular"));
        assert!(crate::deal::valid_slot("challenge_k1_1_ffffffffffffffff"));
        assert!(!crate::deal::valid_slot("challenge_k2_1_ffffffffffffffff"));
    }
}
