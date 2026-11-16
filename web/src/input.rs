//! Gesture decisions are independent of the DOM and share the engine's legality checks.
use spider_engine::Game;

pub const HOLD_MS: f64 = 220.;
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}
#[derive(Clone, Copy, Debug, Default)]
pub struct Bounds {
    pub left: f64,
    pub top: f64,
    pub right: f64,
    pub bottom: f64,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Intent {
    Pending,
    Scroll,
    Drag,
}
pub fn intent(
    current: Intent,
    touch: bool,
    can_drag: bool,
    start: Point,
    point: Point,
    elapsed: f64,
) -> Intent {
    if current != Intent::Pending {
        return current;
    }
    let dx = (point.x - start.x).abs();
    let dy = (point.y - start.y).abs();
    if touch && can_drag && elapsed >= HOLD_MS {
        return Intent::Drag;
    }
    if dx.max(dy) < if touch { 8. } else { 6. } {
        return Intent::Pending;
    }
    if !touch {
        return if can_drag {
            Intent::Drag
        } else {
            Intent::Pending
        };
    }
    if can_drag && dx > dy * 1.15 {
        Intent::Drag
    } else {
        Intent::Scroll
    }
}
pub fn column_at(point: Point, table: Bounds, columns: &[(f64, f64)]) -> Option<usize> {
    if point.x < table.left
        || point.x > table.right
        || point.y < table.top
        || point.y > table.bottom
    {
        return None;
    }
    columns
        .iter()
        .enumerate()
        .min_by(|(_, a), (_, b)| {
            (point.x - (a.0 + a.1) / 2.)
                .abs()
                .total_cmp(&(point.x - (b.0 + b.1) / 2.).abs())
        })
        .map(|(i, _)| i)
}
pub fn scroll_delta(point: Point, table: Bounds) -> i32 {
    if point.x < table.left || point.x > table.right {
        return 0;
    }
    if point.y >= table.top - 20. && point.y < table.top + 28. {
        -7
    } else if point.y <= table.bottom + 20. && point.y > table.bottom - 28. {
        7
    } else {
        0
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Dialog {
    None,
    Pause,
    ConfirmNew,
    Corrupt,
    Actions,
    Repeat,
    Parties,
    Preview,
    Share,
    Help,
}
pub fn replaces_session_progress(game: Option<&spider_engine::Session>) -> bool {
    game.is_some_and(|g| !g.won() && g.moves() > 0)
}
pub fn session_replacement_acknowledged(
    reviewed: Option<&spider_engine::Session>,
    current: Option<&spider_engine::Session>,
) -> bool {
    !replaces_session_progress(current) || replaces_session_progress(reviewed)
}
/// Explain the actual boundary that prevents lifting the chosen tail.
pub fn selection_error(game: &Game, column: usize, start: usize) -> &'static str {
    let Some(cards) = game.state.columns.get(column).and_then(|c| c.get(start..)) else {
        return "Выберите открытую карту";
    };
    if cards.iter().any(|c| !c.open) {
        "Сначала откройте карту"
    } else if cards
        .windows(2)
        .any(|p| p[0].suit(game.state.mode) != p[1].suit(game.state.mode))
    {
        "Вместе можно переносить карты одной масти"
    } else {
        "Карты в группе должны идти по порядку: от старшей к младшей"
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    const ORIGIN: Point = Point { x: 100., y: 200. };
    #[test]
    fn tap_and_small_finger_jitter_remain_taps() {
        assert_eq!(
            intent(
                Intent::Pending,
                true,
                true,
                ORIGIN,
                Point { x: 104., y: 206. },
                90.
            ),
            Intent::Pending
        );
    }
    #[test]
    fn horizontal_drag_and_held_vertical_drag_are_supported() {
        assert_eq!(
            intent(
                Intent::Pending,
                true,
                true,
                ORIGIN,
                Point { x: 120., y: 203. },
                80.
            ),
            Intent::Drag
        );
        assert_eq!(
            intent(Intent::Pending, true, true, ORIGIN, ORIGIN, 221.),
            Intent::Drag
        );
        assert_eq!(
            intent(
                Intent::Pending,
                true,
                true,
                ORIGIN,
                Point { x: 101., y: 260. },
                300.
            ),
            Intent::Drag
        );
        assert_eq!(
            intent(
                Intent::Pending,
                false,
                true,
                ORIGIN,
                Point { x: 101., y: 230. },
                20.
            ),
            Intent::Drag
        );
    }
    #[test]
    fn vertical_scroll_never_turns_into_a_move_after_holding() {
        let mode = intent(
            Intent::Pending,
            true,
            true,
            ORIGIN,
            Point { x: 102., y: 250. },
            40.,
        );
        assert_eq!(mode, Intent::Scroll);
        assert_eq!(
            intent(mode, true, true, ORIGIN, Point { x: 160., y: 290. }, 800.),
            Intent::Scroll
        );
        assert_eq!(
            intent(
                Intent::Pending,
                true,
                false,
                ORIGIN,
                Point { x: 150., y: 205. },
                500.
            ),
            Intent::Scroll
        );
    }
    #[test]
    fn drop_uses_the_whole_visible_column_including_empty_space() {
        let bounds = Bounds {
            left: 0.,
            top: 100.,
            right: 390.,
            bottom: 600.,
        };
        let cols: Vec<_> = (0..10)
            .map(|i| (i as f64 * 39., i as f64 * 39. + 34.))
            .collect();
        assert_eq!(
            column_at(Point { x: 254., y: 590. }, bounds, &cols),
            Some(6)
        );
        assert_eq!(column_at(Point { x: 254., y: 610. }, bounds, &cols), None);
        assert_eq!(column_at(Point { x: -3., y: 250. }, bounds, &cols), None);
    }
    #[test]
    fn new_game_warns_only_for_unfinished_ordinary_progress_and_rechecks_at_start() {
        let initial = spider_engine::Session::Spider(Game::new(42));
        let mut changed = Game::new(42);
        changed.deal().unwrap();
        let ordinary = spider_engine::Session::Spider(changed);
        assert!(!replaces_session_progress(None));
        assert!(!replaces_session_progress(Some(&initial)));
        assert!(replaces_session_progress(Some(&ordinary)));
        assert!(!session_replacement_acknowledged(None, Some(&ordinary)));
        assert!(!session_replacement_acknowledged(
            Some(&initial),
            Some(&ordinary)
        ));
        assert!(session_replacement_acknowledged(
            Some(&ordinary),
            Some(&ordinary)
        ));
        assert!(session_replacement_acknowledged(Some(&ordinary), None));
        let id = spider_engine::DealId::Published { index: 0 };
        let mut won = id.start().unwrap();
        for command in spider_engine::deal::catalog(0).unwrap().solution {
            command.apply(&mut won).unwrap();
        }
        let won = spider_engine::Session::Spider(won);
        assert!(!replaces_session_progress(Some(&won)));
        assert!(session_replacement_acknowledged(None, Some(&won)));
    }
    #[test]
    fn selection_explains_suit_and_rank_boundaries_separately() {
        use spider_engine::{Card, Mode};
        let mut g = Game::new_with_mode(42, Mode::Four);
        let card = |id| Card { id, open: true };
        g.state.columns[0] = vec![card(7), card(19), card(18)];
        assert_eq!(
            selection_error(&g, 0, 0),
            "Вместе можно переносить карты одной масти"
        );
        g.state.columns[0] = vec![card(7), card(5)];
        assert!(selection_error(&g, 0, 0).contains("по порядку"));
        g.state.columns[0] = vec![card(6), card(31)];
        assert_eq!(
            selection_error(&g, 0, 0),
            "Вместе можно переносить карты одной масти"
        );
    }
}
