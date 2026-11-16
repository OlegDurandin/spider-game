use spider_engine::{
    Card, Game as Spider, Mode, Session,
    tripeaks::{Command, Game, Status},
};
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
    g.state.status = Status::Playing;
    g.validate().unwrap();
    g
}
fn main() {
    let dir = std::path::PathBuf::from(std::env::args().nth(1).expect("fixture directory"));
    std::fs::create_dir_all(&dir).unwrap();
    let mut s = Spider::new_with_mode(42, Mode::Four);
    s.deal().unwrap();
    let mut k = spider_engine::klondike::Game::new(42);
    k.apply(spider_engine::klondike::Command::Draw).unwrap();
    let mut t = Game::new(42);
    t.apply(Command::Draw).unwrap();
    for (name, g) in [
        ("spider", Session::Spider(s)),
        ("klondike", Session::Klondike(k)),
        ("ordinary", Session::Tripeaks(t)),
        ("start", Session::Tripeaks(Game::new(42))),
        (
            "boundary",
            Session::Tripeaks(position(&[(11, 9), (12, 10), (21, 6)], &[51], 7)),
        ),
        ("last", Session::Tripeaks(position(&[(18, 0)], &[12], 6))),
        ("stuck", Session::Tripeaks(position(&[(18, 0)], &[5], 6))),
    ] {
        g.validate().unwrap();
        let text = serde_json::to_string(&g).unwrap();
        assert_eq!(Session::restore(&text).unwrap(), g);
        std::fs::write(dir.join(format!("{name}.json")), text).unwrap();
    }
}
