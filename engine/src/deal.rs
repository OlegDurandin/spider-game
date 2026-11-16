//! Public, versioned deal IDs. No platform/session data belongs in these codes.
use crate::{Card, Game, Mode, State, VERSION};
use serde::{Deserialize, Serialize};
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum DealId {
    Seeded {
        algorithm: u8,
        mode: Mode,
        seed: u64,
    },
    Published {
        index: u8,
    },
    Daily {
        date: String,
    },
}
impl DealId {
    pub fn code(&self) -> String {
        match self {
            Self::Seeded {
                algorithm,
                mode,
                seed,
            } => format!("s{algorithm}_{}_{seed:016x}", mode.count()),
            Self::Published { index } => format!("p1_{index}"),
            Self::Daily { date } => format!("d1_{date}"),
        }
    }
    pub fn parse(code: &str) -> Result<Self, &'static str> {
        if code.len() > 64 || !code.is_ascii() {
            return Err("Некорректный код расклада");
        }
        let parts: Vec<_> = code.split('_').collect();
        let id = match parts.as_slice() {
            ["s1", count, seed] if seed.len() == 16 => Self::Seeded {
                algorithm: 1,
                mode: match *count {
                    "1" => Mode::One,
                    "2" => Mode::Two,
                    "4" => Mode::Four,
                    _ => return Err("Неизвестная сложность"),
                },
                seed: u64::from_str_radix(seed, 16).map_err(|_| "Некорректный seed")?,
            },
            ["p1", n] => Self::Published {
                index: n.parse().map_err(|_| "Некорректный код расклада")?,
            },
            ["d1", date] => {
                date_days(date)?;
                Self::Daily {
                    date: date.to_string(),
                }
            }
            _ => return Err("Неизвестная версия или некорректный код расклада"),
        };
        id.mode()?;
        if id.code() != code {
            return Err("Некорректный код расклада");
        }
        Ok(id)
    }
    pub fn mode(&self) -> Result<Mode, &'static str> {
        match self {
            Self::Seeded {
                algorithm: 1, mode, ..
            } => Ok(*mode),
            Self::Published { index } if *index < 7 => Ok(Mode::One),
            Self::Daily { date } => {
                date_days(date)?;
                Ok(Mode::One)
            }
            _ => Err("Неизвестная версия расклада"),
        }
    }
    pub fn start(&self) -> Result<Game, &'static str> {
        let mode = self.mode()?;
        match self {
            Self::Seeded { seed, .. } => Ok(Game::new_with_mode(*seed, mode)),
            Self::Published { index } => published(*index, self.clone()),
            Self::Daily { date } => {
                let offset = date_days(date)? - date_days("20261115")?;
                published(offset.rem_euclid(7) as u8, self.clone())
            }
        }
    }
}
#[derive(Deserialize)]
pub struct Published {
    pub columns: [Vec<Card>; 10],
    pub stock: Vec<Card>,
    pub solution: Vec<Command>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Command {
    Move { r#move: [usize; 3] },
    Deal { deal: bool },
}
impl Command {
    pub fn apply(&self, g: &mut Game) -> Result<(), crate::Error> {
        match self {
            Self::Move {
                r#move: [from, start, to],
            } => g.apply(crate::Move {
                from: *from,
                start: *start,
                to: *to,
            }),
            Self::Deal { deal: true } => g.deal(),
            _ => Err(crate::Error::InvalidMove),
        }
    }
}
pub fn catalog(index: u8) -> Result<Published, &'static str> {
    let text = match index {
        0 => include_str!("../daily/0.json"),
        1 => include_str!("../daily/1.json"),
        2 => include_str!("../daily/2.json"),
        3 => include_str!("../daily/3.json"),
        4 => include_str!("../daily/4.json"),
        5 => include_str!("../daily/5.json"),
        6 => include_str!("../daily/6.json"),
        _ => return Err("Неизвестный расклад"),
    };
    serde_json::from_str(text).map_err(|_| "Повреждённый каталог")
}
fn published(index: u8, id: DealId) -> Result<Game, &'static str> {
    let p = catalog(index)?;
    let g = Game {
        version: VERSION,
        deal: Some(id),
        state: State {
            mode: Mode::One,
            columns: p.columns,
            stock: p.stock,
            completed: vec![],
            moves: 0,
        },
        history: vec![],
    };
    g.validate()?;
    Ok(g)
}
// Gregorian civil date arithmetic, independent of locale/timezone and WASM integer width.
fn days(y: i64, m: i64, d: i64) -> i64 {
    let y = y - i64::from(m <= 2);
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = m + if m > 2 { -3 } else { 9 };
    let doy = (153 * mp + 2) / 5 + d - 1;
    era * 146097 + yoe * 365 + yoe / 4 - yoe / 100 + doy - 719468
}
fn civil(z: i64) -> (i64, i64, i64) {
    let z = z + 719468;
    let era = z.div_euclid(146097);
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = mp + if mp < 10 { 3 } else { -9 };
    (y + i64::from(m <= 2), m, d)
}
fn date_days(date: &str) -> Result<i64, &'static str> {
    if date.len() != 8 || !date.bytes().all(|c| c.is_ascii_digit()) {
        return Err("Некорректная дата");
    }
    let y = date[..4].parse::<i64>().map_err(|_| "Некорректная дата")?;
    let m = date[4..6].parse::<i64>().unwrap();
    let d = date[6..].parse::<i64>().unwrap();
    if !(1970..=9999).contains(&y) || !(1..=12).contains(&m) || !(1..=31).contains(&d) {
        return Err("Некорректная дата");
    }
    let n = days(y, m, d);
    if civil(n) != (y, m, d) {
        return Err("Некорректная дата");
    }
    Ok(n)
}
pub fn moscow_date(unix_seconds: i64) -> String {
    let (y, m, d) = civil((unix_seconds + 10800).div_euclid(86400));
    format!("{y:04}{m:02}{d:02}")
}
pub fn daily(unix_seconds: i64) -> DealId {
    DealId::Daily {
        date: moscow_date(unix_seconds),
    }
}

