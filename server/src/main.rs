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
    let token = std::env::var("TELEGRAM_BOT_TOKEN").expect("TELEGRAM_BOT_TOKEN required");
    assert!(!token.trim().is_empty(), "TELEGRAM_BOT_TOKEN required");
    let path = std::env::var("SPIDER_DB").unwrap_or_else(|_| "/data/spider.sqlite3".into());
    let db = spider_server::database(&path).expect("open database");
    let listener = tokio::net::TcpListener::bind("0.0.0.0:8082")
        .await
        .expect("listen");
    println!("spider API listening on 8082");
    axum::serve(listener, spider_server::router(db, token))
        .with_graceful_shutdown(async {
            tokio::signal::ctrl_c().await.ok();
        })
        .await
        .expect("server");
}
