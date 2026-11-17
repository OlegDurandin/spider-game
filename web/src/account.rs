//! Per-account cache and a serialized, revision-checked cloud save queue.
use crate::save_queue::SaveQueue;
use gloo_net::http::Request;
use leptos::prelude::*;
use serde::{Deserialize, Serialize};
use spider_engine::Session as Game;
use std::{cell::RefCell, collections::HashMap};
use wasm_bindgen_futures::spawn_local;

#[derive(Serialize, Deserialize)]
struct Remote {
    user_id: i64,
    revision: i64,
    game: Option<Game>,
}
#[derive(Serialize, Deserialize)]
struct Cached {
    revision: i64,
    game: Game,
    dirty: bool,
}
#[derive(Default)]
struct Client {
    auth: String,
    vk: bool,
    key: String,
    queues: HashMap<String, SaveQueue>,
    active: String,
    legacy: bool,
    note: String,
    ui: Option<(RwSignal<String>, RwSignal<bool>)>,
}
thread_local! { static CLIENT: RefCell<Client> = RefCell::new(Client::default()); }
fn storage() -> Result<web_sys::Storage, ()> {
    web_sys::window()
        .ok_or(())?
        .local_storage()
        .map_err(|_| ())?
        .ok_or(())
}
pub fn is_cloud() -> bool {
    CLIENT.with(|c| !c.borrow().auth.is_empty())
}
pub fn base_key() -> String {
    CLIENT.with(|c| c.borrow().key.clone())
}
pub fn slot() -> String {
    CLIENT.with(|c| {
        let c = c.borrow();
        if c.active.is_empty() {
            "regular".into()
        } else {
            c.active.clone()
        }
    })
}
fn key_for(slot: &str) -> String {
    if slot == "regular" {
        base_key()
    } else {
        format!("{}.{}", base_key(), slot)
    }
}
pub fn key() -> String {
    key_for(&slot())
}
fn queue<R>(slot: &str, f: impl FnOnce(&mut SaveQueue) -> R) -> R {
    CLIENT.with(|c| f(c.borrow_mut().queues.entry(slot.into()).or_default()))
}
fn platform_name() -> &'static str {
    if CLIENT.with(|c| c.borrow().vk) {
        "VK"
    } else {
        "Telegram"
    }
}
fn account_note() -> String {
    format!("Аккаунт {} · прогресс сохранён", platform_name())
}
fn config(name: &str) -> String {
    let w = web_sys::window().unwrap();
    js_sys::Reflect::get(&w, &"SPIDER_CONFIG".into())
        .ok()
        .and_then(|c| js_sys::Reflect::get(&c, &name.into()).ok())
        .and_then(|v| v.as_string())
        .unwrap_or_default()
}
fn endpoint(slot: &str) -> String {
    let path = if slot == "regular" {
        "/api/game".into()
    } else {
        format!("/api/game/{slot}")
    };
    let base = if CLIENT.with(|c| c.borrow().vk) {
        config("vkApiUrl")
    } else {
        String::new()
    };
    format!("{}{path}", base.trim_end_matches('/'))
}
pub fn valid_slot(slot: &str) -> bool {
    spider_engine::deal::valid_slot(slot)
}
pub fn status() -> String {
    CLIENT.with(|c| c.borrow().note.clone())
}
pub fn has_legacy() -> bool {
    CLIENT.with(|c| c.borrow().legacy)
}
pub fn dismiss_legacy() {
    if let Ok(s) = storage() {
        s.set_item(&format!("{}.legacy-handled", key()), "1").ok();
    }
}
pub fn legacy() -> Option<Game> {
    Game::restore(&storage().ok()?.get_item("spider.game.v1").ok()??).ok()
}
pub fn attach(note: RwSignal<String>, retry: RwSignal<bool>) {
    CLIENT.with(|c| c.borrow_mut().ui = Some((note, retry)));
}
fn notify(message: &str, retry: bool) {
    let ui = CLIENT.with(|c| {
        let mut c = c.borrow_mut();
        c.note = message.into();
        c.ui
    });
    if let Some((note, button)) = ui {
        note.set(message.into());
        button.set(retry);
    }
}
fn cache_at(slot: &str, game: &Game, revision: i64, dirty: bool) -> Result<(), ()> {
    let json = serde_json::to_string(&Cached {
        game: game.clone(),
        revision,
        dirty,
    })
    .map_err(|_| ())?;
    storage()?.set_item(&key_for(slot), &json).map_err(|_| ())
}
pub fn peek(slot: &str) -> Result<Option<Game>, &'static str> {
    let Some(json) = storage()
        .map_err(|_| "storage")?
        .get_item(&key_for(slot))
        .map_err(|_| "storage")?
    else {
        return Ok(None);
    };
    let game = if !is_cloud() {
        Game::restore(&json)?
    } else {
        let cached: Cached = serde_json::from_str(&json).map_err(|_| "Повреждённое сохранение")?;
        cached.game
    };
    if !game.fits_slot(slot) {
        return Err("Сохранение не соответствует выбранной партии");
    }
    Ok(Some(game))
}
pub fn saved() -> Result<Option<Game>, &'static str> {
    peek(&slot())
}
pub fn persist(game: &Game) -> Result<(), ()> {
    let slot = slot();
    if !game.fits_slot(&slot) {
        return Err(());
    }
    if !is_cloud() {
        return storage()?
            .set_item(
                &key_for(&slot),
                &serde_json::to_string(game).map_err(|_| ())?,
            )
            .map_err(|_| ());
    }
    let revision = queue(&slot, |q| {
        q.enqueue(game.clone());
        q.revision
    });
    let result = cache_at(&slot, game, revision, true);
    start_slot(&slot);
    result
}
async fn remote(auth: &str, slot: &str) -> Result<Remote, &'static str> {
    let response = Request::get(&endpoint(slot))
        .header("Authorization", auth)
        .send()
        .await
        .map_err(
            |_| "Не удалось связаться с сервером. Проверьте соединение и переоткройте игру.",
        )?;
    if response.status() == 401 {
        return Err(if CLIENT.with(|c| c.borrow().vk) {
            "Не удалось подтвердить аккаунт. Переоткройте игру из VK."
        } else {
            "Не удалось подтвердить аккаунт. Переоткройте игру из Telegram."
        });
    }
    if !response.ok() {
        return Err("Сохранения временно недоступны. Переоткройте игру позже.");
    }
    let r: Remote = response.json().await.map_err(|_| "Ошибка ответа сервера")?;
    if let Some(g) = &r.game {
        g.validate().map_err(|_| "Ошибка сохранения на сервере")?;
        if !g.fits_slot(slot) {
            return Err("Сохранение не соответствует выбранной партии");
        }
    }
    Ok(r)
}
pub async fn initialize() -> Result<(), &'static str> {
    let loc = web_sys::window().unwrap().location();
    let query = web_sys::UrlSearchParams::new_with_str(&loc.search().unwrap_or_default()).unwrap();
    let hash = web_sys::UrlSearchParams::new_with_str(
        loc.hash().unwrap_or_default().trim_start_matches('#'),
    )
    .unwrap();
    let vk_launch = query.has("vk_app_id") || query.has("vk_user_id") || query.has("sign");
    if query.get("demo").as_deref() == Some("1")
        && !vk_launch
        && !query.has("tgWebAppPlatform")
        && !hash.has("tgWebAppData")
    {
        CLIENT.with(|c| c.borrow_mut().key = "spider.demo.v1".into());
        return Ok(());
    }
    let tg = hash
        .get("tgWebAppData")
        .or_else(|| query.get("tgWebAppData"));
    let auth = if vk_launch {
        if tg.is_some() || !query.has("sign") || !query.has("vk_user_id") {
            return Err("Переоткройте игру из VK для входа в аккаунт.");
        }
        CLIENT.with(|c| c.borrow_mut().vk = true);
        format!(
            "vk {}",
            loc.search().unwrap_or_default().trim_start_matches('?')
        )
    } else if let Some(tg) = tg {
        format!("tma {tg}")
    } else {
        if query.has("tgWebAppPlatform") || hash.has("tgWebAppPlatform") {
            return Err("Переоткройте игру из Telegram для входа в аккаунт.");
        }
        CLIENT.with(|c| {
            let mut c = c.borrow_mut();
            c.key = "spider.game.v1".into();
            c.note = "Гостевая игра · прогресс на этом устройстве".into();
        });
        restore_active().await?;
        return Ok(());
    };
    let r = remote(&auth, "regular").await?;
    CLIENT.with(|c| {
        let mut c = c.borrow_mut();
        c.key = format!(
            "spider.{}.{}.v1",
            if c.vk { "vk" } else { "telegram" },
            r.user_id
        );
        c.auth = auth;
        c.active = "regular".into();
        c.note = format!(
            "Аккаунт {} · прогресс сохранён",
            if c.vk { "VK" } else { "Telegram" }
        );
    });
    load_remote("regular", r)?;
    let offer_legacy = legacy().is_some()
        && storage()
            .ok()
            .and_then(|s| {
                s.get_item(&format!("{}.legacy-handled", base_key()))
                    .ok()
                    .flatten()
            })
            .is_none();
    CLIENT.with(|c| c.borrow_mut().legacy = offer_legacy);
    restore_active().await?;
    Ok(())
}
async fn restore_active() -> Result<(), &'static str> {
    let active = storage()
        .ok()
        .and_then(|s| s.get_item(&format!("{}.active", base_key())).ok().flatten())
        .unwrap_or_else(|| "regular".into());
    if active != "regular" && valid_slot(&active) {
        match prepare(&active).await {
            Ok(Some(_)) => activate(&active)?,
            Ok(None) => activate("regular")?,
            Err(e) if !is_cloud() && e != "storage" => activate(&active)?,
            Err(e) => return Err(e),
        }
    }
    Ok(())
}
fn load_remote(slot: &str, r: Remote) -> Result<Option<Game>, &'static str> {
    queue(slot, |q| q.revision = r.revision);
    let local = storage()
        .ok()
        .and_then(|s| s.get_item(&key_for(slot)).ok().flatten())
        .and_then(|s| serde_json::from_str::<Cached>(&s).ok())
        .and_then(|mut c| {
            c.game = Game::restore(&serde_json::to_string(&c.game).ok()?).ok()?;
            Some(c)
        });
    if let Some(c) = local
        .as_ref()
        .filter(|c| c.dirty && c.revision == r.revision)
    {
        queue(slot, |q| q.enqueue(c.game.clone()));
        return Ok(Some(c.game.clone()));
    }
    if local.as_ref().is_some_and(|c| c.dirty) {
        archive_at(slot)?;
    }
    if let Some(g) = &r.game {
        cache_at(slot, g, r.revision, false).map_err(|_| "Не удалось сохранить локальную копию")?;
    } else {
        storage()
            .map_err(|_| "storage")?
            .remove_item(&key_for(slot))
            .map_err(|_| "storage")?;
    }
    Ok(r.game)
}
pub async fn prepare(slot: &str) -> Result<Option<Game>, &'static str> {
    if !valid_slot(slot) {
        return Err("Неизвестная партия");
    }
    if !is_cloud() {
        return peek(slot);
    }
    if queue(slot, |q| q.sending || q.pending.is_some() || q.conflict) {
        return peek(slot);
    }
    let auth = CLIENT.with(|c| c.borrow().auth.clone());
    let r = remote(&auth, slot).await?;
    load_remote(slot, r)
}
pub fn activate(slot: &str) -> Result<(), &'static str> {
    if !valid_slot(slot) {
        return Err("Неизвестная партия");
    }
    storage()
        .map_err(|_| "storage")?
        .set_item(&format!("{}.active", base_key()), slot)
        .map_err(|_| "storage")?;
    CLIENT.with(|c| c.borrow_mut().active = slot.into());
    if queue(slot, |q| q.conflict) {
        notify(
            "Партия изменена на другом устройстве. Загрузите серверную версию.",
            true,
        );
        start_slot(slot);
        return Ok(());
    }
    let note = if is_cloud() {
        account_note()
    } else {
        "Гостевая игра · прогресс на этом устройстве".into()
    };
    notify(&note, false);
    start_slot(slot);
    Ok(())
}
fn archive_at(slot: &str) -> Result<(), &'static str> {
    let s = storage().map_err(|_| "Хранилище недоступно")?;
    if let Some(json) = s
        .get_item(&key_for(slot))
        .map_err(|_| "Хранилище недоступно")?
    {
        s.set_item(
            &format!("{}.conflict.{}", key_for(slot), js_sys::Date::now() as u64),
            &json,
        )
        .map_err(|_| "Не удалось сохранить резервную копию")?;
    }
    Ok(())
}
pub fn conflicted() -> bool {
    queue(&slot(), |q| q.conflict)
}
pub fn retry() {
    if queue(&slot(), |q| q.conflict) {
        // Another tab shares localStorage; archive this tab's actual unsent state.
        let backup = queue(&slot(), |q| {
            q.pending.clone().map(|game| Cached {
                game,
                revision: q.revision,
                dirty: true,
            })
        });
        if let Some(backup) = backup {
            let result = storage().and_then(|s| {
                s.set_item(
                    &format!("{}.conflict.{}", key(), js_sys::Date::now() as u64),
                    &serde_json::to_string(&backup).map_err(|_| ())?,
                )
                .map_err(|_| ())
            });
            if result.is_err() {
                notify(
                    "Не удалось сохранить локальную копию. Освободите место в хранилище.",
                    true,
                );
                return;
            }
        }
        web_sys::window().unwrap().location().reload().ok();
    } else {
        start();
    }
}
pub fn start() {
    start_slot(&slot());
}
fn start_slot(slot: &str) {
    if is_cloud() && queue(slot, SaveQueue::start) {
        let slot = slot.to_string();
        spawn_local(flush(slot));
    }
}
fn notify_slot(slot_name: &str, text: &str, retry: bool) {
    if slot() == slot_name {
        notify(text, retry)
    }
}
async fn flush(slot: String) {
    loop {
        let auth = CLIENT.with(|c| c.borrow().auth.clone());
        let Some((game, revision)) = queue(&slot, SaveQueue::next) else {
            return;
        };
        notify_slot(&slot, "Сохраняем прогресс…", false);
        let body = serde_json::json!({"revision":revision,"game":game});
        let response = match Request::put(&endpoint(&slot))
            .header("Authorization", &auth)
            .json(&body)
        {
            Ok(request) => request.send().await.ok(),
            Err(_) => None,
        };
        let code = response.as_ref().map(|r| r.status());
        let success = if let Some(response) = response.filter(|r| r.ok()) {
            response.json::<Remote>().await.ok()
        } else {
            None
        };
        if let Some(r) = success {
            let pending = queue(&slot, |q| {
                q.acknowledge(r.revision);
                q.pending.clone()
            });
            if cache_at(
                &slot,
                pending.as_ref().unwrap_or(&game),
                r.revision,
                pending.is_some(),
            )
            .is_err()
            {
                notify_slot(
                    &slot,
                    "На сервере сохранено. Локальная копия недоступна.",
                    false,
                );
            } else {
                notify_slot(&slot, &account_note(), false);
            }
        } else {
            queue(&slot, |q| q.fail(game, code == Some(409)));
            notify_slot(
                &slot,
                match code {
                    Some(409) => {
                        "Партия изменена на другом устройстве. Загрузите её, чтобы продолжить синхронизацию."
                    }
                    Some(401) => {
                        "Вход устарел. Переоткройте игру из своей платформы; ходы сохранены на устройстве."
                    }
                    _ => "Нет синхронизации. Ходы сохранены на устройстве — повторите отправку.",
                },
                true,
            );
            return;
        }
    }
}
/// Refresh only an idle, unchanged client; a request must never undo a newer local move.
pub async fn refresh() -> Option<Game> {
    let active = slot();
    if !is_cloud() {
        return None;
    }
    let revision = queue(&active, |q| {
        if q.idle_at(q.revision) {
            Some(q.revision)
        } else {
            None
        }
    })?;
    let auth = CLIENT.with(|c| c.borrow().auth.clone());
    let r = remote(&auth, &active).await.ok()?;
    if slot() != active || !queue(&active, |q| q.idle_at(revision)) || r.revision == revision {
        return None;
    }
    let g = r.game?;
    cache_at(&active, &g, r.revision, false).ok()?;
    queue(&active, |q| q.revision = r.revision);
    notify("Загружена ваша последняя партия", false);
    Some(g)
}
