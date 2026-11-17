mod account;
mod drag;
mod help;
mod input;
mod klondike;
mod motion;
mod release;
mod save_queue;
mod tripeaks;
use input::Dialog;
use leptos::prelude::*;
use spider_engine::{Game, Kind, Move, Session, label};
use wasm_bindgen::prelude::*;

type Selection = Option<(usize, usize)>;
fn seed() -> u64 {
    (js_sys::Math::random() * (u64::MAX as f64)) as u64
}
fn persist(game: &Game) -> Result<(), ()> {
    persist_session(&Session::Spider(game.clone()))
}
fn persist_session(game: &Session) -> Result<(), ()> {
    release::record_result(game);
    account::persist(game)
}
fn saved() -> Result<Option<Session>, &'static str> {
    account::saved()
}
fn demo() -> Game {
    let mut g = Game::new(765);
    // A valid 104-card fixture: 7–6–5 in column four and an exposed 8 in seven.
    for (col, offset, rank) in [(3, 3, 7), (3, 2, 6), (3, 1, 5), (6, 1, 8), (8, 1, 9)] {
        let pos = g.state.columns[col].len() - offset;
        let old = g.state.columns[col][pos].id;
        let id = rank - 1;
        for c in g
            .state
            .columns
            .iter_mut()
            .flatten()
            .chain(g.state.stock.iter_mut())
        {
            if c.id == id {
                c.id = old;
            } else if c.id == old {
                c.id = id;
            }
        }
        g.state.columns[col][pos].open = true;
    }
    g.deal = None;
    g
}
fn command(
    game: RwSignal<Game>,
    selection: RwSignal<Selection>,
    notice: RwSignal<String>,
    op: impl FnOnce(&mut Game) -> Result<(), spider_engine::Error>,
) {
    if motion::busy() {
        return;
    }
    let mut g = game.get_untracked();
    let before = motion::capture(&g.state);
    drag::cancel();
    match op(&mut g) {
        Ok(()) => {
            if persist(&g).is_err() {
                notice.set("В этой сессии прогресс не сохраняется".into());
            } else {
                notice.set(String::new());
            }
            let after = g.state.clone();
            game.set(g);
            selection.set(None);
            motion::play(before, after);
        }
        Err(e) => notice.set(e.message().into()),
    }
}
fn tap(
    col: usize,
    card: Option<usize>,
    game: RwSignal<Game>,
    selection: RwSignal<Selection>,
    notice: RwSignal<String>,
) {
    if motion::busy() {
        return;
    }
    let g = game.get_untracked();
    if let Some((from, start)) = selection.get_untracked() {
        let m = Move {
            from,
            start,
            to: col,
        };
        if g.legal(m) {
            command(game, selection, notice, |g| g.apply(m));
            return;
        }
        if from == col && card == Some(start) {
            selection.set(None);
            notice.set(String::new());
            return;
        }
    }
    if let Some(start) = card {
        if g.selectable(col, start) {
            selection.set(Some((col, start)));
            notice.set(String::new());
        } else {
            selection.set(None);
            notice.set(input::selection_error(&g, col, start).into());
        }
    } else {
        selection.set(None);
    }
}
fn action_text(g: &Game, selection: Selection) -> String {
    let Some((from, start)) = selection else {
        return "Коснитесь карты или удерживайте и перетащите".into();
    };
    let targets = g.targets(from, start);
    if targets.len() == 1 {
        match g.state.columns[targets[0]].last().map(|c| c.rank()) {
            None => "Коснитесь пустого столбца".into(),
            Some(r) => format!(
                "Коснитесь {}",
                match r {
                    1 => "туза",
                    2 => "двойки",
                    3 => "тройки",
                    4 => "четвёрки",
                    5 => "пятёрки",
                    6 => "шестёрки",
                    7 => "семёрки",
                    8 => "восьмёрки",
                    9 => "девятки",
                    10 => "десятки",
                    11 => "валета",
                    12 => "дамы",
                    _ => "короля",
                }
            ),
        }
    } else if targets.is_empty() {
        "Нет подходящего столбца. Выберите другие карты".into()
    } else {
        "Выберите подсвеченный столбец".into()
    }
}
#[component]
fn App() -> impl IntoView {
    let is_demo = account::key() == "spider.demo.v1";
    let loaded = if is_demo {
        Ok(Some(Session::Spider(demo())))
    } else {
        saved()
    };
    let had_saved = matches!(&loaded, Ok(Some(_)));
    let corrupt = loaded.as_ref().is_err_and(|e| *e != "storage");
    let storage_failed = loaded.as_ref().is_err_and(|e| *e == "storage");
    let session = RwSignal::new(loaded.ok().flatten().unwrap_or_else(|| {
        Session::new(
            if account::slot() == "klondike_regular" || account::slot().starts_with("challenge_k") {
                Kind::Klondike
            } else if account::slot() == "tripeaks_regular"
                || account::slot().starts_with("challenge_t")
            {
                Kind::Tripeaks
            } else {
                Kind::Spider
            },
            seed(),
            spider_engine::Mode::One,
        )
    }));
    let game = RwSignal::new(
        session
            .get_untracked()
            .spider()
            .cloned()
            .unwrap_or_else(|| Game::new(0)),
    );
    // The existing Spider board and drag layer keep their typed engine state.
    // Only the active Spider branch can publish changes to the shared session.
    Effect::new(move |_| {
        if let Session::Spider(next) = session.get() {
            if game.get_untracked() != next {
                game.set(next);
            }
        }
    });
    Effect::new(move |_| {
        let next = game.get();
        if session.get_untracked().kind() == Kind::Spider
            && session.get_untracked().spider() != Some(&next)
        {
            session.set(Session::Spider(next));
        }
    });
    let new_mode = RwSignal::new(game.get_untracked().state.mode);
    let selection = RwSignal::new(None::<(usize, usize)>);
    let notice = RwSignal::new(if storage_failed {
        "В этой сессии прогресс не сохраняется".into()
    } else {
        String::new()
    });
    let dialog = RwSignal::new(if corrupt {
        Dialog::Corrupt
    } else {
        Dialog::None
    });
    let account_note = RwSignal::new(account::status());
    let retry_sync = RwSignal::new(false);
    let legacy_available = RwSignal::new(account::has_legacy());
    let import_confirm = RwSignal::new(false);
    account::attach(account_note, retry_sync);
    let no_moves = RwSignal::new(false);
    Effect::new(move |_| {
        game.track();
        no_moves.set(false);
    });
    let active_slot = RwSignal::new(account::slot());
    let loading = RwSignal::new(false);
    let future_kind = RwSignal::new(session.get_untracked().kind());
    let release_ui = release::Ui {
        game: session,
        selection,
        notice,
        dialog,
        mode: new_mode,
        slot: active_slot,
        loading,
        future_kind,
        help: RwSignal::new(help::Context::capture(
            session.get_untracked().kind(),
            session.get_untracked().spider_mode(),
            future_kind.get_untracked(),
            new_mode.get_untracked(),
            Dialog::None,
        )),
    };
    release::record_result(&session.get_untracked());
    let animating = RwSignal::new(false);
    motion::attach(animating);
    let dragging = RwSignal::new(false);
    let hover = RwSignal::new(None::<usize>);
    let drag_controls = drag::Controls {
        game,
        selection,
        notice,
        hover,
        dragging,
        dialog,
    };
    drag::install(drag_controls);
    if !corrupt
        && !is_demo
        && (!account::is_cloud() || !had_saved)
        && persist_session(&session.get_untracked()).is_err()
    {
        notice.set("В этой сессии прогресс не сохраняется".into());
    }
    account::start();
    let restore = Closure::<dyn FnMut()>::new(move || {
        if !web_sys::window().unwrap().document().unwrap().hidden() && !is_demo {
            if account::is_cloud() {
                account::start();
                wasm_bindgen_futures::spawn_local(async move {
                    if let Some(g) = account::refresh().await {
                        drag::cancel();
                        motion::cancel();
                        selection.set(None);
                        new_mode.set(g.spider_mode());
                        session.set(g);
                    }
                });
                return;
            }
            match saved() {
                Ok(Some(g)) => {
                    drag::cancel();
                    motion::cancel();
                    session.set(g);
                    selection.set(None);
                }
                Err("storage") => notice.set("В этой сессии прогресс не сохраняется".into()),
                Err(_) => dialog.set(Dialog::Corrupt),
                _ => {}
            }
        }
    });
    web_sys::window()
        .unwrap()
        .document()
        .unwrap()
        .add_event_listener_with_callback("visibilitychange", restore.as_ref().unchecked_ref())
        .ok();
    restore.forget();
    let ready = web_sys::Event::new("spider-ready").unwrap();
    web_sys::window().unwrap().dispatch_event(&ready).ok();
    view! {
        <main class="game">
            <div class="game-surface" inert=move ||dialog.get()!=Dialog::None || import_confirm.get() || (session.get().finished() && !animating.get())>
            <header>
                <div class="brand"><Show when=move||session.get().kind()==Kind::Spider><span class="spider" aria-hidden="true">"♜"</span></Show><h1>{move||session.get().kind().name()}</h1><Show when=move||session.get().kind()==Kind::Spider><span class="mode">{move ||game.get().state.mode.label()}</span></Show></div>
                <div class="header-actions">
                    <button class="new-game" disabled=move||loading.get() aria-label="Новая игра" on:click=move |_| {release_ui.open_new();}>
                        <span aria-hidden="true">"↻"</span><span>"Новая"<br/>"игра"</span>
                    </button>
                    <button class="help-entry" disabled=move||loading.get() || animating.get() || dragging.get() aria-label="Как играть" on:click=move |_|release_ui.open_help()>"?"</button>
                    <button class="pause" disabled=move||loading.get() aria-label="Пауза" on:click=move |_| {drag::cancel();motion::cancel();dialog.set(Dialog::Pause);}>"Ⅱ"</button>
                </div>
            </header>
            <div class="account-bar" aria-live="polite"><span class="account-status">{move ||account_note.get()}</span><div class="account-actions">
                <Show when=move ||retry_sync.get()><button class="text-button" on:click=move |_|account::retry()>{move ||{account_note.get();if account::conflicted(){"Загрузить с сервера"}else{"Повторить отправку"}}}</button></Show>
                <Show when=move ||legacy_available.get() && active_slot.get()=="regular"><button class="text-button" on:click=move |_|{drag::cancel();motion::cancel();import_confirm.set(true);}>"Перенести прежнюю партию"</button></Show>
            </div></div>
            <Show when=move||session.get().kind()==Kind::Spider>
            <section class="stats" aria-label="Счёт"><span>"Ходы "<b>{move ||game.get().state.moves}</b></span><span class="collected-count">"Собрано "<b>{move ||game.get().state.completed.len()}"/8"</b></span></section>
            <section class="table-scroll" class:animating=move ||animating.get() class:dragging=move ||dragging.get() aria-label="Игровой стол" on:pointerdown=move |ev|drag::pointer_down(ev,drag_controls) on:contextmenu=move |ev|ev.prevent_default()>
                <div class="columns">
                {move || {
                    let g = game.get(); let selected = selection.get();
                    let targets = selected.map(|(f,s)|g.targets(f,s)).unwrap_or_default();
                    g.state.columns.iter().enumerate().map(|(col,cards)| {
                        let target = targets.contains(&col);
                        let selected_start = selected.filter(|(f,_)|*f==col).map(|(_,s)|s);
                        let mut y = 0;
                        let positions: Vec<_> = cards.iter().map(|c| { let top=y; y += if c.open { 42 } else { 16 }; top }).collect();
                        let height = positions.last().copied().unwrap_or(0)+128;
                        view! {
                            <div class="column" class:target=target class:drop-hover=move ||hover.get()==Some(col) style=format!("height:calc(var(--unit) * {height})") on:click=move |ev|{if !drag::consume_click(&ev) {tap(col,None,game,selection,notice)}}>
                                <span class="empty-mark" aria-hidden="true">"♠"</span>
                                {target.then(||view!{<span class="target-mark" aria-hidden="true">"↓"</span>})}
                                {cards.iter().enumerate().map(|(index,c)| {
                                    let rank=label(c.rank()); let open=c.open; let suit=c.suit(g.state.mode);
                                    let face=if (2..=10).contains(&c.rank()) {format!("cards/numeric-{}.svg",suit.code())} else {format!("cards/art-v2/{}{}.svg",rank,suit.code())};
                                    let accessible=if open {format!("{} {}, столбец {}",rank,suit.name(),col+1)} else {format!("Закрытая карта, столбец {}",col+1)};
                                    let top=positions[index];
                                    view! {
                                        <button class="card" class:back=!open class:red=suit.red() class:ten=(c.rank()==10) style=format!("top:calc(var(--unit) * {top})")
                                            aria-label=accessible data-card-id=c.id draggable="false"
                                            on:click=move |ev| {ev.stop_propagation();if !drag::consume_click(&ev){tap(col,Some(index),game,selection,notice)}}>
                                            {open.then(||view!{<img class="card-face" src=face alt="" draggable="false"/><span class="rank">{rank}<small>{suit.symbol()}</small></span>})}
                                        </button>
                                    }
                                }).collect_view()}
                                {selected_start.map(|s|view!{<div class="selection-ring" style=format!("top:calc(var(--unit) * {});height:calc(var(--unit) * {})",positions[s],height-positions[s])></div>})}
                            </div>
                        }
                    }).collect_view()
                }}
                </div>
                <div class="table-emblem" aria-hidden="true">"♠"</div>
            </section>
            <section class="action" aria-live="polite">
                <p>{move ||if dragging.get() {hover.get().map(|col|format!("Отпустите карты в столбец {}",col+1)).unwrap_or_else(||"Перетащите на подсвеченный столбец".into())}else{action_text(&game.get(),selection.get())}}</p>
                <div class="mini-cards">{move ||selection.get().map(|(f,s)|{let g=game.get();g.state.columns[f][s..].iter().map(|c|{let suit=c.suit(g.state.mode);view!{<span class:red=suit.red()>{label(c.rank())}{suit.symbol()}</span>}}).collect_view()})}</div>
                <p class="notice">{move ||notice.get()}</p>
                <Show when=move ||no_moves.get()><button class="text-button" on:click=move |_|{release_ui.open_new();}>"Новая игра"</button></Show>
            </section>
            <footer>
                <button disabled=move ||game.get().history.is_empty() || animating.get() || dragging.get() on:click=move |_| { no_moves.set(false);command(game,selection,notice,|g|{g.undo();Ok(())}); }><span class="tool-icon">"↶"</span><span>"Отменить"</span></button>
                <button disabled=move ||animating.get() || dragging.get() on:click=move |_| {
                    let g=game.get_untracked();no_moves.set(false);
                    if let Some(m)=g.hint() { selection.set(Some((m.from,m.start)));notice.set("Подсказка: коснитесь подсвеченной цели".into()); }
                    else if !g.state.stock.is_empty() && g.state.columns.iter().all(|c|!c.is_empty()) { selection.set(None);notice.set("Полезных перестановок не найдено. Можно раздать следующий ряд".into()); }
                    else { selection.set(None);notice.set("Полезных перестановок не найдено. Попробуйте другой ход, отмену или новую партию".into());no_moves.set(true); }
                }><span class="tool-icon bulb">"♧"</span><span>"Подсказка"</span></button>
                <button class="deal-button" disabled=move ||animating.get() || dragging.get() on:click=move |_| {no_moves.set(false);command(game,selection,notice,|g|g.deal());}><span class="tool-icon">"▱"</span><span>"Раздать ряд"<small>{move ||format!("Осталось: {}",game.get().state.stock.len()/10)}</small></span></button>
            </footer>
            </Show>
            <Show when=move||session.get().kind()==Kind::Klondike><klondike::Board game=session notice=notice dialog=dialog/></Show>
            <Show when=move||session.get().kind()==Kind::Tripeaks><tripeaks::Board game=session notice=notice dialog=dialog/></Show>
            </div>
            <Show when=move ||import_confirm.get()>
                <div class="overlay"><section class="modal" role="dialog" aria-modal="true" aria-label="Перенос прежней партии">
                    <h2>"Это ваша прежняя партия?"</h2>
                    <p>"На устройстве есть партия без привязки к аккаунту. Перенести её в текущий аккаунт? Текущая партия будет заменена."</p>
                    <button class="primary" on:click=move |_|{
                        if let Some(old)=account::legacy(){drag::cancel();motion::cancel();selection.set(None);new_mode.set(old.spider_mode());session.set(old.clone());if persist_session(&old).is_err(){notice.set("Локальное сохранение недоступно".into());}}
                        account::dismiss_legacy();legacy_available.set(false);import_confirm.set(false);
                    }>"Да, перенести мою партию"</button>
                    <button on:click=move |_|{account::dismiss_legacy();legacy_available.set(false);import_confirm.set(false);}>"Продолжить без переноса"</button>
                </section></div>
            </Show>
            <release::ReleaseMenu ui=release_ui animating=animating/>
        </main>
    }
}
fn main() {
    console_error_panic_hook::set_once();
    wasm_bindgen_futures::spawn_local(async {
        match account::initialize().await {
            Ok(()) => {
                if let Some(el) = web_sys::window()
                    .unwrap()
                    .document()
                    .unwrap()
                    .get_element_by_id("startup")
                {
                    el.remove();
                }
                leptos::mount::mount_to_body(App);
            }
            Err(message) => {
                if let Some(el) = web_sys::window()
                    .unwrap()
                    .document()
                    .unwrap()
                    .get_element_by_id("startup")
                {
                    el.set_text_content(Some(message));
                }
            }
        }
    });
}
