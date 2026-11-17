pub mod vk;
use axum::{
    Json, Router,
    extract::{DefaultBodyLimit, Path, State},
    http::{HeaderMap, StatusCode},
    routing::get,
};
use hmac::{Hmac, Mac};
use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use spider_engine::Session;
use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

type HmacSha256 = Hmac<Sha256>;
type ApiError = (StatusCode, &'static str);
#[derive(Clone)]
pub struct AppState {
    pub db: Arc<Mutex<Connection>>,
    pub token: Arc<str>,
    pub vk: Option<vk::Config>,
}
#[derive(Serialize, Deserialize)]
pub struct UserGame {
    pub user_id: i64,
    pub revision: i64,
    pub game: Option<Session>,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SaveGame {
    pub revision: i64,
    pub game: Session,
}

pub fn database(path: &str) -> rusqlite::Result<Connection> {
    let db = Connection::open(path)?;
    db.busy_timeout(Duration::from_secs(5))?;
    db.execute_batch(
        "PRAGMA journal_mode=WAL; PRAGMA synchronous=FULL;
        CREATE TABLE IF NOT EXISTS games (
          user_id INTEGER PRIMARY KEY, revision INTEGER NOT NULL CHECK(revision > 0),
          game TEXT NOT NULL, updated_at INTEGER NOT NULL
        );
        CREATE TABLE IF NOT EXISTS game_slots (
          user_id INTEGER NOT NULL, slot TEXT NOT NULL, revision INTEGER NOT NULL CHECK(revision > 0),
          game TEXT NOT NULL, updated_at INTEGER NOT NULL, PRIMARY KEY(user_id,slot)
        );",
    )?;
    db.execute_batch("CREATE TABLE IF NOT EXISTS vk_games (user_id INTEGER PRIMARY KEY, revision INTEGER NOT NULL CHECK(revision > 0), game TEXT NOT NULL, updated_at INTEGER NOT NULL);
    CREATE TABLE IF NOT EXISTS vk_game_slots (user_id INTEGER NOT NULL, slot TEXT NOT NULL, revision INTEGER NOT NULL CHECK(revision > 0), game TEXT NOT NULL, updated_at INTEGER NOT NULL, PRIMARY KEY(user_id,slot));")?;
    Ok(db)
}
pub fn router(db: Connection, token: String) -> Router {
    router_with_vk(db, token, None, vec![])
}
pub fn router_with_vk(
    db: Connection,
    token: String,
    vk: Option<vk::Config>,
    origins: Vec<String>,
) -> Router {
    Router::new()
        .route("/healthz", get(|| async { "ok\n" }))
        .route("/api/game", get(load).put(save))
        .route("/api/game/{slot}", get(load_named).put(save_named))
        .layer(DefaultBodyLimit::max(2 * 1024 * 1024))
        .layer(axum::middleware::map_response(
            |mut response: axum::response::Response| async {
                response
                    .headers_mut()
                    .insert("cache-control", "no-store".parse().unwrap());
                response
            },
        ))
        .layer(axum::middleware::from_fn(
            move |req: axum::extract::Request, next: axum::middleware::Next| {
                let origins = origins.clone();
                async move {
                    let origin = req
                        .headers()
                        .get("origin")
                        .and_then(|v| v.to_str().ok())
                        .map(str::to_string);
                    let api = req.uri().path().starts_with("/api/");
                    let same_origin = req
                        .headers()
                        .get("host")
                        .and_then(|h| h.to_str().ok())
                        .is_some_and(|h| {
                            origin.as_ref().is_some_and(|o| {
                                o == &format!("https://{h}") || o == &format!("http://{h}")
                            })
                        });
                    if api && !same_origin && origin.as_ref().is_some_and(|o| !origins.contains(o))
                    {
                        return axum::response::IntoResponse::into_response((
                            StatusCode::FORBIDDEN,
                            "Origin not allowed",
                        ));
                    }
                    let mut response = if api && req.method() == axum::http::Method::OPTIONS {
                        axum::response::IntoResponse::into_response(StatusCode::NO_CONTENT)
                    } else {
                        next.run(req).await
                    };
                    if api {
                        if let Some(origin) = origin {
                            if let Ok(v) = origin.parse() {
                                response
                                    .headers_mut()
                                    .insert("access-control-allow-origin", v);
                            }
                            response
                                .headers_mut()
                                .insert("vary", "Origin".parse().unwrap());
                            response.headers_mut().insert(
                                "access-control-allow-methods",
                                "GET, PUT, OPTIONS".parse().unwrap(),
                            );
                            response.headers_mut().insert(
                                "access-control-allow-headers",
                                "Authorization, Content-Type".parse().unwrap(),
                            );
                            response
                                .headers_mut()
                                .insert("access-control-max-age", "600".parse().unwrap());
                        }
                    }
                    response
                }
            },
        ))
        .with_state(AppState {
            db: Arc::new(Mutex::new(db)),
            token: token.into(),
            vk,
        })
}
fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

/// Only signed Telegram launch data determines the account. Never accept a client user ID.
pub fn authenticate(data: &str, token: &str, now: u64) -> Result<i64, &'static str> {
    if data.len() > 16_384 || token.is_empty() {
        return Err("invalid auth");
    }
    let fields: Vec<(String, String)> =
        serde_urlencoded::from_str(data).map_err(|_| "invalid auth")?;
    let mut sorted = BTreeMap::new();
    for (key, value) in fields {
        if sorted.insert(key, value).is_some() {
            return Err("duplicate field");
        }
    }
    let hash =
        hex::decode(sorted.remove("hash").ok_or("missing hash")?).map_err(|_| "invalid hash")?;
    let check = sorted
        .iter()
        .map(|(k, v)| format!("{k}={v}"))
        .collect::<Vec<_>>()
        .join("\n");
    let mut secret = HmacSha256::new_from_slice(b"WebAppData").unwrap();
    secret.update(token.as_bytes());
    let mut signature = HmacSha256::new_from_slice(&secret.finalize().into_bytes()).unwrap();
    signature.update(check.as_bytes());
    signature
        .verify_slice(&hash)
        .map_err(|_| "invalid signature")?;
    let date: u64 = sorted
        .get("auth_date")
        .ok_or("missing date")?
        .parse()
        .map_err(|_| "invalid date")?;
    if date > now.saturating_add(30) || now.saturating_sub(date) > 86_400 {
        return Err("expired auth");
    }
    #[derive(Deserialize)]
    struct User {
        id: i64,
    }
    let user: User = serde_json::from_str(sorted.get("user").ok_or("missing user")?)
        .map_err(|_| "invalid user")?;
    if user.id <= 0 || user.id > (1_i64 << 52) - 1 {
        return Err("invalid id");
    }
    Ok(user.id)
}
#[derive(Clone, Copy)]
struct Account {
    id: i64,
    vk: bool,
}
impl Account {
    fn games(self) -> &'static str {
        if self.vk { "vk_games" } else { "games" }
    }
    fn slots(self) -> &'static str {
        if self.vk {
            "vk_game_slots"
        } else {
            "game_slots"
        }
    }
}
fn user(headers: &HeaderMap, state: &AppState) -> Result<Account, ApiError> {
    let proof = headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .ok_or((StatusCode::UNAUTHORIZED, "Open the game from its platform"))?;
    let account = if let Some(data) = proof.strip_prefix("tma ") {
        authenticate(data, &state.token, now()).map(|id| Account { id, vk: false })
    } else if let Some(data) = proof.strip_prefix("vk ") {
        state
            .vk
            .as_ref()
            .ok_or("VK disabled")
            .and_then(|c| vk::verify(data, c, now()))
            .map(|id| Account { id, vk: true })
    } else {
        Err("unknown platform")
    };
    account.map_err(|_| {
        (
            StatusCode::UNAUTHORIZED,
            "Invalid or expired platform session",
        )
    })
}
fn internal(_: impl std::fmt::Debug) -> ApiError {
    (StatusCode::INTERNAL_SERVER_ERROR, "Storage unavailable")
}
async fn load(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<UserGame>, ApiError> {
    load_for(state, headers, "regular".into()).await
}
async fn load_named(
    State(state): State<AppState>,
    Path(slot): Path<String>,
    headers: HeaderMap,
) -> Result<Json<UserGame>, ApiError> {
    load_for(state, headers, slot).await
}
async fn load_for(
    state: AppState,
    headers: HeaderMap,
    slot: String,
) -> Result<Json<UserGame>, ApiError> {
    if !spider_engine::deal::valid_slot(&slot) {
        return Err((StatusCode::BAD_REQUEST, "Invalid slot"));
    }
    let account = user(&headers, &state)?;
    let id = account.id;
    tokio::task::spawn_blocking(move || {
        let db = state.db.lock().map_err(internal)?;
        let row: Option<(i64, String)> = if slot == "regular" {
            db.query_row(
                &format!(
                    "SELECT revision,game FROM {} WHERE user_id=?1",
                    account.games()
                ),
                [id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()
        } else {
            db.query_row(
                &format!(
                    "SELECT revision,game FROM {} WHERE user_id=?1 AND slot=?2",
                    account.slots()
                ),
                params![id, slot],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()
        }
        .map_err(internal)?;
        let (revision, game) = match row {
            Some((revision, json)) => (revision, Some(Session::restore(&json).map_err(internal)?)),
            None => (0, None),
        };
        Ok(Json(UserGame {
            user_id: id,
            revision,
            game,
        }))
    })
    .await
    .map_err(internal)?
}
async fn save(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<SaveGame>,
) -> Result<Json<UserGame>, ApiError> {
    save_for(state, headers, input, "regular".into()).await
}
async fn save_named(
    State(state): State<AppState>,
    Path(slot): Path<String>,
    headers: HeaderMap,
    Json(input): Json<SaveGame>,
) -> Result<Json<UserGame>, ApiError> {
    save_for(state, headers, input, slot).await
}
async fn save_for(
    state: AppState,
    headers: HeaderMap,
    mut input: SaveGame,
    slot: String,
) -> Result<Json<UserGame>, ApiError> {
    if !spider_engine::deal::valid_slot(&slot) {
        return Err((StatusCode::BAD_REQUEST, "Invalid slot"));
    }
    let account = user(&headers, &state)?;
    let id = account.id;
    input.game = Session::restore(&serde_json::to_string(&input.game).map_err(internal)?)
        .map_err(|_| (StatusCode::UNPROCESSABLE_ENTITY, "Invalid game"))?;
    if !input.game.fits_slot(&slot) {
        return Err((StatusCode::UNPROCESSABLE_ENTITY, "Game does not match slot"));
    }
    if let Some(code) = slot
        .strip_prefix("daily_")
        .map(|d| format!("d1_{d}"))
        .or_else(|| slot.strip_prefix("challenge_").map(str::to_string))
    {
        if input.game.deal_code().as_deref() != Some(code.as_str()) {
            return Err((StatusCode::UNPROCESSABLE_ENTITY, "Deal does not match slot"));
        }
    }
    if input.revision < 0 || input.revision == i64::MAX {
        return Err((StatusCode::BAD_REQUEST, "Invalid revision"));
    }
    tokio::task::spawn_blocking(move || {
        let db = state.db.lock().map_err(internal)?;
        let json=serde_json::to_string(&input.game).map_err(internal)?;
        let changed = if slot!="regular" {
            if input.revision==0 {db.execute(&format!("INSERT INTO {}(user_id,slot,revision,game,updated_at) VALUES(?1,?2,1,?3,?4) ON CONFLICT(user_id,slot) DO NOTHING",account.slots()),params![id,slot,json,now() as i64])}
            else {db.execute(&format!("UPDATE {} SET revision=revision+1,game=?3,updated_at=?4 WHERE user_id=?1 AND slot=?2 AND revision=?5",account.slots()),params![id,slot,json,now() as i64,input.revision])}
        } else if input.revision == 0 {
            db.execute(&format!("INSERT INTO {}(user_id,revision,game,updated_at) VALUES (?1,1,?2,?3) ON CONFLICT(user_id) DO NOTHING",account.games()),params![id,json,now() as i64])
        } else {
            db.execute(&format!("UPDATE {} SET revision=revision+1,game=?2,updated_at=?3 WHERE user_id=?1 AND revision=?4",account.games()),params![id,json,now() as i64,input.revision])
        }.map_err(internal)?;
        if changed != 1 { return Err((StatusCode::CONFLICT,"Game changed on another device")); }
        // No need to echo the entire game and history after every save.
        Ok(Json(UserGame {user_id:id,revision:input.revision+1,game:None}))
    }).await.map_err(internal)?
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{body::Body, http::Request};
    use http_body_util::BodyExt;
    use spider_engine::Game;
    use tower::ServiceExt;
    const TOKEN: &str = "123456:test-only-token";
    fn signed(id: i64, date: u64) -> String {
        let mut fields = BTreeMap::from([
            ("auth_date", date.to_string()),
            ("query_id", "test".into()),
            ("user", format!("{{\"id\":{id},\"first_name\":\"Test\"}}")),
            ("signature", "signed-new-field".into()),
        ]);
        let check = fields
            .iter()
            .map(|(k, v)| format!("{k}={v}"))
            .collect::<Vec<_>>()
            .join("\n");
        let mut secret = HmacSha256::new_from_slice(b"WebAppData").unwrap();
        secret.update(TOKEN.as_bytes());
        let mut mac = HmacSha256::new_from_slice(&secret.finalize().into_bytes()).unwrap();
        mac.update(check.as_bytes());
        fields.insert("hash", hex::encode(mac.finalize().into_bytes()));
        serde_urlencoded::to_string(fields).unwrap()
    }
    async fn request(
        app: Router,
        method: &str,
        auth: Option<&str>,
        body: Option<serde_json::Value>,
    ) -> (StatusCode, Vec<u8>) {
        request_path(app, method, auth, body, "/api/game").await
    }
    async fn request_path(
        app: Router,
        method: &str,
        auth: Option<&str>,
        body: Option<serde_json::Value>,
        path: &str,
    ) -> (StatusCode, Vec<u8>) {
        let mut req = Request::builder()
            .method(method)
            .uri(path)
            .header("content-type", "application/json");
        if let Some(auth) = auth {
            req = req.header("authorization", format!("tma {auth}"));
        }
        let r = app
            .oneshot(
                req.body(Body::from(body.map(|v| v.to_string()).unwrap_or_default()))
                    .unwrap(),
            )
            .await
            .unwrap();
        (
            r.status(),
            r.into_body().collect().await.unwrap().to_bytes().to_vec(),
        )
    }
    #[test]
    fn rejects_forged_expired_future_and_duplicate_auth() {
        let t = 1_800_000_000;
        let valid = signed(4_000_000_001, t);
        assert_eq!(authenticate(&valid, TOKEN, t), Ok(4_000_000_001));
        assert!(authenticate(&valid, "wrong-token", t).is_err());
        assert!(authenticate(&valid.replace("4000000001", "4000000002"), TOKEN, t).is_err());
        assert!(authenticate(&valid, TOKEN, t + 86_401).is_err());
        assert!(authenticate(&signed(1, t + 31), TOKEN, t).is_err());
        assert!(authenticate(&(valid + "&auth_date=1800000000"), TOKEN, t).is_err());
        assert!(authenticate(&signed(-1, t), TOKEN, t).is_err());
        assert!(authenticate(&signed(1, t), "", t).is_err());
    }
    #[tokio::test]
    async fn users_are_isolated_and_stale_writes_cannot_overwrite() {
        let app = router(database(":memory:").unwrap(), TOKEN.into());
        let a = signed(101, now());
        let b = signed(202, now());
        assert_eq!(
            request(app.clone(), "GET", None, None).await.0,
            StatusCode::UNAUTHORIZED
        );
        let ga = Game::new_with_mode(1, spider_engine::Mode::Four);
        let gb = Game::new_with_mode(2, spider_engine::Mode::Two);
        assert_eq!(
            request(
                app.clone(),
                "PUT",
                Some(&a),
                Some(serde_json::json!({"revision":0,"game":ga}))
            )
            .await
            .0,
            StatusCode::OK
        );
        let (_, json) = request(app.clone(), "GET", Some(&b), None).await;
        let empty: UserGame = serde_json::from_slice(&json).unwrap();
        assert!(empty.game.is_none());
        assert_eq!(empty.revision, 0);
        assert_eq!(
            request(
                app.clone(),
                "PUT",
                Some(&b),
                Some(serde_json::json!({"revision":0,"game":gb}))
            )
            .await
            .0,
            StatusCode::OK
        );
        for (auth, expected, id) in [(&a, &ga, 101), (&b, &gb, 202)] {
            let (_, json) = request(app.clone(), "GET", Some(auth), None).await;
            let loaded: UserGame = serde_json::from_slice(&json).unwrap();
            assert_eq!(
                loaded.game.as_ref().and_then(Session::spider),
                Some(expected)
            );
            assert_eq!(loaded.user_id, id);
        }
        let write = serde_json::json!({"revision":1,"game":ga});
        let (x, y) = tokio::join!(
            request(app.clone(), "PUT", Some(&a), Some(write.clone())),
            request(app.clone(), "PUT", Some(&a), Some(write))
        );
        assert!(
            (x.0 == StatusCode::OK && y.0 == StatusCode::CONFLICT)
                || (y.0 == StatusCode::OK && x.0 == StatusCode::CONFLICT)
        );
        assert_eq!(
            request(
                app.clone(),
                "PUT",
                Some(&a),
                Some(serde_json::json!({"revision":0,"game":gb}))
            )
            .await
            .0,
            StatusCode::CONFLICT
        );
        assert_eq!(
            request(
                app.clone(),
                "PUT",
                Some(&a),
                Some(serde_json::json!({"revision":2,"game":gb,"user_id":202}))
            )
            .await
            .0,
            StatusCode::UNPROCESSABLE_ENTITY
        );
        let mut bad = ga;
        bad.state.stock[0].id = bad.state.stock[1].id;
        assert_eq!(
            request(
                app,
                "PUT",
                Some(&a),
                Some(serde_json::json!({"revision":2,"game":bad}))
            )
            .await
            .0,
            StatusCode::UNPROCESSABLE_ENTITY
        );
    }
    #[tokio::test]
    async fn database_survives_reopen_with_history_and_mode() {
        let path = std::env::temp_dir().join(format!(
            "spider-test-{}-{}.sqlite3",
            std::process::id(),
            now()
        ));
        let path = path.to_str().unwrap();
        let auth = signed(303, now());
        let mut game = Game::new_with_mode(17, spider_engine::Mode::Two);
        game.deal().unwrap();
        let app = router(database(path).unwrap(), TOKEN.into());
        assert_eq!(
            request(
                app.clone(),
                "PUT",
                Some(&auth),
                Some(serde_json::json!({"revision":0,"game":game}))
            )
            .await
            .0,
            StatusCode::OK
        );
        drop(app);
        let reopened = router(database(path).unwrap(), TOKEN.into());
        let (_, json) = request(reopened, "GET", Some(&auth), None).await;
        assert_eq!(
            serde_json::from_slice::<UserGame>(&json).unwrap().game,
            Some(Session::Spider(game))
        );
        std::fs::remove_file(path).unwrap();
    }
    #[tokio::test]
    async fn slots_are_independent_and_old_games_migrate() {
        let app = router(database(":memory:").unwrap(), TOKEN.into());
        let a = signed(101, now());
        let b = signed(202, now());
        let mut regular = Game::new_with_mode(42, spider_engine::Mode::Four);
        regular.deal().unwrap();
        let mut daily = spider_engine::DealId::parse("d1_20261115")
            .unwrap()
            .start()
            .unwrap();
        daily.deal().unwrap();
        let mut challenge = spider_engine::DealId::parse("s1_2_ffffffffffffffff")
            .unwrap()
            .start()
            .unwrap();
        challenge.deal().unwrap();
        let paths = [
            "/api/game",
            "/api/game/daily_20261115",
            "/api/game/challenge_s1_2_ffffffffffffffff",
        ];
        for (path, game) in paths.iter().zip([&regular, &daily, &challenge]) {
            assert_eq!(
                request_path(
                    app.clone(),
                    "PUT",
                    Some(&a),
                    Some(serde_json::json!({"revision":0,"game":game})),
                    path
                )
                .await
                .0,
                StatusCode::OK
            );
        }
        for (path, game) in paths.iter().zip([&regular, &daily, &challenge]) {
            let (_, json) = request_path(app.clone(), "GET", Some(&a), None, path).await;
            let saved: UserGame = serde_json::from_slice(&json).unwrap();
            assert_eq!(saved.game.as_ref().and_then(Session::spider), Some(game));
            assert_eq!(saved.revision, 1);
            let (_, json) = request_path(app.clone(), "GET", Some(&b), None, path).await;
            assert!(
                serde_json::from_slice::<UserGame>(&json)
                    .unwrap()
                    .game
                    .is_none()
            );
        }
        assert_eq!(
            request_path(
                app.clone(),
                "PUT",
                Some(&a),
                Some(serde_json::json!({"revision":1,"game":challenge})),
                paths[1]
            )
            .await
            .0,
            StatusCode::UNPROCESSABLE_ENTITY
        );
        assert_eq!(
            request_path(
                app.clone(),
                "GET",
                Some(&a),
                None,
                "/api/game/challenge_bad"
            )
            .await
            .0,
            StatusCode::BAD_REQUEST
        );
        assert_eq!(
            request_path(
                app.clone(),
                "PUT",
                Some(&a),
                Some(serde_json::json!({"revision":0,"game":daily})),
                paths[1]
            )
            .await
            .0,
            StatusCode::CONFLICT
        );
        let mut old = serde_json::to_value(&regular).unwrap();
        old["version"] = 2.into();
        old.as_object_mut().unwrap().remove("deal");
        assert_eq!(
            request_path(
                app.clone(),
                "PUT",
                Some(&a),
                Some(serde_json::json!({"revision":1,"game":old})),
                paths[0]
            )
            .await
            .0,
            StatusCode::OK
        );
        let (_, json) = request_path(app, "GET", Some(&a), None, paths[0]).await;
        let restored = serde_json::from_slice::<UserGame>(&json)
            .unwrap()
            .game
            .unwrap()
            .into_spider()
            .unwrap();
        assert!(restored.deal.is_none());
        assert_eq!(restored.state, regular.state);
        assert_eq!(restored.history, regular.history);
    }
    #[tokio::test]
    async fn klondike_spider_and_challenge_survive_reopen_in_separate_user_slots() {
        use spider_engine::klondike::{Command, Game as Klondike};
        let path = format!(
            "/tmp/spider12-api-{}-{}.sqlite3",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );
        let app = router(database(&path).unwrap(), TOKEN.into());
        let a = signed(111, now());
        let b = signed(222, now());
        let mut spider = Game::new_with_mode(42, spider_engine::Mode::Four);
        spider.deal().unwrap();
        let mut k = Klondike::new(u64::MAX);
        for _ in 0..25 {
            k.apply(Command::Draw).unwrap();
        }
        let mut challenge = Klondike::new(0x0123456789abcdef);
        challenge.apply(Command::Draw).unwrap();
        let mut t = spider_engine::tripeaks::Game::new(42);
        t.apply(spider_engine::tripeaks::Command::Draw).unwrap();
        let records = [
            (
                "/api/game/tripeaks_regular".into(),
                Session::Tripeaks(t.clone()),
            ),
            (
                "/api/game/challenge_t1_1_000000000000002a".into(),
                Session::Tripeaks(t),
            ),
            ("/api/game".to_string(), Session::Spider(spider)),
            ("/api/game/klondike_regular".into(), Session::Klondike(k)),
            (
                "/api/game/challenge_k1_1_0123456789abcdef".into(),
                Session::Klondike(challenge),
            ),
        ];
        for (path, g) in &records {
            assert_eq!(
                request_path(
                    app.clone(),
                    "PUT",
                    Some(&a),
                    Some(serde_json::json!({"revision":0,"game":g})),
                    path
                )
                .await
                .0,
                StatusCode::OK
            );
            let (_, body) = request_path(app.clone(), "GET", Some(&b), None, path).await;
            assert!(
                serde_json::from_slice::<UserGame>(&body)
                    .unwrap()
                    .game
                    .is_none()
            );
        }
        assert_eq!(
            request_path(
                app.clone(),
                "PUT",
                Some(&a),
                Some(serde_json::json!({"revision":1,"game":records[3].1})),
                "/api/game"
            )
            .await
            .0,
            StatusCode::UNPROCESSABLE_ENTITY
        );
        assert_eq!(
            request_path(
                app.clone(),
                "PUT",
                Some(&a),
                Some(serde_json::json!({"revision":1,"game":records[2].1})),
                "/api/game/klondike_regular"
            )
            .await
            .0,
            StatusCode::UNPROCESSABLE_ENTITY
        );
        drop(app);
        let reopened = router(database(&path).unwrap(), TOKEN.into());
        for (path, g) in records {
            let (_, body) = request_path(reopened.clone(), "GET", Some(&a), None, &path).await;
            let saved = serde_json::from_slice::<UserGame>(&body).unwrap();
            assert_eq!(saved.revision, 1);
            assert_eq!(saved.game, Some(g));
        }
        drop(reopened);
        std::fs::remove_file(path).unwrap();
    }
}

#[cfg(test)]
mod vk_integration {
    use super::*;
    use axum::{body::Body, http::Request};
    use http_body_util::BodyExt;
    use tower::ServiceExt;
    async fn req(
        app: Router,
        method: &str,
        path: &str,
        proof: &str,
        body: Option<serde_json::Value>,
    ) -> (StatusCode, serde_json::Value) {
        let mut r = Request::builder()
            .method(method)
            .uri(path)
            .header("authorization", proof);
        if body.is_some() {
            r = r.header("content-type", "application/json")
        }
        let out = app
            .oneshot(
                r.body(Body::from(body.map(|v| v.to_string()).unwrap_or_default()))
                    .unwrap(),
            )
            .await
            .unwrap();
        let code = out.status();
        let bytes = out.into_body().collect().await.unwrap().to_bytes();
        (code, serde_json::from_slice(&bytes).unwrap_or_default())
    }
    fn config() -> vk::Config {
        vk::Config {
            app_id: 123,
            secret: "vk-test-secret".into(),
        }
    }
    #[tokio::test]
    async fn vk_accounts_slots_revisions_and_disk_restore() {
        let path = format!(
            "/tmp/spider-vk-{}-{}.sqlite3",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );
        let app = router_with_vk(
            database(&path).unwrap(),
            "tg-test".into(),
            Some(config()),
            vec![],
        );
        let a = format!("vk {}", vk::signed(777, 123, now()));
        let b = format!("vk {}", vk::signed(778, 123, now()));
        for (slot, kind) in [
            ("regular", spider_engine::Kind::Spider),
            ("klondike_regular", spider_engine::Kind::Klondike),
            ("tripeaks_regular", spider_engine::Kind::Tripeaks),
        ] {
            let mut game = Session::new(kind, 42, spider_engine::Mode::Four);
            match &mut game {
                Session::Spider(g) => g.deal().unwrap(),
                Session::Klondike(g) => g.apply(spider_engine::klondike::Command::Draw).unwrap(),
                Session::Tripeaks(g) => g.apply(spider_engine::tripeaks::Command::Draw).unwrap(),
            };
            let body = serde_json::json!({"revision":0,"game":game});
            let url = format!("/api/game/{slot}");
            assert_eq!(
                req(app.clone(), "PUT", &url, &a, Some(body.clone()))
                    .await
                    .0,
                StatusCode::OK
            );
            assert_eq!(
                req(app.clone(), "PUT", &url, &a, Some(body)).await.0,
                StatusCode::CONFLICT
            );
            let (_, other) = req(app.clone(), "GET", &url, &b, None).await;
            assert!(other["game"].is_null());
            let reopened = router_with_vk(
                database(&path).unwrap(),
                String::new(),
                Some(config()),
                vec![],
            );
            let (_, saved) = req(reopened, "GET", &url, &a, None).await;
            assert_eq!(saved["user_id"], 777);
            assert_eq!(saved["revision"], 1);
            assert_eq!(Session::restore(&saved["game"].to_string()).unwrap(), game);
        }
        // Same numeric id on Telegram has an independent legacy row.
        let mut fields = BTreeMap::from([
            ("auth_date".to_string(), now().to_string()),
            ("user".into(), "{\"id\":777}".into()),
        ]);
        let check = fields
            .iter()
            .map(|(k, v)| format!("{k}={v}"))
            .collect::<Vec<_>>()
            .join("\n");
        let mut secret = HmacSha256::new_from_slice(b"WebAppData").unwrap();
        secret.update(b"tg-test");
        let mut sig = HmacSha256::new_from_slice(&secret.finalize().into_bytes()).unwrap();
        sig.update(check.as_bytes());
        fields.insert("hash".into(), hex::encode(sig.finalize().into_bytes()));
        let tg = format!("tma {}", serde_urlencoded::to_string(fields).unwrap());
        let (_, tg_empty) = req(app.clone(), "GET", "/api/game", &tg, None).await;
        assert!(tg_empty["game"].is_null());
        let game = Session::new(spider_engine::Kind::Spider, 99, spider_engine::Mode::One);
        assert_eq!(
            req(
                app.clone(),
                "PUT",
                "/api/game",
                &tg,
                Some(serde_json::json!({"revision":0,"game":game}))
            )
            .await
            .0,
            StatusCode::OK
        );
        let (_, vk_saved) = req(app.clone(), "GET", "/api/game", &a, None).await;
        assert_ne!(vk_saved["game"], serde_json::to_value(game).unwrap());
        assert_eq!(
            req(
                app,
                "GET",
                "/api/game",
                &a.replace("vk_user_id=777", "vk_user_id=778"),
                None
            )
            .await
            .0,
            StatusCode::UNAUTHORIZED
        );
    }
    #[tokio::test]
    async fn cors_is_exact_and_never_replaces_auth() {
        let app = router_with_vk(
            database(":memory:").unwrap(),
            String::new(),
            Some(config()),
            vec!["https://app.example.vk-apps.com".into()],
        );
        for (origin, expected) in [
            ("https://app.example.vk-apps.com", StatusCode::NO_CONTENT),
            ("https://evil.example", StatusCode::FORBIDDEN),
        ] {
            let r = app
                .clone()
                .oneshot(
                    Request::builder()
                        .method("OPTIONS")
                        .uri("/api/game")
                        .header("origin", origin)
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(r.status(), expected);
            if expected == StatusCode::NO_CONTENT {
                assert_eq!(r.headers()["access-control-allow-origin"], origin);
            }
        }
        let r = app
            .oneshot(
                Request::builder()
                    .uri("/api/game")
                    .header("origin", "https://app.example.vk-apps.com")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(r.status(), StatusCode::UNAUTHORIZED);
    }
}
