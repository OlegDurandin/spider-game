# VK Mini App Пасьянса

Приложение: https://vk.com/app54898968.
Адрес клиента/API: https://spider.62-109-4-100.sslip.io/.
Telegram-версия остаётся на `85.137.95.37`, её сервисы и база не переносятся.

## Настройка кабинета VK

В настройках Mini App 54898968 укажите один адрес для Web, мобильной
веб-версии и мобильных клиентов: `https://spider.62-109-4-100.sslip.io/`.
После сохранения откройте приложение именно через `https://vk.com/app54898968`:
обычная ссылка на сервер без параметров запуска открывает гостевой режим.
Не копируйте и не публикуйте полный адрес внутри приложения с `sign` и `vk_*`.

Схема соответствует соседнему Blitz Poker: параметры запуска удостоверяют
VK-пользователя, защищённый ключ находится только на API. Профиль/аватар для
сохранений не требуются. Токен сообщества не нужен; чат-бот сообщений не входит
в эту Mini App. Отдельная загрузка на VK hosting для текущей серверной схемы не нужна.

## Авторизация и данные

- Клиент отправляет исходный query в `Authorization: vk <query>`, без записи
  подписи в localStorage. Сервер проверяет HMAC-SHA256/base64url, нужный app id,
  положительный user id, `vk_ts` (до 24 часов, не более 30 секунд в будущем),
  отклоняет дубли параметров. Неподписанный/подменённый VK-запуск не становится гостем.
- В таблицах `vk_games` / `vk_game_slots` VK-партии отделены от исторических
  Telegram `games` / `game_slots`, даже при равных числовых user id.
  Кеш VK: `spider.vk.<id>.v1`, TG: `spider.telegram.<id>.v1`, гость: `spider.game.v1`.
- Все три пасьянса, дневной Паук и вызовы используют прежние слоты, историю отмен,
  последовательную отправку и revision. Конфликт даёт 409, локальная ветка сохраняется.
- Russian VK deployment содержит только VK-реквизиты. Telegram auth в нём отключена.
- CORS разрешает собственный origin и точные origins из `VK_ALLOWED_ORIGINS`.
  При переходе на VK hosting перечислите конкретные HTTPS origins, без путей/масок,
  в локальном `.env`, повторите выкладку. Публичный `VK_API_URL` задаёт origin API.
- Nginx не логирует query URL и отключает access log для `/api/`; в блоке Caddy
  нет access log. Auth proof и защищённый ключ не входят в публичные ссылки.

Реализация проверки сверена с официальным
[примером VKCOM](https://github.com/VKCOM/vk-apps-launch-params/blob/master/examples/golang.go)
и `blitz-poker/crates/pvp-server/src/vk.rs`.

## Выкладка

В локальном `.env`: `VK_APP_ID`, `VK_APP_SECRET` — защищённый ключ именно
приложения Пасьянса. Файл игнорируется Git. Для обновления:

```sh
/usr/bin/python3 scripts/prepare-public-config.py
(cd web && trunk build --release --locked)
bash deploy/vk/deploy.sh
/usr/bin/python3 deploy/sync_vk_bots_server.py --repo ../bots-server --apply
```

VPS `62.109.4.100`, каталог `/opt/bots/bots/spider-vk`.
Compose project `spider-vk`: `spider-vk-web` на `127.0.0.1:8094`,
`spider-vk-api` без опубликованного порта, том `spider-vk_user-data`.
Серверный `.env` имеет права 600; избранные VK-реквизиты передаются через
SSH stdin отдельно от исходников/статики. Полный локальный `.env` не копируется.

Перед изменением сохраняются образы, Compose и Caddy в
`/opt/bots/backups/spider-vk-<UTC>/`; для уже работающего API создаётся online
SQLite backup в `/data/backups/`. При ошибке переключения выполняется откат
образов/Caddy. Тома при этом не удаляются. Не использовать `down -v`.
Для Docker без BuildKit предусмотрен `deploy/vk/Dockerfile.api` без cache mounts.
Скрипт меняет только блок `spider-vk` живого Caddy; чужие pending-изменения
из bots-server не применяются.

## Проверки

- 70 Rust-тестов workspace: 49 движка, 9 API, 12 клиента — прошли.
- VK-подпись, неверный ключ/app/id/время, дубли параметров, изоляция двух VK
  пользователей и Telegram с тем же id, 409, три слота и открытие SQLite заново.
- CORS preflight для разрешённого/чужого origin и обязательность авторизации.
- `scripts/qa/vk-browser.py`: настоящий headless Chrome + native Rust API
  на временной SQLite и тестовом секрете, два HTTP origin. Проверены вход,
  ходы/сохранение/перезагрузка трёх игр, второй пользователь, отсутствие
  guest fallback и записанных auth proof, открытие справки.
- `scripts/qa/help-browser.py`: прежняя навигация и гостевой режим на 360/390 px.
- Тесты JS: VK Bridge LocationChanged доставляет приглашение, копирование через
  VKWebAppCopyText, прежние Telegram/VK ссылки; SDK в этих тестах имитируется.
- `cargo fmt --all --check`, release WASM, синтаксис deploy-скриптов.
- Перед выкладкой исходники и release-файлы проверены на совпадения с локальными
  credential values: значений защищённого ключа/токенов в них нет.

Физические Telegram/VK WebView и настоящий подписанный запуск из кабинета VK
требуют проверки после настройки адресов Mini App. Локальные сквозные тесты
используют синтетическую подпись, а не вход живого VK-пользователя.
