//! Release 1.1 navigation: each kind of attempt has an independent storage slot.
use crate::{account, drag, input::Dialog, motion, persist_session as persist};
use leptos::prelude::*;
use spider_engine::{Kind, Mode, PublicDeal as DealId, Session as Game};
use wasm_bindgen::{JsCast, closure::Closure};
use wasm_bindgen_futures::{JsFuture, spawn_local};

#[derive(Clone, Copy)]
pub struct Ui {
    pub game: RwSignal<Game>,
    pub selection: RwSignal<Option<(usize, usize)>>,
    pub notice: RwSignal<String>,
    pub dialog: RwSignal<Dialog>,
    pub mode: RwSignal<Mode>,
    pub slot: RwSignal<String>,
    pub loading: RwSignal<bool>,
    pub future_kind: RwSignal<Kind>,
    pub help: RwSignal<crate::help::Context>,
}
impl Ui {
    pub fn open_help(self) {
        let from = self.dialog.get_untracked();
        if from == Dialog::Help {
            return;
        }
        let game = self.game.get_untracked();
        self.help.set(crate::help::Context::capture(
            game.kind(),
            game.spider_mode(),
            self.future_kind.get_untracked(),
            self.mode.get_untracked(),
            from,
        ));
        drag::cancel();
        if let Some(w) = web_sys::window() {
            if let Ok(h) = w.history() {
                h.push_state_with_url(&"spider-help".into(), "", None).ok();
            }
        }
        self.dialog.set(Dialog::Help);
    }
    fn close_help(self) {
        self.dialog.set(self.help.get_untracked().return_to);
        if let Some(w) = web_sys::window() {
            if let Ok(h) = w.history() {
                if h.state().ok().and_then(|s| s.as_string()).as_deref() == Some("spider-help") {
                    h.back().ok();
                }
            }
        }
    }

    pub fn open_new(self) {
        drag::cancel();
        motion::cancel();
        self.selection.set(None);
        self.future_kind.set(self.game.get_untracked().kind());
        self.mode.set(self.game.get_untracked().spider_mode());
        self.dialog.set(Dialog::ConfirmNew);
    }
    pub fn undo(self) {
        let mut g = self.game.get_untracked();
        if g.undo() {
            self.selection.set(None);
            if persist(&g).is_err() {
                self.notice.set("Не удалось сохранить партию".into());
            }
            self.game.set(g);
        }
        self.dialog.set(Dialog::None);
    }

