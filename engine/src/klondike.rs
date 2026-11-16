//! Klondike v1: draw one, unlimited redeals, foundation-to-tableau moves allowed.
use crate::{Card, HISTORY_LIMIT, Mode};
use serde::{Deserialize, Serialize};

pub const VARIANT: &str = "draw_one_unlimited_foundation_back";
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Playing,
    Won,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct State {
    pub columns: [Vec<Card>; 7],
    /// The last element is the next card to draw.
    pub stock: Vec<Card>,
    pub waste: Vec<Card>,
    /// Spades, hearts, clubs, diamonds, bottom to top.
    pub foundations: [Vec<Card>; 4],
    pub moves: u32,
    pub status: Status,
}
impl State {
    pub fn won(&self) -> bool {
        self.status == Status::Won
    }
    pub fn collected(&self) -> usize {
        self.foundations.iter().map(Vec::len).sum()
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Game {
    pub version: u8,
    pub variant: String,
    pub deal: Deal,
    pub state: State,
    pub history: Vec<State>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Deal {
    pub algorithm: u8,
    pub seed: u64,
}
impl Deal {
    pub fn code(self) -> String {
        format!("k{}_1_{:016x}", self.algorithm, self.seed)
    }
    pub fn parse(code: &str) -> Result<Self, &'static str> {
        let parts: Vec<_> = code.split('_').collect();
        match parts.as_slice() {
            ["k1", "1", seed] if seed.len() == 16 => {
                let id = Self {
                    algorithm: 1,
                    seed: u64::from_str_radix(seed, 16).map_err(|_| "Некорректный код расклада")?,
                };
                if id.code() != code {
                    return Err("Некорректный код расклада");
                }
                Ok(id)
            }
            _ => Err("Неизвестная версия или вариант «Косынки»"),
        }
    }
    pub fn start(self) -> Result<Game, &'static str> {
        if self.algorithm != 1 {
            return Err("Неизвестная версия раздачи");
        }
        Ok(Game::new(self.seed))
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "place", rename_all = "snake_case")]
pub enum Source {
    Tableau { column: usize, start: usize },
    Waste,
    Foundation { index: usize },
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "place", content = "index", rename_all = "snake_case")]
pub enum Target {
    Tableau(usize),
    Foundation(usize),
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum Command {
    Move { from: Source, to: Target },
    Draw,
}
fn red(c: Card) -> bool {
    c.suit(Mode::Four).red()
}
fn descending(cards: &[Card]) -> bool {
    !cards.is_empty()
        && cards.iter().all(|c| c.open)
        && cards
            .windows(2)
            .all(|p| p[0].rank() == p[1].rank() + 1 && red(p[0]) != red(p[1]))
}
impl Game {
    pub fn new(seed: u64) -> Self {
        let mut deck: Vec<_> = (0..52).map(|id| Card { id, open: false }).collect();
        let mut rng = seed;
        for i in (1..deck.len()).rev() {
            rng = rng.wrapping_add(0x9e3779b97f4a7c15);
            let mut z = rng;
            z = (z ^ (z >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
            z = (z ^ (z >> 27)).wrapping_mul(0x94d049bb133111eb);
            z ^= z >> 31;
            deck.swap(i, (z % (i as u64 + 1)) as usize);
        }
        // Frozen k1 contract: deal complete columns left to right from the deck's front.
        let columns = std::array::from_fn(|i| {
            let mut c: Vec<_> = deck.drain(..i + 1).collect();
            c.last_mut().unwrap().open = true;
            c
        });
        Self {
            version: 1,
            variant: VARIANT.into(),
            deal: Deal { algorithm: 1, seed },
            state: State {
                columns,
                stock: deck,
                waste: vec![],
                foundations: std::array::from_fn(|_| vec![]),
                moves: 0,
                status: Status::Playing,
            },
            history: vec![],
        }
    }
    pub fn cards(&self, source: Source) -> Option<&[Card]> {
        match source {
            Source::Tableau { column, start } => self
                .state
                .columns
                .get(column)?
                .get(start..)
                .filter(|c| descending(c)),
            Source::Waste => self.state.waste.last().map(std::slice::from_ref),
            Source::Foundation { index } => self
                .state
                .foundations
                .get(index)?
                .last()
                .map(std::slice::from_ref),
        }
    }
    pub fn selectable(&self, source: Source) -> bool {
        !self.state.won() && self.cards(source).is_some()
    }
    pub fn legal(&self, command: Command) -> bool {
        if self.state.won() || self.state.moves == u32::MAX {
            return false;
        }
        let Command::Move { from, to } = command else {
            return !self.state.stock.is_empty() || !self.state.waste.is_empty();
        };
        let Some(cards) = self.cards(from) else {
            return false;
        };
        let first = cards[0];
        match to {
            Target::Tableau(column) => {
                if matches!(from,Source::Tableau{column:c,..} if c==column) {
                    return false;
                }
                self.state.columns.get(column).is_some_and(|c| {
                    c.last().map_or(first.rank() == 13, |top| {
                        top.open && top.rank() == first.rank() + 1 && red(*top) != red(first)
                    })
                })
            }
            Target::Foundation(index) => {
                if cards.len() != 1
                    || matches!(from, Source::Foundation { .. })
                    || index != first.id as usize / 13
                {
                    return false;
                }
                self.state
                    .foundations
                    .get(index)
                    .is_some_and(|c| first.rank() as usize == c.len() + 1)
            }
        }
    }
    pub fn targets(&self, from: Source) -> Vec<Target> {
        (0..7)
            .map(Target::Tableau)
            .chain((0..4).map(Target::Foundation))
            .filter(|to| self.legal(Command::Move { from, to: *to }))
            .collect()
    }
    pub fn apply(&mut self, command: Command) -> Result<(), &'static str> {
        if !self.legal(command) {
            return Err("Недопустимый ход");
        }
        self.history.push(self.state.clone());
        if self.history.len() > HISTORY_LIMIT {
            self.history.remove(0);
        }
        match command {
            Command::Draw => {
                if let Some(mut c) = self.state.stock.pop() {
                    c.open = true;
                    self.state.waste.push(c);
                } else {
                    self.state.stock = self
                        .state
                        .waste
                        .drain(..)
                        .rev()
                        .map(|mut c| {
                            c.open = false;
                            c
                        })
                        .collect();
                }
            }
            Command::Move { from, to } => {
                let cards = match from {
                    Source::Tableau { column, start } => {
                        let cards = self.state.columns[column].split_off(start);
                        if let Some(top) = self.state.columns[column].last_mut() {
                            top.open = true;
                        }
                        cards
                    }
                    Source::Waste => vec![self.state.waste.pop().unwrap()],
                    Source::Foundation { index } => {
                        vec![self.state.foundations[index].pop().unwrap()]
                    }
                };
                match to {
                    Target::Tableau(c) => self.state.columns[c].extend(cards),
                    Target::Foundation(i) => self.state.foundations[i].extend(cards),
                }
            }
        }
        self.state.moves += 1;
        self.state.status = if self.state.collected() == 52 {
            Status::Won
        } else {
            Status::Playing
        };
        Ok(())
    }
    pub fn undo(&mut self) -> bool {
        if let Some(s) = self.history.pop() {
            self.state = s;
            true
        } else {
            false
        }
    }
    pub fn hint(&self) -> Option<Command> {
        if self.state.won() {
            return None;
        }
        let sources = self
            .state
            .columns
            .iter()
            .enumerate()
            .flat_map(|(column, c)| {
                (0..c.len()).map(move |start| Source::Tableau { column, start })
            })
            .chain(std::iter::once(Source::Waste))
            .chain((0..4).map(|index| Source::Foundation { index }));
        let mut best = None;
        for from in sources {
            for to in self.targets(from) {
                let cmd = Command::Move { from, to };
                let mut trial = self.clone();
                trial.apply(cmd).ok()?;
                if self
                    .history
                    .iter()
                    .rev()
                    .take(20)
                    .any(|s| same_position(s, &trial.state))
                {
                    continue;
                }
                let mut score = match to {
                    Target::Foundation(_) => 100,
                    Target::Tableau(_) => 20,
                };
                if let Source::Tableau { column, start } = from {
                    if start > 0 && !self.state.columns[column][start - 1].open {
                        score += 70;
                    }
                    if start == 0
                        && matches!(to,Target::Tableau(c) if self.state.columns[c].is_empty())
                    {
                        score -= 100;
                    }
                }
                if matches!(from, Source::Waste) {
                    score += 10;
                }
                if matches!(from, Source::Foundation { .. }) {
                    score -= 150;
                }
                if best.is_none_or(|(old, _)| score > old) {
                    best = Some((score, cmd));
                }
            }
        }
        if let Some((score, cmd)) = best {
            if score > 0 {
                return Some(cmd);
            }
        }
        if self.legal(Command::Draw) {
            Some(Command::Draw)
        } else {
            best.map(|(_, cmd)| cmd)
        }
    }
    pub fn validate(&self) -> Result<(), &'static str> {
        if self.version != 1
            || self.variant != VARIANT
            || self.deal.algorithm != 1
            || self.history.len() > HISTORY_LIMIT
        {
            return Err("Неподдерживаемый формат «Косынки»");
        }
        for s in std::iter::once(&self.state).chain(self.history.iter()) {
            validate_state(s)?;
        }
        Ok(())
    }
    pub fn restore(json: &str) -> Result<Self, &'static str> {
        let g: Self =
            serde_json::from_str(json).map_err(|_| "Повреждённое сохранение «Косынки»")?;
        g.validate()?;
        Ok(g)
    }
}
fn same_position(a: &State, b: &State) -> bool {
    a.columns == b.columns
        && a.stock == b.stock
        && a.waste == b.waste
        && a.foundations == b.foundations
}
fn validate_state(s: &State) -> Result<(), &'static str> {
    let mut seen = [false; 52];
    for c in s
        .columns
        .iter()
        .flatten()
        .chain(s.stock.iter())
        .chain(s.waste.iter())
        .chain(s.foundations.iter().flatten())
    {
        if c.id >= 52 || seen[c.id as usize] {
            return Err("Неверный набор карт «Косынки»");
        }
        seen[c.id as usize] = true;
    }
    if seen.contains(&false) || s.stock.iter().any(|c| c.open) || s.waste.iter().any(|c| !c.open) {
        return Err("Неверный запас или сброс");
    }
    for column in &s.columns {
        if column.is_empty() {
            continue;
        }
        let Some(start) = column.iter().position(|c| c.open) else {
            return Err("Не открыта верхняя карта");
        };
        if !descending(&column[start..]) {
            return Err("Неверный открытый хвост");
        }
    }
    for (index, f) in s.foundations.iter().enumerate() {
        if f.iter().enumerate().any(|(rank, c)| {
            !c.open || c.id as usize / 13 != index || c.rank() as usize != rank + 1
        }) {
            return Err("Неверный дом");
        }
    }
    if s.won() != (s.collected() == 52) {
        return Err("Неверный статус партии");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn c(id: u8) -> Card {
        Card { id, open: true }
    }
    fn source(column: usize, start: usize) -> Source {
        Source::Tableau { column, start }
    }
    fn mv(from: Source, to: Target) -> Command {
        Command::Move { from, to }
    }
    pub(super) fn position(
        columns: Vec<Vec<Card>>,
        stock: Vec<Card>,
        waste: Vec<Card>,
        foundations: [Vec<Card>; 4],
    ) -> Game {
        let mut g = Game::new(42);
        g.state.columns = std::array::from_fn(|i| columns.get(i).cloned().unwrap_or_default());
        let used: Vec<_> = g
            .state
            .columns
            .iter()
            .flatten()
            .chain(stock.iter())
            .chain(waste.iter())
            .chain(foundations.iter().flatten())
            .map(|c| c.id)
            .collect();
        g.state.stock = (0..52)
            .filter(|id| !used.contains(id))
            .map(|id| Card { id, open: false })
            .chain(stock)
            .collect();
        g.state.waste = waste;
        g.state.foundations = foundations;
        g.state.status = if g.state.collected() == 52 {
            Status::Won
        } else {
            Status::Playing
        };
        g.validate().unwrap();
        g
    }
    fn empty_foundations() -> [Vec<Card>; 4] {
        std::array::from_fn(|_| vec![])
    }
    #[test]
    fn fixed_start_has_52_unique_cards_and_exact_cross_target_order() {
        let golden: std::collections::BTreeMap<String, State> =
            serde_json::from_str(include_str!("../fixtures/klondike-v1.json")).unwrap();
        for (code, state) in golden {
            let g = Deal::parse(&code).unwrap().start().unwrap();
            g.validate().unwrap();
            assert_eq!(g.state, state, "{code}");
            assert_eq!(
                g.state.columns.each_ref().map(Vec::len),
                [1, 2, 3, 4, 5, 6, 7]
            );
            assert_eq!(
                g.state.columns.iter().flatten().filter(|c| c.open).count(),
                7
            );
            assert_eq!(g.state.stock.len(), 24);
            assert!(g.state.waste.is_empty());
            assert_eq!(g.state.collected(), 0);
        }
    }
    #[test]
    fn alternating_tail_targets_are_exact_and_same_color_is_rejected() {
        let mut g = position(
            vec![
                vec![c(8), c(20), c(32)],
                vec![c(34)],
                vec![c(47)],
                vec![c(46)],
            ],
            vec![],
            vec![],
            empty_foundations(),
        );
        assert!(g.selectable(source(0, 0)));
        assert!(g.selectable(source(0, 1)));
        assert!(g.selectable(source(0, 2)));
        assert_eq!(g.targets(source(0, 1)), vec![Target::Tableau(1)]);
        assert_eq!(g.targets(source(0, 2)), vec![Target::Tableau(3)]);
        assert!(!g.legal(mv(source(0, 1), Target::Tableau(2))));
        g.apply(mv(source(0, 1), Target::Tableau(1))).unwrap();
        g.validate().unwrap();
        assert_eq!(g.state.columns[0], vec![c(8)]);
        assert_eq!(g.state.columns[1], vec![c(34), c(20), c(32)]);
    }
    #[test]
    fn only_kings_fill_empty_columns_and_flip_is_undone_atomically() {
        let closed = Card {
            id: 30,
            open: false,
        };
        let mut g = position(
            vec![vec![closed, c(12), c(24)]],
            vec![],
            vec![],
            empty_foundations(),
        );
        let old = g.clone();
        assert!(!g.legal(mv(source(0, 2), Target::Tableau(1))));
        g.apply(mv(source(0, 1), Target::Tableau(1))).unwrap();
        g.validate().unwrap();
        assert!(g.state.columns[0][0].open);
        assert_eq!(g.state.moves, 1);
        assert!(g.undo());
        assert_eq!(g, old);
    }
    #[test]
    fn waste_card_can_choose_tableau_or_its_own_foundation_and_return() {
        let mut f = empty_foundations();
        f[3] = (39..43).map(c).collect();
        f[1] = (13..17).map(c).collect();
        let mut g = position(vec![vec![c(31)]], vec![], vec![c(43)], f);
        assert_eq!(
            g.targets(Source::Waste),
            vec![Target::Tableau(0), Target::Foundation(3)]
        );
        assert!(!g.legal(mv(Source::Waste, Target::Foundation(1))));
        g.apply(mv(Source::Waste, Target::Foundation(3))).unwrap();
        assert!(g.legal(mv(Source::Foundation { index: 3 }, Target::Tableau(0))));
        assert!(!g.legal(mv(Source::Foundation { index: 3 }, Target::Foundation(3))));
        g.apply(mv(Source::Foundation { index: 3 }, Target::Tableau(0)))
            .unwrap();
        g.validate().unwrap();
        assert_eq!(g.state.columns[0], vec![c(31), c(43)]);
        assert_eq!(g.state.moves, 2);
    }
    #[test]
    fn redeal_keeps_unplayed_order_and_undo_and_restore_are_exact() {
        let stock = [8, 13, 29].map(|id| Card { id, open: false }).to_vec();
        let mut g = position(vec![], stock, vec![], empty_foundations());
        g.apply(Command::Draw).unwrap();
        assert_eq!(g.state.waste.last(), Some(&c(29)));
        g.apply(Command::Draw).unwrap();
        assert_eq!(g.state.waste.last(), Some(&c(13)));
        g.apply(mv(Source::Waste, Target::Foundation(1))).unwrap();
        g.apply(Command::Draw).unwrap();
        assert_eq!(g.state.waste.last(), Some(&c(8)));
        while !g.state.stock.is_empty() {
            g.apply(Command::Draw).unwrap();
        }
        let before = g.clone();
        g.apply(Command::Draw).unwrap();
        assert!(g.state.waste.is_empty());
        assert!(g.state.stock.iter().all(|c| !c.open));
        let after = g.clone();
        assert!(g.undo());
        assert_eq!(g, before);
        g.apply(Command::Draw).unwrap();
        assert_eq!(g, after);
        g = Game::restore(&serde_json::to_string(&g).unwrap()).unwrap();
        g.apply(Command::Draw).unwrap();
        assert_eq!(g.state.waste.last(), Some(&c(29)));
        g.apply(Command::Draw).unwrap();
        assert_eq!(g.state.waste.last(), Some(&c(8)));
        for _ in 0..3 {
            while !g.state.stock.is_empty() {
                g.apply(Command::Draw).unwrap();
            }
            g.apply(Command::Draw).unwrap();
            g.validate().unwrap();
        }
        assert_eq!(g.state.foundations[1], vec![c(13)]);
    }
    #[test]
    fn last_king_wins_once_restore_and_undo_recover_51_cards() {
        let f = std::array::from_fn(|i| {
            (i as u8 * 13..i as u8 * 13 + if i == 3 { 12 } else { 13 })
                .map(c)
                .collect()
        });
        let mut g = position(vec![vec![c(51)]], vec![], vec![], f);
        let before = g.clone();
        assert!(!g.legal(Command::Draw));
        assert!(g.apply(Command::Draw).is_err());
        assert_eq!(g, before);
        g.apply(mv(source(0, 0), Target::Foundation(3))).unwrap();
        assert!(g.state.won());
        assert_eq!(g.state.moves, 1);
        let saved = serde_json::to_string(&g).unwrap();
        g = Game::restore(&saved).unwrap();
        let won = g.clone();
        assert!(g.apply(Command::Draw).is_err());
        assert_eq!(g, won);
        assert!(g.undo());
        assert_eq!(g, before);
        assert_eq!(g.state.collected(), 51);
    }
    #[test]
    fn commands_are_atomic_and_hints_use_the_same_legal_moves() {
        for seed in [1, u64::MAX] {
            let initial = Game::new(seed);
            let mut g = initial.clone();
            for step in 0..300 {
                let before = g.clone();
                assert!(g.apply(mv(source(99, 99), Target::Tableau(99))).is_err());
                assert_eq!(g, before);
                let Some(cmd) = g.hint() else { break };
                assert!(g.legal(cmd));
                g.apply(cmd).unwrap();
                g.validate().unwrap();
                if step % 17 == 0 {
                    let after = g.clone();
                    g.undo();
                    assert_eq!(g.state, before.state);
                    g.apply(cmd).unwrap();
                    assert_eq!(g, after);
                }
                let restored = Game::restore(&serde_json::to_string(&g).unwrap()).unwrap();
                assert_eq!(g, restored);
            }
            assert_eq!(g.deal.start().unwrap(), initial);
            assert!(g.history.len() <= HISTORY_LIMIT);
        }
    }
    #[test]
    fn corrupt_status_cards_history_and_unknown_variants_are_rejected() {
        let g = Game::new(42);
        for field in ["status", "cards", "variant", "history", "algorithm"] {
            let mut v = serde_json::to_value(&g).unwrap();
            match field {
                "status" => v["state"]["status"] = "won".into(),
                "cards" => v["state"]["stock"][0] = v["state"]["stock"][1].clone(),
                "variant" => v["variant"] = "draw_three".into(),
                "algorithm" => v["deal"]["algorithm"] = 2.into(),
                _ => {
                    let mut s = g.state.clone();
                    s.stock[0].open = true;
                    v["history"] = serde_json::json!([s]);
                }
            }
            assert!(Game::restore(&v.to_string()).is_err(), "{field}");
        }
    }
}
