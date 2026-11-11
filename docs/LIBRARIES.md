# Бібліотеки: що беремо, що тестуємо, що відкидаємо

Дослідження під Rust-порт. Кожен рядок — перевірений факт (версія, ліцензія,
дерево залежностей) або явно позначена оцінка.

## Що нам насправді потрібно

1. **Steam CM** — логін, ліцензії, PICS (appinfo), depot keys. Без цього ми
   назавжди залежимо від Hubcap.
2. **CDN** — маніфести, чанки, AES, zstd/lzma, delta-патчі.
3. **VDF/ACF** — читати й писати `appmanifest_*.acf` без втрати полів.
4. **HTTP/TLS і WebSocket** — API, CDP.
5. **ZIP** — архіви Hubcap.
6. **Вікно** — зараз python+webkit, хочемо один бінарник.
7. **UI Steam** — зараз CDP-інжект, це крихко.
8. **Арт** — обкладинки для ігор без `library_600x900`.
9. **Lua** — перевіряти наші плани справжнім інтерпретатором.
10. **FS watch** — ловити момент, коли Steam дописав `appmanifest`.

## Кандидати

| Бібліотека | Версія | Ліцензія | Що дає | Ціна в білді | Вердикт |
| --- | --- | --- | --- | --- | --- |
| **steamroom** | 0.3.0 | Apache-2.0 | CM-клієнт, QR-логін, PICS, розбір маніфестів, AES-чанки, CDN-пул, KeyValue binary+text | tokio+reqwest+rustls+prost+rsa ≈ 120 крейтів | **Беремо за feature-флагом.** Єдине, що дає «свій акаунт» без Hubcap |
| **steamroom-client** | 0.3.0 | Apache-2.0 | Те саме + retry, delta-патчі, прогрес | те саме дерево | Беремо разом із steamroom |
| steamdepot | 0.3.3 | **LGPL-2.1** | Те саме, інша реалізація | те саме | **Не лінкуємо.** LGPL + статичний Rust = зобов'язання віддавати об'єктники. Тільки як референс або окремий процес |
| **keyvalues-parser** | 0.2.4 | MIT/Apache-2.0 | VDF text: парс + рендер, 458k завантажень | 2 крейти | **Беремо** для appmanifest і config.vdf |
| **steamlocate** | 2.1.1 | MIT | Знаходить Steam, бібліотеки, ігри | 3 крейти | **Беремо.** Перевірено на цій машині |
| **ureq** | 3.5.1 | MIT/Apache-2.0 | Блокуючий HTTP з rustls | ~20 крейтів | **Беремо** замість `curl`-процесів |
| **tungstenite** | 0.30.0 | MIT/Apache-2.0 | WebSocket для CDP | 5 крейтів | **Беремо** замість власної реалізації |
| **zip** | 2.x | MIT | Читання архівів Hubcap | 3 крейти | Беремо |
| **notify** | 8.2.2 | CC0-1.0 | inotify: стежити за `appmanifest` і логом Steam | 4 крейти | Беремо для watcher |
| mlua | 0.12.2 | MIT | Lua 5.4/LuaJIT з Rust | системний lua або vendored | Опційно: валідація планів перед записом |
| steamguard | 0.18.4 | MIT | 2FA, mobile confirmations | середнє | Тільки якщо робимо повний логін у свій акаунт |
| prost | 0.14.5 | Apache-2.0 | Protobuf для CM | тягне rustc-білд | Не окремо: приходить із steamroom |
| wry + tao | 0.57 / 0.37 | MIT/Apache-2.0 | Вікно з webview (webkit2gtk) | ~60 крейтів | **Пізніше.** Дає один бінарник замість python-хелпера |
| egui / iced / slint | 0.37 / 0.14 / 1.18 | MIT | GUI без webview | 30–60 крейтів, але вбиває нашу верстку | Ні |
| steamgriddb_api | 0.3.1 | MIT | Арт зі SteamGridDB | закинута з 2022, async на reqwest | **Ні.** Три HTTP-виклики простіші за залежність |
| steamworks | 0.13.1 | MIT | Офіційний SDK | потребує Steamworks SDK | Ні: він для своїх ігор, не для менеджера бібліотеки |