    fn install(self, slot: &str, game: Game) -> Result<(), &'static str> {
        if !game.fits_slot(slot) {
            return Err("Партия не соответствует выбранному месту сохранения");
        }
        account::activate(slot)?;
        self.selection.set(None);
        self.mode.set(game.spider_mode());
        self.game.set(game.clone());
        self.slot.set(slot.into());
        self.dialog.set(Dialog::None);
        self.notice.set(String::new());
        persist(&game).map_err(|_| "Не удалось сохранить локальную копию")
    }
    pub fn change(self, slot: String, replacement: Option<Game>) {
        self.load(slot, replacement, true);
    }
    pub fn resume(self, slot: String, fallback: Option<Game>) {
        self.load(slot, fallback, false);
    }
    fn load(self, slot: String, replacement: Option<Game>, force: bool) {
        if self.loading.get_untracked() {
            return;
        }
        self.loading.set(true);
        drag::cancel();
        motion::cancel();
        spawn_local(async move {
            let result = account::prepare(&slot).await;
            match result {
                Ok(saved) => {
                    let game = if force {
                        replacement.or(saved)
                    } else {
                        saved.or(replacement)
                    };
                    if let Some(g) = game {
                        if let Err(e) = self.install(&slot, g) {
                            self.notice.set(e.into());
                        }
                    } else {
                        self.notice.set("Сохранённая партия не найдена".into());
                    }
                }
                Err(e) => self.notice.set(e.into()),
            }
            self.loading.set(false);
        });
    }
}
fn storage() -> Option<web_sys::Storage> {
    web_sys::window()?.local_storage().ok()?
}
pub fn record_result(game: &Game) {
    if !game.won() {
        return;
    }
    let Some(id) = game.deal_id() else { return };
    let key = format!("{}.best.{}", account::base_key(), id.code());
    if let Some(s) = storage() {
        let old = s
            .get_item(&key)
            .ok()
            .flatten()
            .and_then(|v| v.parse::<u32>().ok());
        let score = old.map_or(game.moves(), |old| old.min(game.moves()));
        s.set_item(&key, &score.to_string()).ok();
    }
}
pub fn best(game: &Game) -> Option<u32> {
    let id = game.deal_id()?;
    storage()?
        .get_item(&format!("{}.best.{}", account::base_key(), id.code()))
        .ok()??
        .parse()
        .ok()
}
pub fn label(slot: &str) -> String {
    if slot == "regular" {
        "Паук · Обычная партия".into()
    } else if slot == "tripeaks_regular" {
        "Три пика · Обычная партия".into()
    } else if slot == "klondike_regular" {
        "Косынка · Обычная партия".into()
    } else if let Some(date) = slot.strip_prefix("daily_") {
        format!(
            "Паук · Расклад дня · {}.{}.{}",
            &date[6..],
            &date[4..6],
            &date[..4]
        )
    } else {
        let kind = slot
            .strip_prefix("challenge_")
            .and_then(|s| DealId::parse(s).ok())
            .map_or(Kind::Spider, |id| id.kind());
        format!("{} · Вызов друга", kind.name())
    }
}
fn js(
    name: &str,
    args: &[wasm_bindgen::JsValue],
) -> Result<wasm_bindgen::JsValue, wasm_bindgen::JsValue> {
    let w = web_sys::window().unwrap();
    let f = js_sys::Reflect::get(&w, &name.into())?.dyn_into::<js_sys::Function>()?;
    let array = js_sys::Array::new();
    for a in args {
        array.push(a);
    }
    f.apply(&w, &array)
}
fn incoming() -> Option<String> {
    if let Ok(code) = js("spiderIncomingDeal", &[]) {
        return code.as_string();
    }
    let loc = web_sys::window()?.location();
    let q = web_sys::UrlSearchParams::new_with_str(&loc.search().ok()?).ok()?;
    let h =
        web_sys::UrlSearchParams::new_with_str(loc.hash().ok()?.trim_start_matches('#')).ok()?;
    q.get("deal")
        .or_else(|| q.get("tgWebAppStartParam"))
        .or_else(|| h.get("tgWebAppStartParam"))
        .or_else(|| {
            let raw = h.get("tgWebAppData").or_else(|| q.get("tgWebAppData"))?;
            web_sys::UrlSearchParams::new_with_str(&raw)
                .ok()?
                .get("start_param")
        })
        .or_else(|| loc.hash().ok()?.strip_prefix("#/deal/").map(str::to_string))
}
#[derive(Clone)]
struct Preview {
    id: Result<DealId, String>,
    slot: String,
    saved: Option<Game>,
    ready: bool,
}
fn preview(code: String, daily: bool, ui: Ui, signal: RwSignal<Option<Preview>>) {
    if ui.loading.get_untracked() {
        return;
    }
    drag::cancel();
    motion::cancel();
    ui.selection.set(None);
    ui.dialog.set(Dialog::Preview);
    match DealId::parse(&code) {
        Err(e) => signal.set(Some(Preview {
            id: Err(e.into()),
            slot: String::new(),
            saved: None,
            ready: true,
        })),
        Ok(id) => {
            let slot = if daily {
                match &id {
                    DealId::Spider(spider_engine::DealId::Daily { date }) => {
                        format!("daily_{date}")
                    }
                    _ => return,
                }
            } else {
                format!("challenge_{code}")
            };
            signal.set(Some(Preview {
                id: Ok(id.clone()),
                slot: slot.clone(),
                saved: None,
                ready: false,
            }));
            ui.loading.set(true);
            spawn_local(async move {
                match account::prepare(&slot).await {
                    Ok(saved) => signal.set(Some(Preview {
                        id: Ok(id),
                        slot,
                        saved,
                        ready: true,
                    })),
                    Err(e) => signal.set(Some(Preview {
                        id: Err(e.into()),
                        slot,
                        saved: None,
                        ready: true,
                    })),
                }
                ui.loading.set(false);
            });
        }
    }
}
fn share(ui: Ui, sharing: RwSignal<Option<String>>) {
    let g = ui.game.get_untracked();
    if let Some(id) = g.deal_id() {
        if let Some(link) = js("spiderDealLink", &[id.code().into()])
            .ok()
            .and_then(|v| v.as_string())
        {
            sharing.set(Some(link));
            ui.notice.set(String::new());
            ui.dialog.set(Dialog::Share);
        } else {
            ui.notice
                .set("Не удалось подготовить ссылку. Переоткройте приложение.".into());
        }
    }
}
fn recent() -> Vec<String> {
    let Some(s) = storage() else { return vec![] };
    let prefix = format!("{}.", account::base_key());
    let mut slots: Vec<_> = (0..s.length().unwrap_or(0))
        .filter_map(|i| s.key(i).ok().flatten())
        .filter_map(|key| key.strip_prefix(&prefix).map(str::to_string))
        .filter(|slot| account::valid_slot(slot))
        .collect();
    slots.sort();
    slots.dedup();
    slots
}
/// One dialog signal owns every application panel; transitions replace the panel.
#[component]
pub fn ReleaseMenu(ui: Ui, animating: RwSignal<bool>) -> impl IntoView {
    let pending = RwSignal::new(None::<Preview>);
    let sharing = RwSignal::new(None::<String>);
    let received = RwSignal::new(None::<String>);
    let ordinary = RwSignal::new(None::<Game>);
    let new_ready = RwSignal::new(false);
    let new_error = RwSignal::new(String::new());
    let recover_regular = RwSignal::new(false);

    let close = move || {
        if ui.dialog.get_untracked() == Dialog::Help {
            ui.close_help();
            return;
        }
        ui.dialog.set(match ui.dialog.get_untracked() {
            Dialog::Actions => {
                if ui.game.get_untracked().finished() {
                    Dialog::None
                } else {
                    Dialog::Pause
                }
            }
            Dialog::Repeat => Dialog::Actions,
            Dialog::ConfirmNew if recover_regular.get_untracked() => Dialog::Corrupt,
            Dialog::Corrupt => return,
            _ => Dialog::None,
        });
    };
    let share_done = Closure::<dyn FnMut()>::new(move || ui.dialog.set(Dialog::None));
    let window = web_sys::window().unwrap();
    window
        .add_event_listener_with_callback("spider-share-done", share_done.as_ref().unchecked_ref())
        .ok();
    share_done.forget();
    let escape =
        Closure::<dyn FnMut(web_sys::KeyboardEvent)>::new(move |event: web_sys::KeyboardEvent| {
            if event.key() == "Escape"
                && ui.dialog.get_untracked() != Dialog::None
                && !ui.loading.get_untracked()
            {
                event.prevent_default();
                close();
            }
        });
    window
        .add_event_listener_with_callback("keydown", escape.as_ref().unchecked_ref())
        .ok();
    escape.forget();
    let browser_back = Closure::<dyn FnMut()>::new(move || {
        if ui.dialog.get_untracked() == Dialog::Help {
            ui.dialog.set(ui.help.get_untracked().return_to);
        }
    });
    window
        .add_event_listener_with_callback("popstate", browser_back.as_ref().unchecked_ref())
        .ok();
    browser_back.forget();
    let platform_back = Closure::<dyn FnMut()>::new(move || {
        if ui.dialog.get_untracked() != Dialog::None && !ui.loading.get_untracked() {
            close();
        }
    });
    window
        .add_event_listener_with_callback("spider-back", platform_back.as_ref().unchecked_ref())
        .ok();
    platform_back.forget();
    Effect::new(move |_| {
        js(
            "spiderSetBackVisible",
            &[(ui.dialog.get() != Dialog::None).into()],
        )
        .ok();
    });

    // Read the ordinary slot, irrespective of the table from which this panel opened.
    Effect::new(move |_| {
        if ui.dialog.get() == Dialog::ConfirmNew {
            ordinary.set(None);
            new_ready.set(false);
            new_error.set(String::new());
            let kind = ui.future_kind.get();
            ui.loading.set(true);
            spawn_local(async move {
                let result = account::prepare(kind.regular_slot()).await;
                if ui.dialog.get_untracked() == Dialog::ConfirmNew
                    && ui.future_kind.get_untracked() == kind
                {
                    match result {
                        Ok(saved) => {
                            ordinary.set(saved);
                            new_ready.set(true);
                        }
                        Err(_)
                            if recover_regular.get_untracked()
                                && kind.regular_slot() == ui.slot.get_untracked() =>
                        {
                            new_ready.set(true)
                        }
                        Err(e) => new_error.set(e.into()),
                    }
                }
                ui.loading.set(false);
            });
        }
    });
    let start_new = move |_| {
        if ui.loading.get_untracked() || !new_ready.get_untracked() {
            return;
        }
        ui.loading.set(true);
        new_error.set(String::new());
        let kind = ui.future_kind.get_untracked();
        spawn_local(async move {
            let result = account::prepare(kind.regular_slot()).await;
            if ui.dialog.get_untracked() == Dialog::ConfirmNew {
                match result {
                    Ok(saved) => {
                        let acknowledged = crate::input::session_replacement_acknowledged(
                            ordinary.get_untracked().as_ref(),
                            saved.as_ref(),
                        );
                        ordinary.set(saved);
                        if acknowledged {
                            recover_regular.set(false);
                            let next = Game::new(
                                ui.future_kind.get_untracked(),
                                crate::seed(),
                                ui.mode.get_untracked(),
                            );
                            if let Err(e) = ui.install(kind.regular_slot(), next) {
                                ui.notice.set(e.into());
                            }
                        }
                    }
                    Err(_)
                        if recover_regular.get_untracked()
                            && kind.regular_slot() == ui.slot.get_untracked() =>
                    {
                        recover_regular.set(false);
                        if let Err(e) = ui.install(
                            kind.regular_slot(),
                            Game::new(
                                ui.future_kind.get_untracked(),
                                crate::seed(),
                                ui.mode.get_untracked(),
                            ),
                        ) {
                            ui.notice.set(e.into());
                        }
                    }
                    Err(e) => new_error.set(e.into()),
                }
            }
            ui.loading.set(false);
        });
    };
    let restart = move || {
        if let Some(id) = ui.game.get_untracked().deal_id() {
            match id.start() {
                Ok(g) => ui.change(ui.slot.get_untracked(), Some(g)),
                Err(e) => ui.notice.set(e.into()),
            }
        }
    };
    let open_incoming = move || {
        if ui.loading.get_untracked() {
            return;
        }
        if let Some(code) = incoming() {
            if received.get_untracked().as_ref() != Some(&code) {
                received.set(Some(code.clone()));
                preview(code, false, ui, pending);
            }
        }
    };
    Effect::new(move |_| {
        if !ui.loading.get() {
            open_incoming();
        }
    });
    let callback = Closure::<dyn FnMut()>::new(open_incoming);
    for event in ["hashchange", "spider-platform-ready", "load"] {
        window
            .add_event_listener_with_callback(event, callback.as_ref().unchecked_ref())
            .ok();
    }
    callback.forget();
    let initial = Closure::once_into_js(move |_: f64| open_incoming());
    window.request_animation_frame(initial.unchecked_ref()).ok();

    view! {
     <Show when=move ||ui.dialog.get()!=Dialog::None || (ui.game.get().finished() && !animating.get())>
      <div class="overlay compact-overlay" on:click=move |e|e.stop_propagation() on:pointerdown=move |e|e.stop_propagation()>
       <section class="modal app-panel" class:new-game-panel=move ||ui.dialog.get()==Dialog::ConfirmNew class:help-panel=move||ui.dialog.get()==Dialog::Help role="dialog" aria-modal="true" aria-label=move ||match ui.dialog.get(){Dialog::ConfirmNew=>"Новая игра",Dialog::Pause=>"Пауза",Dialog::Actions=>"Действия с раскладом",Dialog::Parties=>"Мои партии",Dialog::Repeat=>"Повтор расклада",Dialog::Share=>"Поделиться раскладом",Dialog::Help=>"Как играть",Dialog::Preview=>"Открыть расклад",Dialog::Corrupt=>"Сохранение повреждено",Dialog::None=>if ui.game.get().stuck(){"Ходов нет"}else{"Победа"}}>
        {move ||match ui.dialog.get() {
          Dialog::ConfirmNew=>view!{
            <div class="panel-heading"><h2>"Новая игра"</h2><button class="icon-button" aria-label="Закрыть новую игру" on:click=move |_|close()>"×"</button></div>
            <div class="game-picker mode-picker" role="radiogroup" aria-label="Вид пасьянса">
              { [Kind::Spider,Kind::Klondike,Kind::Tripeaks].into_iter().map(|kind|view!{<button type="button" role="radio" disabled=move||ui.loading.get() aria-checked=move||(ui.future_kind.get()==kind).to_string() class:chosen=move||ui.future_kind.get()==kind on:click=move |_|ui.future_kind.set(kind)>{kind.name()}</button>}).collect_view() }
            </div>
            <button class="new-help text-button" on:click=move |_|ui.open_help()>"Как играть?"</button>
            <Show when=move||ui.future_kind.get()==Kind::Spider fallback=move||view!{<p class="variant-label">{move||if ui.future_kind.get()==Kind::Tripeaks{"Один проход запаса"}else{"По одной карте"}}</p>}>
            <div class="mode-picker" role="radiogroup" aria-label="Количество мастей">
              {Mode::ALL.into_iter().map(|m|view!{<button type="button" role="radio" aria-checked=move ||(ui.mode.get()==m).to_string() class:chosen=move ||ui.mode.get()==m on:click=move |_|ui.mode.set(m)>{m.label()}</button>}).collect_view()}
            </div>
            </Show>
            <Show when=move ||crate::input::replaces_session_progress(ordinary.get().as_ref())><p class="replacement-warning">{move||match ui.future_kind.get(){Kind::Spider=>"Предыдущая обычная партия в „Паука“ будет заменена",Kind::Klondike=>"Текущая партия в „Косынку“ будет заменена",Kind::Tripeaks=>"Текущая партия в „Три пика“ будет заменена"}}</p></Show>
            <Show when=move ||!new_error.get().is_empty()><p role="alert">{move ||new_error.get()}</p></Show>
            <Show when=move ||ui.loading.get()><p class="panel-status" role="status">"Проверяем сохранение…"</p></Show>
            <button class="primary" disabled=move ||ui.loading.get() || !new_ready.get() on:click=start_new>"Начать игру"</button>
          }.into_any(),
          Dialog::Pause=>view!{
            <div class="panel-heading"><h2>"Пауза"</h2><button class="icon-button" aria-label="Действия с раскладом" on:click=move |_|ui.dialog.set(Dialog::Actions)>"⋯"</button></div>
            <button class="primary" on:click=move |_|ui.dialog.set(Dialog::None)>"Продолжить"</button>
            <nav class="panel-rows" aria-label="Выбор партии">
              <button on:click=move |_|ui.open_new()>"Новая игра"<span aria-hidden="true">"›"</span></button>
              <button disabled=move ||ui.loading.get() on:click=move |_|preview(spider_engine::deal::daily((js_sys::Date::now()/1000.) as i64).code(),true,ui,pending)>{move||if ui.game.get().kind()!=Kind::Spider{"Расклад дня · Паук"}else{"Расклад дня"}}<span aria-hidden="true">"›"</span></button>
              <button on:click=move |_|ui.dialog.set(Dialog::Parties)>"Мои партии"<span aria-hidden="true">"›"</span></button>
            </nav>
          }.into_any(),
          Dialog::Actions=>view!{
            <div class="panel-heading"><h2>"Действия с раскладом"</h2><button class="icon-button" aria-label="Назад" on:click=move |_|close()>"×"</button></div>
            <p class="attempt-label">{move ||format!("{} · {}",label(&ui.slot.get()),ui.game.get().mode_label())}</p>
            <Show when=move ||ui.game.get().deal_id().is_some() fallback=||view!{<p class="legacy-note">"У прежней партии нет кода. Повтор и ссылка будут доступны со следующей новой партии."</p>}>
              <div class="panel-rows">
                <button disabled=move ||ui.loading.get() on:click=move |_|{if crate::input::replaces_session_progress(Some(&ui.game.get_untracked())){ui.dialog.set(Dialog::Repeat)}else{restart()}}>"Начать расклад заново"</button>
                <button disabled=move ||ui.loading.get() on:click=move |_|share(ui,sharing)>"Поделиться раскладом"</button>
              </div>
            </Show>
            <button class="row-button" on:click=move |_|ui.open_help()>"Как играть"</button>
            <Show when=move ||ui.game.get().finished() && (ui.game.get().history_len()>0)><button class="row-button" on:click=move |_|{ui.undo();}>"Отменить последний ход"</button></Show>
            <Show when=move ||ui.game.get().finished()><div class="panel-rows"><button on:click=move |_|preview(spider_engine::deal::daily((js_sys::Date::now()/1000.) as i64).code(),true,ui,pending)>{move||if ui.game.get().kind()!=Kind::Spider{"Расклад дня · Паук"}else{"Расклад дня"}}</button><button on:click=move |_|ui.dialog.set(Dialog::Parties)>"Мои партии"</button></div></Show>
          }.into_any(),
          Dialog::Repeat=>view!{
            <div class="panel-heading"><h2>"Начать расклад заново?"</h2></div>
            <p>"Ходы и история текущей попытки сбросятся. Лучший результат сохранится."</p>
            <button class="primary" disabled=move ||ui.loading.get() on:click=move |_|restart()>"Начать заново"</button>
            <button on:click=move |_|close()>"Отмена"</button>
          }.into_any(),
          Dialog::Parties=>view!{
            <div class="panel-heading"><h2>"Мои партии"</h2><button class="icon-button" aria-label="Вернуться к столу" on:click=move |_|ui.dialog.set(Dialog::None)>"×"</button></div>
            <p class="attempt-label">{move ||format!("На столе: {}",label(&ui.slot.get()))}</p>
            <div class="panel-rows">
              <Show when=move ||ui.slot.get()!="regular" && account::peek("regular").ok().flatten().is_some()><button disabled=move ||ui.loading.get() on:click=move |_|ui.change("regular".into(),None)>"Паук · Обычная партия"</button></Show>
              {move ||recent().into_iter().filter(|s|s!=&ui.slot.get()).filter_map(|s|account::peek(&s).ok().flatten().map(|g|(s,g))).map(|(s,g)|{let text=format!("{} · {} · ходы: {}",label(&s),g.mode_label(),g.moves());view!{<button disabled=move ||ui.loading.get() on:click=move |_|ui.change(s.clone(),None)>{text}</button>}}).collect_view()}
            </div>
            <button on:click=move |_|ui.dialog.set(Dialog::None)>"Вернуться к столу"</button>
          }.into_any(),
          Dialog::Preview=>view!{
            <div class="panel-heading"><h2>{move ||if pending.get().is_some_and(|p|p.slot.starts_with("daily_")){"Расклад дня"}else{"Вызов друга"}}</h2><button class="icon-button" aria-label="К моей игре" on:click=move |_|ui.dialog.set(Dialog::None)>"×"</button></div>
            <p>{move ||pending.get().map(|p|match p.id{Ok(id)=>{let context=if p.slot.starts_with("daily_"){label(&p.slot)}else{"Вызов друга".into()};id.label().map(|title|format!("{} · {}",context,if p.slot.starts_with("daily_"){title.strip_prefix("Паук · ").unwrap_or(&title)}else{&title})).unwrap_or_else(str::to_string)},Err(e)=>e})}</p>
            <Show when=move ||pending.get().is_some_and(|p|p.ready && p.id.is_ok())>
              <button class="primary" disabled=move ||ui.loading.get() on:click=move |_|{if let Some(p)=pending.get_untracked(){if let Ok(id)=p.id{match id.start(){Ok(g)=>{if p.saved.as_ref().is_some_and(|g|g.won()){ui.change(p.slot,Some(g));}else{ui.resume(p.slot,Some(g));}},Err(e)=>ui.notice.set(e.into())}}}}>{move ||pending.get().map(|p|if p.saved.as_ref().is_some_and(|g|g.won()){"Сыграть ещё раз"}else if p.saved.is_some(){"Продолжить"}else{"Играть"})}</button>
              <Show when=move ||pending.get().is_some_and(|p|p.saved.is_some())><button disabled=move ||ui.loading.get() on:click=move |_|{if let Some(p)=pending.get_untracked(){if let Ok(id)=p.id{if let Ok(g)=id.start(){ui.change(p.slot,Some(g));}}}}>"Начать сначала"</button></Show>
            </Show>
            <Show when=move ||pending.get().is_some_and(|p|!p.ready)><p role="status">"Проверяем расклад…"</p></Show>
            <button on:click=move |_|ui.dialog.set(Dialog::None)>"К моей игре"</button>
          }.into_any(),
          Dialog::Share=>view!{
            <div class="panel-heading"><h2>"Поделиться раскладом"</h2><button class="icon-button" aria-label="Вернуться к игре" on:click=move |_|ui.dialog.set(Dialog::None)>"×"</button></div>
            <p>"Друг начнёт с исходной позиции. Его партия будет независимой."</p>
            <textarea class="share-link" readonly prop:value=move ||sharing.get().unwrap_or_default() aria-label="Ссылка на расклад"></textarea>
            <button class="primary" on:click=move |_|{let g=ui.game.get_untracked();if let Some(id)=g.deal_id(){let text=if g.won(){format!("«{}»: победа за {} ходов. Сыграешь тот же расклад?",g.kind().name(),g.moves())}else{format!("Попробуй этот расклад: {} · {}",g.kind().name(),g.mode_label())};js("spiderShareDeal",&[id.code().into(),text.into()]).ok();}}>"Отправить другу"</button>
            <button on:click=move |_|{if let Some(link)=sharing.get_untracked(){if let Ok(value)=js("spiderCopyLink",&[link.into()]){spawn_local(async move{let ok=JsFuture::from(js_sys::Promise::resolve(&value)).await.ok().and_then(|v|v.as_bool()).unwrap_or(false);ui.notice.set(if ok{"Ссылка скопирована"}else{"Выделите и скопируйте ссылку вручную"}.into());});}}}>"Скопировать ссылку"</button>
            <Show when=move ||!ui.notice.get().is_empty()><p role="status">{move ||ui.notice.get()}</p></Show>
            <button on:click=move |_|ui.dialog.set(Dialog::None)>"Вернуться к игре"</button>
          }.into_any(),
          Dialog::Help=>view!{<crate::help::Screen context=ui.help.get_untracked() on_back=Callback::new(move |_|close())/>}.into_any(),
          Dialog::Corrupt=>view!{
            <div class="panel-heading"><h2>"Сохранение повреждено"</h2></div><p>"Не удалось восстановить партию. Начните новую игру."</p>
            <button class="primary" on:click=move |_|{recover_regular.set(!account::is_cloud() && ui.slot.get_untracked()==ui.game.get_untracked().kind().regular_slot());ui.open_new();}>"Новая игра"</button>
          }.into_any(),
          Dialog::None=>view!{
            <div class="panel-heading"><h2>{move||if ui.game.get().stuck(){"Ходов нет"}else{"Победа!"}}</h2><button class="icon-button" aria-label="Действия с раскладом" on:click=move |_|ui.dialog.set(Dialog::Actions)>"⋯"</button></div>
            <Show when=move||ui.game.get().tripeaks().is_some()><p>{move||ui.game.get().tripeaks().map(|g|format!("Лучшая цепочка: {}",g.state.best_chain))}</p></Show>
            <Show when=move||{ui.game.get().history_len()>0}><button class="row-button" on:click=move |_|ui.undo()>"Отменить последний ход"</button></Show>
            <p class="victory-result">{move ||format!("{} за {} ходов",if ui.game.get().stuck(){"Запас пуст, подходящих свободных карт нет"}else if ui.game.get().kind()==Kind::Tripeaks{"Все 28 карт сняты"}else if ui.game.get().kind()==Kind::Klondike{"Все 52 карты собраны в дома"}else{"Все 8 серий собраны"},ui.game.get().moves())}</p>
            <p class="attempt-label">{move ||format!("{} · {}",label(&ui.slot.get()),ui.game.get().mode_label())}</p>
            <Show when=move ||best(&ui.game.get()).is_some()><p>{move ||best(&ui.game.get()).map(|n|format!("Лучший результат: {n} ходов"))}</p></Show>
            <button class="primary" on:click=move |_|ui.open_new()>"Новая игра"</button>
            <Show when=move ||ui.game.get().deal_id().is_some() fallback=||view!{<p class="legacy-note">"Для прежней партии ссылка недоступна."</p>}><button on:click=move |_|share(ui,sharing)>"Вызвать друга"</button></Show>
          }.into_any(),
        }}
        <Show when=move ||ui.dialog.get()!=Dialog::ConfirmNew && !ui.notice.get().is_empty() && ui.dialog.get()!=Dialog::Share && ui.dialog.get()!=Dialog::Help><p class="panel-status" role="status">{move ||ui.notice.get()}</p></Show>
       </section>
      </div>
     </Show>
    }
}
