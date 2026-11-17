//! Complete synthetic two-suit Spider position for help navigation acceptance.
use spider_engine::{Card, Game, Mode, Session};
fn main() {
    let mut g = Game::new_with_mode(42, Mode::Two);
    g.state.columns = std::array::from_fn(|_| vec![]);
    g.state.stock.clear();
    g.state.columns[0] = [6, 5, 4]
        .into_iter()
        .map(|id| Card { id, open: true })
        .collect();
    g.state.columns[1] = vec![Card { id: 7, open: true }];
    g.state.columns[9] = (0..104)
        .filter(|id| ![4, 5, 6, 7].contains(id))
        .map(|id| Card { id, open: true })
        .collect();
    g.validate().unwrap();
    let json = serde_json::to_string(&Session::Spider(g)).unwrap();
    std::fs::write(std::env::args().nth(1).expect("output JSON"), json).unwrap();
}
