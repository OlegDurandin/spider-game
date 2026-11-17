#[tokio::main]
async fn main() {
    if std::env::args().any(|a| a == "--healthcheck") {
        use std::io::{Read, Write};
        let mut stream = std::net::TcpStream::connect_timeout(
            &"127.0.0.1:8082".parse().unwrap(),
            std::time::Duration::from_secs(2),
        )
        .expect("health connect");
        stream
            .set_read_timeout(Some(std::time::Duration::from_secs(2)))
            .unwrap();
        stream
            .write_all(b"GET /healthz HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")
            .unwrap();
        let mut response = String::new();
        stream.read_to_string(&mut response).unwrap();
        assert!(response.starts_with("HTTP/1.1 200"));
        return;
    }
    if let Some(destination) = std::env::args().skip_while(|a| a != "--backup").nth(1) {
        let path = std::env::var("SPIDER_DB").unwrap_or_else(|_| "/data/spider.sqlite3".into());
        let db = spider_server::database(&path).expect("open database for backup");
        if let Some(parent) = std::path::Path::new(&destination).parent() {
            std::fs::create_dir_all(parent).expect("create backup directory");
        }
        db.execute("VACUUM INTO ?1", [destination])
            .expect("consistent SQLite backup");
        println!("SQLite backup complete");
        return;
    }
    let token = std::env::var("TELEGRAM_BOT_TOKEN").unwrap_or_default();
    let vk_id = std::env::var("VK_APP_ID").unwrap_or_default();
    let vk_secret = std::env::var("VK_APP_SECRET").unwrap_or_default();
    let vk = if vk_id.is_empty() && vk_secret.is_empty() {
        None
    } else {
        Some(spider_server::vk::Config {
            app_id: vk_id.parse().expect("numeric VK_APP_ID required"),
            secret: vk_secret,
        })
    };
    assert!(
        !token.trim().is_empty()
            || vk
                .as_ref()
                .is_some_and(|c| c.app_id > 0 && !c.secret.is_empty()),
        "Configure Telegram or VK authentication"
    );
    let origins = std::env::var("SPIDER_ALLOWED_ORIGINS")
        .unwrap_or_default()
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .collect();
    let path = std::env::var("SPIDER_DB").unwrap_or_else(|_| "/data/spider.sqlite3".into());
    let db = spider_server::database(&path).expect("open database");
    let listener = tokio::net::TcpListener::bind("0.0.0.0:8082")
        .await
        .expect("listen");
    println!("spider API listening on 8082");
    axum::serve(
        listener,
        spider_server::router_with_vk(db, token, vk, origins),
    )
    .with_graceful_shutdown(async {
        tokio::signal::ctrl_c().await.ok();
    })
    .await
    .expect("server");
}
