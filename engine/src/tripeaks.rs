//! TriPeaks v1. Positions A=0..3, B=3..9, C=9..18, D=18..28.
use crate::{Card, HISTORY_LIMIT};
use serde::{Deserialize, Serialize};
pub const VARIANT: &str = "single_pass_wrap";
/// The sole topology for both exposure and the board's coordinates.
pub fn children(index: usize) -> Option<[usize; 2]> {
    match index {
        0..=2 => Some([3 + index * 2, 4 + index * 2]),
        3..=8 => {
            let b = index - 3;
            let c = 9 + b / 2 * 3 + b % 2;
            Some([c, c + 1])
        }
        9..=17 => Some([18 + index - 9, 19 + index - 9]),
        _ => None,
    }
}
/// Horizontal center in bottom-card spacings, derived from covering cards.
pub fn center(index: usize) -> f64 {
    children(index).map_or((index.saturating_sub(18)) as f64, |[a, b]| {
        (center(a) + center(b)) / 2.
    })
}
pub fn row(index: usize) -> usize {
    match index {
        0..=2 => 0,
        3..=8 => 1,
        9..=17 => 2,
        _ => 3,
    }
}
pub fn adjacent(a: u8, b: u8) -> bool {
    (1..=13).contains(&a) && (1..=13).contains(&b) && (a.abs_diff(b) == 1 || a.abs_diff(b) == 12)
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Playing,
    Won,
    Stuck,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct State {
    pub tableau: [Option<Card>; 28],
    /// Last card is drawn next.
    pub stock: Vec<Card>,
    pub waste: Vec<Card>,
    pub moves: u32,
    pub chain: u32,
    pub best_chain: u32,
    pub status: Status,
}
impl State {
    pub fn won(&self) -> bool {
        self.status == Status::Won
    }
    pub fn free(&self, index: usize) -> bool {
        index < 28
            && self.tableau[index].is_some()
            && children(index)
                .is_none_or(|pair| pair.into_iter().all(|i| self.tableau[i].is_none()))
    }
    fn legal_card(&self, index: usize) -> bool {
        self.free(index)
            && self.tableau[index].is_some_and(|c| {
                c.open
                    && self
                        .waste
                        .last()
                        .is_some_and(|w| adjacent(c.rank(), w.rank()))
            })
    }
    fn outcome(&self) -> Status {
        if self.tableau.iter().all(Option::is_none) {
            Status::Won
        } else if self.stock.is_empty() && !(0..28).any(|i| self.legal_card(i)) {
            Status::Stuck
        } else {
            Status::Playing
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Deal {
    pub algorithm: u8,
    pub seed: u64,
}
impl Deal {
    pub fn code(self) -> String {
        format!("t{}_1_{:016x}", self.algorithm, self.seed)
    }
    pub fn parse(code: &str) -> Result<Self, &'static str> {
        let seed = code
            .strip_prefix("t1_1_")
            .filter(|s| s.len() == 16)
            .ok_or("Неизвестная версия «Трёх пиков»")?;
        let d = Self {
            algorithm: 1,
            seed: u64::from_str_radix(seed, 16).map_err(|_| "Некорректный код расклада")?,
        };
        if d.code() != code {
            return Err("Некорректный код расклада");
        };
        Ok(d)
    }
    pub fn start(self) -> Result<Game, &'static str> {
        if self.algorithm != 1 {
            return Err("Неизвестная версия раздачи");
        };
        Ok(Game::new(self.seed))
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
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Command {
    Remove(usize),
    Draw,
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
        let tableau = std::array::from_fn(|i| {
            let mut c = deck.remove(0);
            c.open = i >= 18;
            Some(c)
        });
        let mut first = deck.remove(0);
        first.open = true;
        Self {
            version: 1,
            variant: VARIANT.into(),
            deal: Deal { algorithm: 1, seed },
            state: State {
                tableau,
                stock: deck,
                waste: vec![first],
                moves: 0,
                chain: 0,
                best_chain: 0,
                status: Status::Playing,
            },
            history: vec![],
        }
    }
    pub fn legal(&self, cmd: Command) -> bool {
        self.state.status == Status::Playing
            && match cmd {
                Command::Remove(i) => self.state.legal_card(i),
                Command::Draw => !self.state.stock.is_empty(),
            }
    }
    pub fn hint(&self) -> Option<Command> {
        (0..28)
            .map(Command::Remove)
            .find(|&c| self.legal(c))
            .or_else(|| self.legal(Command::Draw).then_some(Command::Draw))
    }
    pub fn apply(&mut self, cmd: Command) -> Result<(), &'static str> {
        if !self.legal(cmd) {
            return Err("Выберите свободную карту на ранг выше или ниже");
        }
        if self.history.len() == HISTORY_LIMIT {
            self.history.remove(0);
        }
        self.history.push(self.state.clone());
        match cmd {
            Command::Remove(i) => {
                self.state.waste.push(self.state.tableau[i].take().unwrap());
                self.state.chain += 1;
                self.state.best_chain = self.state.best_chain.max(self.state.chain);
            }
            Command::Draw => {
                let mut c = self.state.stock.pop().unwrap();
                c.open = true;
                self.state.waste.push(c);
                self.state.chain = 0;
            }
        }
        for i in 0..28 {
            let free = self.state.free(i);
            if let Some(c) = &mut self.state.tableau[i] {
                c.open = free;
            }
        }
        self.state.moves += 1;
        self.state.status = self.state.outcome();
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
    pub fn validate(&self) -> Result<(), &'static str> {
        if self.version != 1
            || self.variant != VARIANT
            || self.deal.algorithm != 1
            || self.history.len() > HISTORY_LIMIT
        {
            return Err("Неподдерживаемый формат");
        }
        for s in std::iter::once(&self.state).chain(&self.history) {
            if s.waste.is_empty()
                || s.stock.len() > 23
                || s.chain > s.best_chain
                || s.best_chain > 28
                || s.best_chain > s.moves
                || s.status != s.outcome()
            {
                return Err("Неверное состояние");
            }
            let mut seen = [false; 52];
            for c in s.tableau.iter().flatten().chain(&s.stock).chain(&s.waste) {
                if c.id >= 52 || seen[c.id as usize] {
                    return Err("Неверная колода");
                };
                seen[c.id as usize] = true;
            }
            if !seen.into_iter().all(|v| v)
                || s.stock.iter().any(|c| c.open)
                || s.waste.iter().any(|c| !c.open)
                || (0..28).any(|i| s.tableau[i].is_some_and(|c| c.open != s.free(i)))
            {
                return Err("Неверная открытость");
            }
            let removed = s.tableau.iter().filter(|c| c.is_none()).count();
            if s.moves as usize != removed + 23 - s.stock.len() || s.chain as usize > removed {
                return Err("Неверный счётчик");
            }
        }
        Ok(())
    }
    pub fn restore(json: &str) -> Result<Self, &'static str> {
        let g: Self = serde_json::from_str(json).map_err(|_| "Повреждённое сохранение")?;
        g.validate()?;
        Ok(g)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    // Synthetic complete positions retaining all physical cards for restore checks.
    fn position(table: &[(usize, u8)], stock: &[u8], top: u8) -> Game {
        let mut g = Game::new(42);
        g.state.tableau = [None; 28];
        for &(i, id) in table {
            g.state.tableau[i] = Some(Card { id, open: true });
        }
        for i in 0..28 {
            let free = g.state.free(i);
            if let Some(c) = &mut g.state.tableau[i] {
                c.open = free;
            }
        }
        g.state.stock = stock.iter().map(|&id| Card { id, open: false }).collect();
        g.state.waste = (0..52)
            .filter(|id| *id != top && !stock.contains(id) && !table.iter().any(|(_, c)| c == id))
            .map(|id| Card { id, open: true })
            .collect();
        g.state.waste.push(Card {
            id: top,
            open: true,
        });
        g.state.moves = (28 - table.len() + 23 - stock.len()) as u32;
        g.state.status = g.state.outcome();
        g.validate().unwrap();
        g
    }
    #[test]
    fn initial_distribution_and_frozen_code() {
        let g = Game::new(42);
        assert_eq!(g, Game::new(42));
        assert_ne!(g, Game::new(43));
        g.validate().unwrap();
        assert_eq!(g.state.tableau.iter().flatten().count(), 28);
        assert_eq!(
            g.state.tableau.iter().flatten().filter(|c| c.open).count(),
            10
        );
        assert_eq!((g.state.waste.len(), g.state.stock.len()), (1, 23));
        assert_eq!(
            g.state
                .tableau
                .iter()
                .flatten()
                .map(|c| c.id)
                .collect::<Vec<_>>(),
            [
                6, 28, 12, 39, 40, 0, 26, 25, 22, 4, 46, 36, 27, 42, 31, 17, 14, 43, 18, 13, 19, 1,
                33, 7, 16, 32, 51, 48
            ]
        );
        assert_eq!(Deal::parse(&g.deal.code()).unwrap().start().unwrap(), g);
        for bad in [
            "t2_1_000000000000002a",
            "t1_2_000000000000002a",
            "t1_1_FFFFFFFFFFFFFFFF",
            "t1_1_a",
        ] {
            assert!(Deal::parse(bad).is_err());
        }
    }
    #[test]
    fn ranks_taps_and_new_waste() {
        for a in 1..=13 {
            for b in 1..=13 {
                assert_eq!(adjacent(a, b), a.abs_diff(b) == 1 || a.abs_diff(b) == 12);
            }
        }
        let mut g = position(&[(18, 6), (19, 18), (20, 32), (0, 9)], &[51], 7);
        let before = g.clone();
        assert!(g.apply(Command::Remove(0)).is_err());
        assert!(g.apply(Command::Remove(99)).is_err());
        assert_eq!(g, before);
        g.apply(Command::Remove(18)).unwrap();
        assert_eq!(g.state.waste.last().unwrap().rank(), 7);
        g.apply(Command::Remove(19)).unwrap();
        assert_eq!(g.state.waste.last().unwrap().rank(), 6);
        g.apply(Command::Remove(20)).unwrap();
        assert_eq!((g.state.chain, g.state.best_chain), (3, 3));
        let before = g.clone();
        g.apply(Command::Draw).unwrap();
        assert_eq!((g.state.chain, g.state.best_chain), (0, 3));
        assert!(g.undo());
        assert_eq!(g, before);
    }
    #[test]
    fn shared_boundary_opens_two_and_undo_closes_both() {
        let mut g = position(&[(11, 9), (12, 10), (21, 6)], &[51], 7);
        let before = g.clone();
        assert!(!g.state.tableau[11].unwrap().open && !g.state.tableau[12].unwrap().open);
        assert_eq!(children(11), Some([20, 21]));
        assert_eq!(children(12), Some([21, 22]));
        assert_eq!(g.hint(), Some(Command::Remove(21)));
        assert_eq!(g, before);
        g.apply(Command::Remove(21)).unwrap();
        assert!(g.state.tableau[11].unwrap().open && g.state.tableau[12].unwrap().open);
        g.validate().unwrap();
        assert!(g.undo());
        assert_eq!(g, before);
    }
    #[test]
    fn last_stock_all_three_outcomes_and_result_undo() {
        let mut g = position(&[(18, 0)], &[12], 6);
        assert_eq!(g.hint(), Some(Command::Draw));
        let before = g.clone();
        g.apply(Command::Draw).unwrap();
        assert_eq!(g.state.status, Status::Playing);
        assert!(g.state.stock.is_empty());
        let last = g.clone();
        g.apply(Command::Remove(18)).unwrap();
        assert_eq!(g.state.status, Status::Won);
        assert_eq!((g.state.chain, g.state.best_chain), (1, 1));
        g.validate().unwrap();
        let mut restored = Game::restore(&serde_json::to_string(&g).unwrap()).unwrap();
        assert!(restored.undo());
        assert_eq!(restored, last);
        assert!(restored.undo());
        assert_eq!(restored, before);
        let mut g = position(&[(18, 0)], &[5], 6);
        let before = g.clone();
        g.apply(Command::Draw).unwrap();
        assert_eq!(g.state.status, Status::Stuck);
        assert_eq!(g.hint(), None);
        g.validate().unwrap();
        g.undo();
        assert_eq!(g, before);
    }
    #[test]
    fn generated_play_roundtrips_and_rejects_corruption() {
        for seed in 0..80 {
            let mut g = Game::new(seed);
            while let Some(cmd) = g.hint() {
                let before = g.clone();
                g.apply(cmd).unwrap();
                g.validate().unwrap();
                let text = serde_json::to_string(&g).unwrap();
                assert_eq!(Game::restore(&text).unwrap(), g);
                let mut trial = g.clone();
                assert!(trial.undo());
                assert_eq!(trial, before);
            }
            assert_ne!(g.state.status, Status::Playing);
        }
        let mut g = Game::new(42);
        g.state.stock[0] = g.state.stock[1];
        assert!(g.validate().is_err());
        let mut g = Game::new(42);
        g.state.tableau[0].as_mut().unwrap().open = true;
        assert!(g.validate().is_err());
        let mut g = Game::new(42);
        g.apply(Command::Draw).unwrap();
        g.history[0].status = Status::Won;
        assert!(g.validate().is_err());
    }
}
