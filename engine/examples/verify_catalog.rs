use spider_engine::{DealId, Game, Mode};
fn place(g: &mut Game, col: usize, offset: usize, id: u8) {
    let pos = g.state.columns[col].len() - offset;
    let old = g.state.columns[col][pos].id;
    for c in g
        .state
        .columns
        .iter_mut()
        .flatten()
        .chain(g.state.stock.iter_mut())
    {
        if c.id == id {
            c.id = old
        } else if c.id == old {
            c.id = id
        }
    }
    g.state.columns[col][pos].open = true;
}
fn main() {
    let export = std::env::args().nth(1).map(std::path::PathBuf::from);
    if let Some(dir) = &export {
        std::fs::create_dir_all(dir).unwrap();
    }
    for index in 0..7 {
        let id = DealId::Published { index };
        let mut game = id.start().unwrap();
        let initial = game.clone();
        let commands = spider_engine::deal::catalog(index).unwrap().solution;
        for command in &commands {
            command.apply(&mut game).unwrap();
            game.validate().unwrap();
        }
        assert!(game.state.won());
        println!(
            "{}: {} legal commands, 8/8 complete",
            id.code(),
            commands.len()
        );
        if index == 0 {
            if let Some(dir) = &export {
                for (name, g) in [("published-start", &initial), ("published-win", &game)] {
                    std::fs::write(
                        dir.join(format!("{name}.json")),
                        serde_json::to_vec(g).unwrap(),
                    )
                    .unwrap();
                }
            }
        }
    }
    if let Some(dir) = export {
        for (name, code) in [
            ("compact-ordinary", "s1_1_000000000000002a"),
            ("compact-day", "d1_20261115"),
            ("compact-challenge", "s1_2_0123456789abcdef"),
        ] {
            let mut g = DealId::parse(code).unwrap().start().unwrap();
            g.deal().unwrap();
            std::fs::write(
                dir.join(format!("{name}.json")),
                serde_json::to_vec(&g).unwrap(),
            )
            .unwrap();
        }
        for mode in [Mode::Two, Mode::Four] {
            let mut g = Game::new_with_mode(42, mode);
            g.deal().unwrap();
            let mut v = serde_json::to_value(&g).unwrap();
            v["version"] = 2.into();
            v.as_object_mut().unwrap().remove("deal");
            std::fs::write(
                dir.join(format!("legacy-{}.json", mode.count())),
                serde_json::to_vec(&v).unwrap(),
            )
            .unwrap();
        }
        let mut g = Game::new_with_mode(765, Mode::Two);
        for (col, offset, id) in [
            (3, 3, 7),
            (3, 2, 19),
            (3, 1, 18),
            (6, 1, 33),
            (8, 1, 20),
            (0, 1, 2),
            (1, 1, 15),
            (2, 1, 28),
            (4, 1, 41),
            (5, 1, 54),
            (7, 1, 67),
        ] {
            place(&mut g, col, offset, id);
        }
        let mut closed = std::mem::take(&mut g.state.columns[9]);
        for c in &mut closed {
            c.open = false;
        }
        closed.append(&mut g.state.columns[0]);
        g.state.columns[0] = closed;
        g.deal = None;
        g.validate().unwrap();
        std::fs::write(dir.join("mixed.json"), serde_json::to_vec(&g).unwrap()).unwrap();
    }
}
