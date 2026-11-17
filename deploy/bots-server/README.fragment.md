<!-- BEGIN spider-game deployment -->
## Пасьянс на вечер / spider-web

Развёрнут 14.11.2026, HTTPS и healthcheck проверены. Первая выкладка:
`spider-web:local-929e3cd2ecb6` (nginx-образ из проверенной WASM-сборки, не из GHCR).
Caddy backup: `/opt/bots/backups/spider-20261114T184517Z/Caddyfile.before`.

Обновлено 16.11.2026 до версии 1.3. Web: `spider-web:local-ae897d93034f`,
API: `spider-api:local-e54895ba3d00`. Перед обновлением сохранена SQLite-копия,
предыдущие образы и Caddy — `/opt/bots/backups/spider-20261116T120241Z/`.

17.11.2026 выложен встроенный экран «Как играть»: вход `?` со стола и ссылка
из выбора новой партии, контекст всех трёх игр, примеры на игровых картах,
раскрываемые вопросы и возврат с сохранением выделения/прокрутки.
Web: `spider-web:local-20b2cdfbde4a`; API и формат данных прежние.
Backup перед переключением: `/opt/bots/backups/spider-20261117T143133Z/`.
Проверки: 66 Rust-тестов, release WASM и Chrome на 360/390 px; отчёт и
скриншоты — `spider-game/docs/how-to-play.md`.

Версия 1.3: «Паук» на 1, 2 и 4 масти, «Косынка» и «Три пика».
Rust + Leptos CSR/WASM. У всех трёх игр отдельные обычные партии;
повтор и ссылки воспроизводят исходную раздачу. «Расклад дня» остаётся Пауком.
Личные партии Telegram
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
