<!-- BEGIN spider-game deployment -->
## Паук / spider-web

Развёрнут 14.11.2026, HTTPS и healthcheck проверены. Первая выкладка:
`spider-web:local-929e3cd2ecb6` (nginx-образ из проверенной WASM-сборки, не из GHCR).
Caddy backup: `/opt/bots/backups/spider-20261114T184517Z/Caddyfile.before`.

Пасьянс на 1, 2 и 4 масти: Rust + Leptos CSR/WASM. Личные партии Telegram
синхронизируются через Rust API и SQLite; гостевые партии остаются в localStorage.
Токен бота используется только API и никогда не попадает в frontend.
VPS `85.137.95.37`, контейнер `spider-web`, `127.0.0.1:8094 → 8080`, HTTPS
`https://spider.85-137-95-37.sslip.io`. Health: `/healthz`.

Конфигурация зеркалируется из `../spider-game/deploy/bots-server/` скриптом
`/usr/bin/python3 ../spider-game/deploy/sync_bots_server.py --repo . --apply`.
Первичная выкладка проверенной локальной сборки: `../spider-game/deploy/deploy.sh`.
Скрипт собирает nginx из готового WASM и API на VPS, сохраняет предыдущие образы
и добавляет только блок Паука к действующему Caddyfile, сохраняя расхождения
других проектов. Полный локальный Caddyfile вслепую не применять.

Следующие registry-релизы после успешного CI:
`./bots.sh 85.137.95.37 deploy spider-web sha-<коммит>`.
Проверка: `curl -fsS https://spider.85-137-95-37.sslip.io/healthz`.
Откат: `./bots.sh 85.137.95.37 rollback spider-web` (при наличии предыдущего образа).
Сброс/обновление контейнера сохраняет том с партиями. Реальная проверка
Telegram/VK на устройствах описывается отдельно в отчёте проекта.
Rust API: контейнер `spider-api`, без внешнего порта; Telegram initData проверяется
секретом `TELEGRAM_BOT_TOKEN` в `bots/spider-web/.env`. Партии пользователей хранятся
в постоянном томе `spider-web_user-data`, SQLite `/data/spider.sqlite3`.
API-образ собирается через `spider-game/deploy/deploy.sh`; обновление только web через
bots.sh сохраняет API-образ. Не выполнять `docker compose down -v`: это удаляет партии.

<!-- END spider-game deployment -->
