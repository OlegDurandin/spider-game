use spider_engine::{
    Card, Game as Spider, Mode, PublicDeal, Session,
    klondike::{Command, Game, Source, Status, Target},
};
fn c(id: u8) -> Card {
    Card { id, open: true }
}
fn position(columns: Vec<Vec<Card>>, waste: Vec<Card>, foundations: [Vec<Card>; 4]) -> Game {
    let mut g = Game::new(0x0123456789abcdef);
    g.state.columns = std::array::from_fn(|i| columns.get(i).cloned().unwrap_or_default());
    let used: Vec<_> = g
        .state
        .columns
        .iter()
        .flatten()
        .chain(waste.iter())
        .chain(foundations.iter().flatten())
        .map(|c| c.id)
        .collect();
    g.state.stock = (0..52)
        .filter(|id| !used.contains(id))
        .map(|id| Card { id, open: false })
        .collect();
    g.state.waste = waste;
    g.state.foundations = foundations;
    g.state.status = Status::Playing;
    g.validate().unwrap();
    g
}
fn main() {
    let dir = std::env::args().nth(1).map(std::path::PathBuf::from);
    if let Some(p) = &dir {
        std::fs::create_dir_all(p).unwrap();
    }
    let mut fixtures: Vec<(&str, Session)> = vec![];
    let code = "k1_1_0123456789abcdef";
    let start = PublicDeal::parse(code).unwrap().start().unwrap();
    assert_eq!(start.deal_code().as_deref(), Some(code));
    start.validate().unwrap();
    fixtures.push(("start", start.clone()));
    let mut ordinary = Game::new(42);
    ordinary.apply(Command::Draw).unwrap();
    fixtures.push(("ordinary", ordinary.into()));
    let tail = position(
        vec![
            vec![c(8), c(20), c(32)],
            vec![c(34)],
            vec![c(47)],
            vec![c(46)],
        ],
        vec![],
        std::array::from_fn(|_| vec![]),
    );
    fixtures.push(("tail", tail.into()));
    let mut f = std::array::from_fn(|_| vec![]);
    f[3] = (39..43).map(c).collect();
    f[1] = (13..17).map(c).collect();
    let dual = position(vec![vec![c(31)]], vec![c(43)], f);
    fixtures.push(("dual", dual.into()));
    let f = std::array::from_fn(|i| {
        (i as u8 * 13..i as u8 * 13 + if i == 3 { 12 } else { 13 })
            .map(c)
            .collect()
    });
    let mut win = position(vec![vec![c(51)]], vec![], f);
    fixtures.push(("before-win", win.clone().into()));
    win.apply(Command::Move {
        from: Source::Tableau {
            column: 0,
            start: 0,
        },
        to: Target::Foundation(3),
    })
    .unwrap();
    assert!(win.state.won());
    fixtures.push(("won", win.into()));
    let mut redeal = start.klondike().unwrap().clone();
    for _ in 0..24 {
        redeal.apply(Command::Draw).unwrap();
    }
    assert!(redeal.state.stock.is_empty());
    fixtures.push(("redeal-ready", redeal.clone().into()));
    redeal.apply(Command::Draw).unwrap();
    fixtures.push(("redealt", redeal.into()));
    for (name, g) in fixtures {
        g.validate().unwrap();
        let json = serde_json::to_string(&g).unwrap();
        assert_eq!(Session::restore(&json).unwrap(), g);
        if let Some(p) = &dir {
            std::fs::write(p.join(format!("klondike-{name}.json")), json).unwrap();
        }
        println!(
            "{name}: valid 52-card state, {} moves, {} undo",
            g.moves(),
            g.history_len()
        );
    }
    if let Some(p) = dir {
        let mut old = Spider::new_with_mode(42, Mode::Four);
        old.deal().unwrap();
        let mut v = serde_json::to_value(old).unwrap();
        v["version"] = 2.into();
        v.as_object_mut().unwrap().remove("deal");
        std::fs::write(p.join("spider-legacy4.json"), v.to_string()).unwrap();
    }
}