pub fn valid_slot(slot: &str) -> bool {
    slot == "regular"
        || slot == "klondike_regular"
        || slot == "tripeaks_regular"
        || slot
            .strip_prefix("daily_")
            .is_some_and(|d| DealId::parse(&format!("d1_{d}")).is_ok())
        || slot
            .strip_prefix("challenge_")
            .is_some_and(|d| crate::PublicDeal::parse(d).is_ok())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ids_are_exact_and_versioned() {
        for mode in Mode::ALL {
            for seed in [0, 1, 0x0123456789abcdef, u64::MAX] {
                let id = DealId::Seeded {
                    algorithm: 1,
                    mode,
                    seed,
                };
                assert_eq!(DealId::parse(&id.code()).unwrap(), id);
                assert_eq!(id.start().unwrap(), Game::new_with_mode(seed, mode));
            }
        }
        for bad in [
            "s2_1_0000000000000001",
            "s1_3_0000000000000001",
            "s1_1_18446744073709551616",
            "d1_20260230",
            "p1_7",
            "d1_2026111",
            "s1_1_ABCDEF0000000000",
        ] {
            assert!(DealId::parse(bad).is_err(), "{bad}");
        }
    }
    #[test]
    fn golden_starting_decks_match_every_card_in_all_modes() {
        let fixture: serde_json::Value =
            serde_json::from_str(include_str!("../fixtures/seed-v1.json")).unwrap();
        for (mode, key) in [(Mode::One, "one"), (Mode::Two, "two"), (Mode::Four, "four")] {
            let g = Game::new_with_mode(0x0123456789abcdef, mode);
            assert_eq!(serde_json::to_value(g.state).unwrap(), fixture[key]);
        }
    }
    #[test]
    fn repeat_resets_exactly_and_legacy_is_not_assigned_a_seed() {
        for mode in Mode::ALL {
            let initial = Game::new_with_mode(u64::MAX, mode);
            let mut g = initial.clone();
            g.deal().unwrap();
            if let Some(m) = g.hint() {
                g.apply(m).unwrap();
            }
            g.undo();
            assert_eq!(g.deal.as_ref().unwrap().start().unwrap(), initial);
            let mut old = serde_json::to_value(&g).unwrap();
            old["version"] = 2.into();
            old.as_object_mut().unwrap().remove("deal");
            let mut restored = Game::restore(&old.to_string()).unwrap();
            assert!(restored.deal.is_none());
            assert_eq!(restored.state, g.state);
            assert_eq!(restored.history, g.history);
            assert!(restored.undo());
        }
    }
    #[test]
    fn moscow_midnight_and_leap_days_are_fixed() {
        let midnight = 1794776400;
        assert_eq!(moscow_date(midnight - 1), "20261115");
        assert_eq!(moscow_date(midnight + 1), "20261116");
        assert!(DealId::parse("d1_20240229").is_ok());
        assert!(DealId::parse("d1_20250229").is_err());
        assert_eq!(moscow_date(0), "19700101");
    }
    #[test]
    fn all_catalog_solutions_win_through_public_ids() {
        for index in 0..7 {
            let id = DealId::Published { index };
            let mut game = DealId::parse(&id.code()).unwrap().start().unwrap();
            let initial = game.clone();
            assert_eq!(game.state.stock.len(), 50);
            assert_eq!(
                game.state
                    .columns
                    .iter()
                    .flatten()
                    .filter(|c| c.open)
                    .count(),
                10
            );
            for command in catalog(index).unwrap().solution {
                command.apply(&mut game).unwrap();
                game.validate().unwrap();
            }
            assert!(game.state.won(), "catalog {index}");
            assert_eq!(initial, id.start().unwrap());
        }
    }
    #[test]
    fn first_week_rotation_is_frozen() {
        for (date, index) in [
            ("20261115", 0),
            ("20261116", 1),
            ("20261121", 6),
            ("20261122", 0),
            ("20270101", 5),
        ] {
            let a = DealId::Daily { date: date.into() }.start().unwrap();
            let b = DealId::Published { index }.start().unwrap();
            assert_eq!(a.state, b.state, "{date}");
        }
    }
}