## Не-крейти, які варто знати

| Проєкт | Що це | Чому цікаво |
| --- | --- | --- |
| **Millennium / SteamBrew** | Завантажувач модів Steam: хукає серверний роутер клієнта й патчить JS **на льоту**, поки Steam його віддає | Замінює наш CDP-інжект: панель у меню стає частиною UI, а не DOM-патчем. Linux x86_64 підтримується офіційно. Бекенд плагіна — Lua, фронт — TS |
| **SteamProtocolBreakdown** (EliCunninghamDev) | 873 рядки: покроковий розбір логіну, PICS, depot keys, CDN | Специфікація, за якою можна писати свій CM-клієнт без здогадок |
| **ManifestHub3** | Спільнотний кеш depot-ключів | Ще один провайдер ключів |
| **ManifestDeX.Cli** | .NET CLI для ключів/appinfo | Провайдер, не бібліотека |
| **opensteamdb-api** | Self-hosted API з appinfo | Потребує Postgres/Redis — не для нас |
| **SLSsteam** | C++ референс Linux-клієнта | Читати, не лінкувати |
| **vapor-forge** | Наш рушій, AGPL-3.0 | Викликаємо як `.so` через `LD_AUDIT` — це не лінкування, AGPL не переходить на нас |

## Виміряно на цій машині

Мінімальний каркас (serde, serde_json, sha2, base64, zip, keyvalues-parser,
steamlocate, ureq, tungstenite, notify):

```
cargo build --release   14.4 s
target/                 179 MB
бінарник                308 KB   (strip + lto + opt-level=s)
```

Тобто «40 ГБ» — це міф для нашого випадку. Навіть із steamroom орієнтир
1.5–2 ГБ і 3–5 хвилин першого білда, і то лише якщо вмикати feature.

Перевірено в роботі: `steamlocate` знайшов Steam у `~/.local/share/Steam` і
бібліотеку, `keyvalues-parser` розібрав `AppState` з `appmanifest`.

## Рекомендований стек

**Tier A — ядро (завжди):**
`serde`, `serde_json`, `keyvalues-parser`, `steamlocate`, `ureq`, `tungstenite`,
`zip`, `sha2`, `notify`, `base64`.

**Tier B — `--features steam-login`:**
`steamroom` + `steamroom-client`. Дає: вхід у свій акаунт (QR або пароль),
ліцензії, PICS, depot keys, скачування маніфестів без Hubcap.

**Tier C — `--features webview`:**
`wry` + `tao`, коли прибираємо python-хелпер. Міряємо білд перед тим.

## Чотири експерименти перед портом (по 1–2 години)

1. **steamroom QR-логін** → ліцензії → `get_depot_key(730, 731)` → порівняти з
   тим, що дає Hubcap. Якщо збігається — Hubcap стає запасним шляхом.
2. **Millennium-плагін** із нашою панеллю замість CDP. Перевірити, чи виживає
   після оновлення Steam і чи не конфліктує з `LD_AUDIT` від Vapor Forge.
3. **notify-watcher** на `appmanifest_*.acf`: авто-фікс при `StateFlags=4`.
4. **steamlocate** замість власного парсера `libraryfolders.vdf` — перевірити
   на бібліотеці на іншому диску.

## Що це змінює в плані

- Порт на Rust робимо **без tokio в ядрі**: важкі речі — за feature-флагами.
- Меню в Steam переходить із CDP на Millennium-плагін (менше поломок).
- Ключі: додається шлях «свій акаунт → PICS», Hubcap лишається для решти.
- Арт: SteamGridDB прямими HTTP-викликами для ігор без обкладинки.
- `steamdepot` не чіпаємо через LGPL; якщо потрібен другий реалізаційний
  референс — читаємо, але не лінкуємо.