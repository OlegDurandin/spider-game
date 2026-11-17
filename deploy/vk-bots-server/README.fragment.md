<!-- BEGIN spider-vk deployment -->
## Пасьянс на вечер / VK Mini App

VK-версия Пасьянса: `https://spider.62-109-4-100.sslip.io/`,
VPS `62.109.4.100`, каталог `/opt/bots/bots/spider-vk`.
Web — `spider-vk-web`, `127.0.0.1:8094`; API — `spider-vk-api`, без внешнего порта.
Том `spider-vk_user-data`, SQLite `/data/spider.sqlite3`. Telegram остаётся на своём VPS.

VK_APP_ID и VK_APP_SECRET (защищённый ключ Mini App) хранятся в серверном `.env`
с правами 600. VK user id принимается только после проверки подписи, приложения
и срока запуска. Токен сообщества не нужен. Прогресс всех трёх пасьянсов
и история отмен сохраняются; VK и Telegram имеют отдельные строки и кеши.

Выкладка из `spider-game`: `bash deploy/vk/deploy.sh` после release WASM.
Скрипт делает online backup SQLite, сохраняет образы/Compose/Caddy,
заменяет только блок spider-vk в живом Caddy и ждёт healthy.
Секреты передаются отдельно через SSH stdin; локальный `.env` не копируется.
Не выполнять `docker compose down -v`.

Зеркало: `python3 ../spider-game/deploy/sync_vk_bots_server.py --repo . --apply`.
Проверки и настройка VK: `spider-game/docs/vk.md`.
<!-- END spider-vk deployment -->
