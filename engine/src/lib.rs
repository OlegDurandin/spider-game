use serde::{Deserialize, Serialize};

pub mod deal;
pub mod klondike;
pub mod session;
pub mod tripeaks;
pub use deal::DealId;
pub use session::{Kind, PublicDeal, Session};
pub const VERSION: u8 = 3;
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    #[default]
    One,
    Two,
    Four,
}
impl Mode {
    pub const ALL: [Self; 3] = [Self::One, Self::Two, Self::Four];
    pub fn count(self) -> u8 {
        match self {
            Self::One => 1,
            Self::Two => 2,
            Self::Four => 4,
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::One => "1 масть",
            Self::Two => "2 масти",
            Self::Four => "4 масти",
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Suit {
    Spades,
    Hearts,
    Clubs,
    Diamonds,
}
impl Suit {
    pub fn symbol(self) -> &'static str {
        match self {
            Self::Spades => "♠",
            Self::Hearts => "♥",
            Self::Clubs => "♣",
            Self::Diamonds => "♦",
        }
    }
    pub fn code(self) -> &'static str {
        match self {
            Self::Spades => "S",
            Self::Hearts => "H",
            Self::Clubs => "C",
            Self::Diamonds => "D",
        }
    }
    pub fn name(self) -> &'static str {
        match self {
            Self::Spades => "пик",
            Self::Hearts => "червей",
            Self::Clubs => "треф",
            Self::Diamonds => "бубен",
        }
    }
    pub fn red(self) -> bool {
        matches!(self, Self::Hearts | Self::Diamonds)
    }
}
pub const HISTORY_LIMIT: usize = 100;
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Card {
    pub id: u8,
    pub open: bool,
}
impl Card {
    pub fn suit(self, mode: Mode) -> Suit {
        match (self.id / 13) % mode.count() {
            0 => Suit::Spades,
            1 => Suit::Hearts,
            2 => Suit::Clubs,
            _ => Suit::Diamonds,
        }
    }
    pub fn rank(self) -> u8 {
        self.id % 13 + 1
    }
}
pub fn label(rank: u8) -> String {
    match rank {
        1 => "A".into(),
        11 => "J".into(),
        12 => "Q".into(),
        13 => "K".into(),
        n => n.to_string(),
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct State {
    #[serde(default)]
    pub mode: Mode,
    pub columns: [Vec<Card>; 10],
    pub stock: Vec<Card>,
    pub completed: Vec<Vec<Card>>,
    pub moves: u32,
}
impl State {
    pub fn won(&self) -> bool {
        self.completed.len() == 8
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Game {
    #[serde(default)]
    pub deal: Option<DealId>,
    pub version: u8,
    pub state: State,
    pub history: Vec<State>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Move {
    pub from: usize,
    pub start: usize,
    pub to: usize,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    InvalidMove,
    EmptyColumn,
    EmptyStock,
    Won,
}
impl Error {
    pub fn message(self) -> &'static str {
        match self {
            Self::InvalidMove => "Выберите убывающую серию одной масти",
            Self::EmptyColumn => "Заполните пустой столбец",
            Self::EmptyStock => "Осталось раздач: 0",
            Self::Won => "Партия завершена",
        }
    }
}
impl Game {
    pub fn new(seed: u64) -> Self {
        Self::new_with_mode(seed, Mode::One)
    }
    pub fn new_with_mode(seed: u64, mode: Mode) -> Self {
        let mut deck: Vec<_> = (0..104).map(|id| Card { id, open: false }).collect();
        let mut rng = seed;
        for i in (1..deck.len()).rev() {
            rng = rng.wrapping_add(0x9e3779b97f4a7c15);
            let mut z = rng;
            z = (z ^ (z >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
            z = (z ^ (z >> 27)).wrapping_mul(0x94d049bb133111eb);
            z ^= z >> 31;
            deck.swap(i, (z % (i as u64 + 1)) as usize);
        }
        let columns = std::array::from_fn(|i| {
            let mut col = deck.drain(..if i < 4 { 6 } else { 5 }).collect::<Vec<_>>();
            col.last_mut().unwrap().open = true;
            col
        });
        Self {
            version: VERSION,
            deal: Some(DealId::Seeded {
                algorithm: 1,
                mode,
                seed,
            }),
            state: State {
                mode,
                columns,
                stock: deck,
                completed: vec![],
                moves: 0,
            },
            history: vec![],
        }
    }
    pub fn selectable(&self, col: usize, start: usize) -> bool {
        self.state.columns.get(col).is_some_and(|c| {
            start < c.len()
                && c[start..].iter().all(|c| c.open)
                && c[start..].windows(2).all(|p| {
                    p[0].rank() == p[1].rank() + 1
                        && p[0].suit(self.state.mode) == p[1].suit(self.state.mode)
                })
        })
    }
    pub fn legal(&self, m: Move) -> bool {
        !self.state.won()
            && m.from != m.to
            && self.selectable(m.from, m.start)
            && self.state.columns.get(m.to).is_some_and(|c| {
                c.last().is_none_or(|top| {
                    top.open && top.rank() == self.state.columns[m.from][m.start].rank() + 1
                })
            })
    }
    pub fn targets(&self, from: usize, start: usize) -> Vec<usize> {
        (0..10)
            .filter(|&to| self.legal(Move { from, start, to }))
            .collect()
    }
    fn snapshot(&mut self) {
        if self.history.len() >= HISTORY_LIMIT {
            self.history.remove(0);
        }
        self.history.push(self.state.clone());
    }
    pub fn apply(&mut self, m: Move) -> Result<(), Error> {
        if !self.legal(m) {
            return Err(Error::InvalidMove);
        }
        self.snapshot();
        let tail = self.state.columns[m.from].split_off(m.start);
        self.state.columns[m.to].extend(tail);
        self.finish();
        Ok(())
    }
    pub fn deal(&mut self) -> Result<(), Error> {
        if self.state.won() {
            return Err(Error::Won);
        }
        if self.state.stock.len() < 10 {
            return Err(Error::EmptyStock);
        }
        if self.state.columns.iter().any(Vec::is_empty) {
            return Err(Error::EmptyColumn);
        }
        self.snapshot();
        let row: Vec<_> = self.state.stock.drain(..10).collect();
        for (col, mut card) in self.state.columns.iter_mut().zip(row) {
            card.open = true;
            col.push(card);
        }
        self.finish();
        Ok(())
    }
    fn finish(&mut self) {
        for col in &mut self.state.columns {
            loop {
                if let Some(c) = col.last_mut() {
                    c.open = true;
                }
                if col.len() >= 13
                    && col[col.len() - 13..].iter().enumerate().all(|(i, c)| {
                        c.open
                            && c.rank() == 13 - i as u8
                            && c.suit(self.state.mode) == col[col.len() - 13].suit(self.state.mode)
                    })
                {
                    self.state.completed.push(col.split_off(col.len() - 13));
                } else {
                    break;
                }
            }
        }
        self.state.moves += 1;
    }
    pub fn undo(&mut self) -> bool {
        if let Some(state) = self.history.pop() {
            self.state = state;
            true
        } else {
            false
        }
    }
    /// A hint makes immediate structural progress; legal setup moves remain available manually.
    pub fn hint(&self) -> Option<Move> {
        let seen: Vec<_> = std::iter::once(&self.state)
            .chain(self.history.iter())
            .map(position_key)
            .collect();
        let mut best = None;
        let mut score = i32::MIN;
        for from in 0..10 {
            for start in 0..self.state.columns[from].len() {
                for to in self.targets(from, start) {
                    let c = &self.state.columns[from];
                    let dst = &self.state.columns[to];
                    let exposes = start > 0 && !c[start - 1].open;
                    let frees_column = start == 0 && !dst.is_empty();
                    let joins = dst.last().is_some_and(|top| {
                        top.suit(self.state.mode) == c[start].suit(self.state.mode)
                    });
                    let breaks = start > 0
                        && c[start - 1].open
                        && c[start - 1].rank() == c[start].rank() + 1
                        && c[start - 1].suit(self.state.mode) == c[start].suit(self.state.mode);
                    let join_gain = i32::from(joins) - i32::from(breaks);
                    let fills_for_deal =
                        dst.is_empty() && start > 0 && self.state.stock.len() >= 10;
                    let mut trial = Self {
                        version: VERSION,
                        deal: self.deal.clone(),
                        state: self.state.clone(),
                        history: vec![],
                    };
                    let m = Move { from, start, to };
                    trial.apply(m).ok()?;
                    let collections = trial.state.completed.len() - self.state.completed.len();
                    if collections == 0
                        && !exposes
                        && !frees_column
                        && join_gain <= 0
                        && !fills_for_deal
                    {
                        continue;
                    }
                    // Card copies and column numbering do not make an old position new.
                    if seen.contains(&position_key(&trial.state)) {
                        continue;
                    }
                    let s = collections as i32 * 1000
                        + i32::from(exposes) * 200
                        + i32::from(frees_column) * 120
                        + join_gain * 60
                        + i32::from(fills_for_deal) * 30
                        + (c.len() - start) as i32;
                    if s > score {
                        score = s;
                        best = Some(m);
                    }
                }
            }
        }
        best
    }
    pub fn validate(&self) -> Result<(), &'static str> {
        if self.version != VERSION || self.history.len() > HISTORY_LIMIT {
            return Err("Неподдерживаемый формат");
        }
        if let Some(id) = &self.deal {
            if id.mode()? != self.state.mode {
                return Err("Сложность идентификатора не совпадает");
            }
        }
        for s in std::iter::once(&self.state).chain(self.history.iter()) {
            if s.mode != self.state.mode {
                return Err("Масти в истории не совпадают");
            }
            validate_state(s)?;
        }
        Ok(())
    }
    pub fn restore(json: &str) -> Result<Self, &'static str> {
        let value: serde_json::Value =
            serde_json::from_str(json).map_err(|_| "Повреждённое сохранение")?;
        if value["version"] != 1
            && (value["state"].get("mode").is_none()
                || value["history"]
                    .as_array()
                    .is_some_and(|history| history.iter().any(|s| s.get("mode").is_none())))
        {
            return Err("Не указан режим игры");
        }
        let mut game: Self =
            serde_json::from_value(value).map_err(|_| "Повреждённое сохранение")?;
        if game.version == 2 {
            game.version = VERSION;
            game.deal = None;
        }
        if game.version == 1 {
            if std::iter::once(&game.state)
                .chain(game.history.iter())
                .any(|s| s.mode != Mode::One)
            {
                return Err("Неверный старый режим");
            }
            game.version = VERSION;
            game.deal = None;
        }
        game.validate()?;
        Ok(game)
    }
}
// Ignore counters, duplicate physical copies and column permutations when detecting hint loops.
type CardKey = (u8, u8, bool);
#[derive(PartialEq, Eq)]
struct PositionKey {
    mode: Mode,
    columns: Vec<Vec<CardKey>>,
    stock: Vec<CardKey>,
    completed: usize,
}
fn position_key(s: &State) -> PositionKey {
    let key = |c: &Card| (c.rank(), (c.id / 13) % s.mode.count(), c.open);
    let mut columns: Vec<Vec<_>> = s
        .columns
        .iter()
        .map(|c| c.iter().map(key).collect())
        .collect();
    columns.sort();
    PositionKey {
        mode: s.mode,
        columns,
        stock: s.stock.iter().map(key).collect(),
        completed: s.completed.len(),
    }
}
fn validate_state(s: &State) -> Result<(), &'static str> {
    if s.stock.len() > 50 || s.stock.len() % 10 != 0 || s.completed.len() > 8 {
        return Err("Неверные размеры");
    }
    let mut seen = [false; 104];
    for c in s
        .columns
        .iter()
        .flatten()
        .chain(s.stock.iter())
        .chain(s.completed.iter().flatten())
    {
        if c.id >= 104 || seen[c.id as usize] {
            return Err("Повторяющиеся карты");
        }
        seen[c.id as usize] = true;
    }
    if !seen.into_iter().all(|v| v) || s.stock.iter().any(|c| c.open) {
        return Err("Неверная колода");
    }
    for col in &s.columns {
        if col.last().is_some_and(|c| !c.open) || col.windows(2).any(|p| p[0].open && !p[1].open) {
            return Err("Неверная открытость");
        }
    }
    for run in &s.completed {
        if run.len() != 13
            || !run.iter().enumerate().all(|(i, c)| {
                c.open && c.rank() == 13 - i as u8 && c.suit(s.mode) == run[0].suit(s.mode)
            })
        {
            return Err("Неверная серия");
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn card(rank: u8) -> Card {
        Card {
            id: rank - 1,
            open: true,
        }
    }
    fn position(cols: Vec<Vec<Card>>) -> Game {
        let mut g = Game::new(1);
        g.state.columns = std::array::from_fn(|i| cols.get(i).cloned().unwrap_or_default());
        g.state.stock.clear();
        g.deal = None;
        g
    }
    fn sample() -> Game {
        position(vec![
            vec![
                Card {
                    id: 12,
                    open: false,
                },
                card(7),
                card(6),
                card(5),
            ],
            vec![card(8)],
        ])
    }
    #[test]
    fn seeded_deck_is_complete() {
        let g = Game::new(42);
        assert_eq!(g, Game::new(42));
        assert_ne!(g, Game::new(43));
        assert!(g.validate().is_ok());
        assert_eq!(
            g.state.columns.map(|c| c.len()),
            [6, 6, 6, 6, 5, 5, 5, 5, 5, 5]
        );
        let g = Game::new(42);
        assert_eq!(g.state.stock.len(), 50);
        assert_eq!(
            g.state.columns.iter().flatten().filter(|c| c.open).count(),
            10
        );
    }
    #[test]
    fn sample_move_and_exact_undo() {
        let mut g = sample();
        let before = g.clone();
        assert!(g.selectable(0, 2));
        assert_eq!(g.targets(0, 1).contains(&1), true);
        g.apply(Move {
            from: 0,
            start: 1,
            to: 1,
        })
        .unwrap();
        assert_eq!(
            g.state.columns[1]
                .iter()
                .map(|c| c.rank())
                .collect::<Vec<_>>(),
            [8, 7, 6, 5]
        );
        assert!(g.state.columns[0][0].open);
        assert_eq!(g.state.moves, 1);
        g.undo();
        assert_eq!(g, before);
    }
    #[test]
    fn suffix_selection_moves_only_suffix() {
        let mut g = sample();
        g.state.columns[1] = vec![card(7)];
        g.apply(Move {
            from: 0,
            start: 2,
            to: 1,
        })
        .unwrap();
        assert_eq!(g.state.columns[0].last().unwrap().rank(), 7);
        assert_eq!(g.state.columns[1].len(), 3);
    }
    #[test]
    fn invalid_commands_are_atomic() {
        let mut g = sample();
        g.state.columns[0][2] = card(4);
        let before = g.clone();
        for m in [
            Move {
                from: 0,
                start: 1,
                to: 1,
            },
            Move {
                from: 0,
                start: 3,
                to: 0,
            },
            Move {
                from: 0,
                start: 3,
                to: 1,
            },
            Move {
                from: 0,
                start: 0,
                to: 2,
            },
            Move {
                from: 100,
                start: 0,
                to: 0,
            },
            Move {
                from: 0,
                start: 100,
                to: 1,
            },
        ] {
            assert!(g.apply(m).is_err());
            assert_eq!(g, before);
        }
    }
    #[test]
    fn empty_destination_and_deal_guard() {
        let mut g = sample();
        g.state.stock = vec![Card { id: 0, open: false }; 10];
        let before = g.clone();
        assert_eq!(g.deal(), Err(Error::EmptyColumn));
        assert_eq!(g, before);
        g.apply(Move {
            from: 0,
            start: 1,
            to: 2,
        })
        .unwrap();
        assert_eq!(g.state.columns[2].len(), 3);
    }
    #[test]
    fn deal_order_and_undo() {
        let mut g = Game::new(123);
        let before = g.clone();
        let row = g.state.stock[..10].to_vec();
        g.deal().unwrap();
        assert_eq!(g.state.stock.len(), 40);
        for (col, card) in g.state.columns.iter().zip(row) {
            assert_eq!(col.last().unwrap().id, card.id);
            assert!(col.last().unwrap().open);
        }
        assert_eq!(g.state.moves, 1);
        assert!(g.undo());
        assert_eq!(g, before);
    }
    #[test]
    fn collection_cascades_and_flips() {
        let mut col = vec![Card { id: 6, open: false }];
        col.extend((1..=13).rev().map(card));
        col.extend((2..=13).rev().map(card));
        let mut g = position(vec![col, vec![card(1)]]);
        let before = g.clone();
        g.apply(Move {
            from: 1,
            start: 0,
            to: 0,
        })
        .unwrap();
        assert_eq!(g.state.completed.len(), 2);
        assert_eq!(g.state.columns[0], vec![card(7)]);
        g.undo();
        assert_eq!(g, before);
    }
    #[test]
    fn deal_collects_two_runs_atomically() {
        let mut g = position(vec![
            (2..=13).rev().map(card).collect(),
            (2..=13).rev().map(card).collect(),
        ]);
        for i in 2..10 {
            g.state.columns[i] = vec![card(9)];
        }
        g.state.stock = (0..10)
            .map(|i| Card {
                id: if i < 2 { 0 } else { 4 },
                open: false,
            })
            .collect();
        let before = g.clone();
        g.deal().unwrap();
        assert_eq!(g.state.completed.len(), 2);
        assert_eq!(g.state.moves, 1);
        g.undo();
        assert_eq!(g, before);
    }
    #[test]
    fn victory_survives_restore_and_undo() {
        let mut g = position(vec![]);
        g.state.completed = (0..7)
            .map(|copy| {
                (0..13)
                    .rev()
                    .map(|rank| Card {
                        id: copy * 13 + rank,
                        open: true,
                    })
                    .collect()
            })
            .collect();
        g.state.columns[0] = (1..13)
            .rev()
            .map(|rank| Card {
                id: 91 + rank,
                open: true,
            })
            .collect();
        g.state.columns[1] = vec![Card { id: 91, open: true }];
        assert!(g.validate().is_ok());
        let before = g.clone();
        g.apply(Move {
            from: 1,
            start: 0,
            to: 0,
        })
        .unwrap();
        assert!(g.state.won());
        let mut restored = Game::restore(&serde_json::to_string(&g).unwrap()).unwrap();
        assert!(restored.state.won());
        assert_eq!(restored.deal(), Err(Error::Won));
        restored.undo();
        assert_eq!(restored, before);
    }
    #[test]
    fn corrupted_saves_rejected() {
        let mut g = Game::new(1);
        g.version = 99;
        assert!(g.validate().is_err());
        g.version = VERSION;
        g.state.stock[0].id = g.state.stock[1].id;
        assert!(g.validate().is_err());
        assert!(Game::restore("{}").is_err());
        let mut g = Game::new(1);
        g.history.push(g.state.clone());
        g.history[0].columns[0][0].open = true;
        assert!(g.validate().is_err());
    }
    #[test]
    fn no_wrap_and_hints_share_legality() {
        let g = position(vec![
            vec![card(13)],
            vec![card(1)],
            vec![card(7), card(6), card(5)],
            vec![card(8)],
        ]);
        assert!(!g.legal(Move {
            from: 0,
            start: 0,
            to: 1
        }));
        let m = g.hint().unwrap();
        assert!(g.legal(m));
    }
    #[test]
    fn history_limit_and_branching() {
        let mut g = sample();
        g.state.columns[0][0].id = 7;
        for _ in 0..120 {
            g.apply(Move {
                from: 0,
                start: 1,
                to: 2,
            })
            .unwrap();
            g.apply(Move {
                from: 2,
                start: 0,
                to: 0,
            })
            .unwrap();
        }
        assert_eq!(g.history.len(), HISTORY_LIMIT);
        g.undo();
        let moves = g.state.moves;
        g.apply(Move {
            from: 2,
            start: 0,
            to: 3,
        })
        .unwrap();
        assert_eq!(g.state.moves, moves + 1);
        assert_eq!(g.history.len(), HISTORY_LIMIT);
    }
    #[test]
    fn generated_play_preserves_cards_and_save_history() {
        for seed in 0..12 {
            let mut g = Game::new_with_mode(seed, Mode::ALL[seed as usize % 3]);
            for n in 0..160 {
                let before = g.clone();
                if n % 7 == 0 && g.deal().is_ok() {
                } else if let Some(m) = g.hint() {
                    g.apply(m).unwrap();
                } else {
                    break;
                }
                assert!(g.validate().is_ok());
                if n % 13 == 0 {
                    let restored = Game::restore(&serde_json::to_string(&g).unwrap()).unwrap();
                    assert_eq!(g, restored);
                }
                if n % 11 == 0 {
                    g.undo();
                    assert_eq!(g.state, before.state);
                }
            }
        }
    }
    fn suited(rank: u8, suit: u8) -> Card {
        Card {
            id: suit * 13 + rank - 1,
            open: true,
        }
    }
    #[test]
    fn suit_counts_and_seed_are_stable_in_all_modes() {
        for mode in Mode::ALL {
            let g = Game::new_with_mode(42, mode);
            assert_eq!(g, Game::new_with_mode(42, mode));
            assert!(g.validate().is_ok());
            let mut counts = [0; 4];
            for c in g.state.columns.iter().flatten().chain(g.state.stock.iter()) {
                counts[(c.id / 13 % mode.count()) as usize] += 1;
            }
            for n in 0..4 {
                assert_eq!(
                    counts[n],
                    if n < mode.count() as usize {
                        104 / mode.count() as usize
                    } else {
                        0
                    }
                );
            }
        }
    }
    #[test]
    fn mixed_suit_tail_cannot_move_but_single_and_suited_tails_can() {
        for mode in [Mode::Two, Mode::Four] {
            let mut g = position(vec![
                vec![suited(7, 0), suited(6, 1)],
                vec![suited(8, 1)],
                vec![suited(7, 0)],
            ]);
            g.state.mode = mode;
            let before = g.clone();
            assert!(!g.selectable(0, 0));
            assert!(
                g.apply(Move {
                    from: 0,
                    start: 0,
                    to: 1
                })
                .is_err()
            );
            assert_eq!(g, before);
            g.apply(Move {
                from: 0,
                start: 1,
                to: 2,
            })
            .unwrap();
            g.undo();
            assert_eq!(g, before);
            g.state.columns[0][1] = suited(6, 0);
            g.apply(Move {
                from: 0,
                start: 0,
                to: 1,
            })
            .unwrap();
            assert!(!g.selectable(1, 0));
            assert!(g.selectable(1, 1));
        }
    }
    #[test]
    fn only_same_suit_runs_collect_and_undo_restores_mode() {
        for mode in [Mode::Two, Mode::Four] {
            let mut g = position(vec![
                (2..=13).rev().map(|r| suited(r, 0)).collect(),
                vec![suited(1, 1)],
            ]);
            g.state.mode = mode;
            g.apply(Move {
                from: 1,
                start: 0,
                to: 0,
            })
            .unwrap();
            assert!(g.state.completed.is_empty());
            g.undo();
            g.state.columns[1][0] = suited(1, 0);
            let before = g.clone();
            g.apply(Move {
                from: 1,
                start: 0,
                to: 0,
            })
            .unwrap();
            assert_eq!(g.state.completed.len(), 1);
            assert_eq!(g.state.mode, mode);
            g.undo();
            assert_eq!(g, before);
        }
    }
    #[test]
    fn restores_legacy_save_without_changing_cards_or_history() {
        let mut g = Game::new(31);
        g.deal().unwrap();
        let mut legacy = serde_json::to_value(&g).unwrap();
        legacy["version"] = 1.into();
        legacy.as_object_mut().unwrap().remove("deal");
        g.deal = None;
        legacy["state"].as_object_mut().unwrap().remove("mode");
        for state in legacy["history"].as_array_mut().unwrap() {
            state.as_object_mut().unwrap().remove("mode");
        }
        assert_eq!(Game::restore(&legacy.to_string()).unwrap(), g);
        legacy["version"] = VERSION.into();
        assert!(Game::restore(&legacy.to_string()).is_err());
    }
    #[test]
    fn rejects_wrong_mode_history_and_unsuited_completed_series() {
        let mut g = Game::new_with_mode(7, Mode::Four);
        g.deal().unwrap();
        g.history[0].mode = Mode::Two;
        assert!(Game::restore(&serde_json::to_string(&g).unwrap()).is_err());
        // Valid complete deck, but exchanging aces makes two completed runs mixed.
        let mut g = Game::new_with_mode(7, Mode::Four);
        g.state.columns = std::array::from_fn(|_| vec![]);
        g.state.stock.clear();
        g.state.completed = (0..8)
            .map(|copy| {
                (0..13)
                    .rev()
                    .map(|r| Card {
                        id: copy * 13 + r,
                        open: true,
                    })
                    .collect()
            })
            .collect();
        assert!(g.validate().is_ok());
        let a = g.state.completed[0][12];
        g.state.completed[0][12] = g.state.completed[1][12];
        g.state.completed[1][12] = a;
        assert!(g.validate().is_err());
    }
    #[test]
    fn hints_prefer_suited_joins_over_other_suit_joins() {
        let mut g = position(vec![
            vec![suited(7, 0)],
            vec![suited(8, 1)],
            vec![suited(8, 0)],
        ]);
        g.state.mode = Mode::Four;
        assert_eq!(
            g.hint(),
            Some(Move {
                from: 0,
                start: 0,
                to: 2
            })
        );
    }
    fn blocked(mut columns: Vec<Vec<Card>>, mode: Mode) -> Game {
        columns.resize_with(10, || vec![card(1)]);
        let mut g = position(columns);
        g.state.mode = mode;
        g
    }
    #[test]
    fn hint_does_not_split_a_suited_run_to_another_king() {
        for mode in Mode::ALL {
            for king_suit in 0..mode.count() {
                let g = blocked(
                    vec![
                        vec![suited(13, 0), suited(12, 0), suited(11, 0), suited(10, 0)],
                        vec![suited(13, king_suit)],
                    ],
                    mode,
                );
                assert!(g.legal(Move {
                    from: 0,
                    start: 1,
                    to: 1
                }));
                assert_eq!(g.hint(), None);
            }
        }
    }
    #[test]
    fn hint_keeps_collecting_and_exposing_hidden_cards() {
        let mut g = blocked(
            vec![
                vec![
                    Card {
                        id: 40,
                        open: false,
                    },
                    suited(12, 1),
                    suited(11, 1),
                    suited(10, 1),
                ],
                vec![suited(13, 0)],
            ],
            Mode::Two,
        );
        assert_eq!(
            g.hint(),
            Some(Move {
                from: 0,
                start: 1,
                to: 1
            })
        );
        g = blocked(
            vec![
                (2..=13).rev().map(|r| suited(r, 1)).collect(),
                vec![suited(1, 1)],
            ],
            Mode::Two,
        );
        let m = g.hint().unwrap();
        g.apply(m).unwrap();
        assert_eq!(g.state.completed.len(), 1);
    }
    #[test]
    fn hint_does_not_shuttle_whole_columns_between_empty_spaces() {
        let g = position(vec![vec![card(13), card(12), card(11), card(10)]]);
        assert!(g.legal(Move {
            from: 0,
            start: 0,
            to: 1
        }));
        assert_eq!(g.hint(), None);
    }
    #[test]
    fn hint_can_fill_empty_space_to_enable_dealing() {
        let mut g = blocked(
            vec![vec![card(13), card(12), card(11), card(10)], vec![]],
            Mode::One,
        );
        g.state.stock = vec![
            Card {
                id: 50,
                open: false
            };
            10
        ];
        let m = g.hint().unwrap();
        assert_eq!(m.to, 1);
        assert!(m.start > 0);
        g.apply(m).unwrap();
        assert!(g.state.columns.iter().all(|c| !c.is_empty()));
    }
    #[test]
    fn hints_skip_earlier_positions_even_with_other_card_copies_or_columns() {
        let mut g = blocked(vec![vec![card(7)], vec![card(8)]], Mode::One);
        let mut previous = g.clone();
        previous
            .apply(Move {
                from: 0,
                start: 0,
                to: 1,
            })
            .unwrap();
        previous.state.moves = 99;
        previous.state.columns.swap(1, 4);
        for card in previous.state.columns.iter_mut().flatten() {
            card.id += 13;
        }
        g.history.push(previous.state);
        assert_eq!(g.hint(), None);
    }
    #[test]
    fn repeated_hints_do_not_return_to_equivalent_positions_in_generated_play() {
        for mode in Mode::ALL {
            for seed in 0..8 {
                let mut g = Game::new_with_mode(seed, mode);
                let mut seen = vec![position_key(&g.state)];
                for step in 0..80 {
                    if step % 9 == 0 && g.deal().is_ok() {
                        seen.push(position_key(&g.state));
                        continue;
                    }
                    let Some(m) = g.hint() else { break };
                    g.apply(m).unwrap();
                    let key = position_key(&g.state);
                    assert!(!seen.contains(&key));
                    seen.push(key);
                    g.validate().unwrap();
                }
            }
        }
    }
    #[test]
    fn selection_targets_match_accepting_commands_in_all_modes() {
        for mode in Mode::ALL {
            let heart = if mode == Mode::One { 0 } else { 1 };
            let g = position(vec![
                vec![suited(8, 0), suited(7, heart), suited(6, heart)],
                vec![suited(8, 0)],
                vec![suited(8, heart)],
                vec![],
            ]);
            let mut g = g;
            g.state.mode = mode;
            assert!(g.selectable(0, 1));
            assert_eq!(g.selectable(0, 0), mode == Mode::One);
            for start in [1, 2] {
                let targets = g.targets(0, start);
                for to in 0..10 {
                    let mut trial = g.clone();
                    assert_eq!(
                        targets.contains(&to),
                        trial.apply(Move { from: 0, start, to }).is_ok()
                    );
                }
            }
            assert!(g.targets(0, 1).contains(&1));
            assert!(g.targets(0, 1).contains(&2));
            assert!(g.targets(0, 1).contains(&3));
            assert!(!g.targets(0, 2).contains(&1));
        }
    }
    #[test]
    fn same_color_is_not_same_suit() {
        for (a, b) in [(0, 2), (1, 3)] {
            let mut g = position(vec![vec![suited(7, a), suited(6, b)]]);
            g.state.mode = Mode::Four;
            assert!(!g.selectable(0, 0));
            assert!(g.selectable(0, 1));
        }
    }
}
