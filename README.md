# hoi4-rust-accel (`v1.4` — Linux & Windows)

Нативный ускоритель движка Clausewitz и нечитерный усилитель ИИ для **Hearts of Iron IV** (`Operation Postern v1.20.0.0.8873`, 64-бит) в двух синхронизированных сборках:
- **Linux (`LD_PRELOAD`, нативный Rust):** `libhoi4_rust_accel.so` + лаунчер `launch_hoi4.sh`
- **Windows (`version.dll` прокси, C++20 MSVC x64 ABI):** `windows/dist/version.dll` (подробная инструкция: `windows/README_WINDOWS.md`)
- **Единый конфиг для обеих ОС:** `hoi4_accel.ini` (либо переменные окружения `HOI4_ACCEL_*`).

### Ключевые гарантии
- **100% неинвазивно:** оригинальные бинарники игры (`hoi4` и `hoi4.exe`) и игровые файлы на диске не изменяются ни на один байт.
- **Без читов и нарушения баланса:** ускоритель не трогает `NMilitary` / `NProduction` / `NCountry` (правила боя и производства одинаковы для игрока и бота) и не рисует боту бесплатные заводы, танки или снаряжение. Все улучшения ИИ работают строго через решения ИИ, нативные MP-безопасные команды (`CMoveCommand`, `CAddConstructionCommand`) и веса стратегий.
- **Нулевые конфликты с модами:** работает прямо из `.so` / `version.dll` с ваниллой, всеми DLC, сохранениями и модами (*Road to 56, Kaiserreich/KaiserreduX, BlackICE, TNO, Millennium Dawn, The Fire Rises* и др.).
- **Версионность и быстрый откат:** в GitHub Releases доступны все версии (`v1.0.0`, `v1.1.0`, `v1.2.0`, `v1.3.0`, `v1.4.0`), а любой модуль можно мгновенно отключить флагом `= 0` в `hoi4_accel.ini`.

---

## Быстрый запуск

### Windows
Скопируйте `version.dll` и `hoi4_accel.ini` из `windows/dist/` (или из архива релиза `hoi4-accel-v1.4.0-windows-x64.zip`) прямо в папку с `hoi4.exe` и запускайте игру как обычно. Подробнее — в `windows/README_WINDOWS.md`.

### Linux (Steam)
В Steam: **ПКМ по Hearts of Iron IV → Свойства → Параметры запуска**:
```bash
/путь/к/hoi4-accel/launch_hoi4.sh %command%
```
> **Важно:** не оставляйте флаг `-debug` в параметрах запуска при обычной игре — в режиме `-debug` сам движок HoI4 непрерывно пишет мегабайты файлов в `logs/metrics/`, заметно тормозя симуляцию.

---

## Что нового в `v1.4` (23 модуля движка и ИИ)

1. **`AiCaptureEmpty` (`ai_capture_empty = 1`, НОВОЕ в `v1.4` — нативный захват пустых клеток в гражданках/Сибири/Африке):**
   - Лечит «затуп» планировщика фронта на растянутых линиях, где ванильный `CFrontOrder` вообще не отдаёт приказ на шаг вперёд.
   - Раз в сутки для воюющего ИИ сканирует простаивающие (не в бою, не в движении, не отступающие, орга `>= 35%`) дивизии, находит смежные **полностью пустые** (`HasLandUnitsInProvince == 0`) вражеские клетки (`IsAtWarWith == true`, `CanMoveIntoProvince == true`) и отправляет туда нативную MP-безопасную команду **`CMoveCommand`** (`0x253e000` Linux / `0x13fa480` Windows) с проверкой `CanExecute(0)` + `CSession::Post` (до 6 захватов на страну в сутки).
   - **Защита от котлов:** шаг разрешён только если в исходной клетке остаётся прикрытие (`>= 2` своих дивизий) **либо** вокруг пустой целевой клетки нет занятых врагом провинций.
2. **`AiSmartSupply v3` (`ai_smart_supply = 1` — доводка автостроителя сухопутных `supply_node`):**
   - Ослаблены перестрахованные пороги `v1.3`: радиус проверки BFS снижен с `3` до **`2` клеток** (чтобы узлы строились и на европейских фронтах, а не только в пустой Сибири), порог промышленности снижен до **`>= 12` заводов** (работает и в гражданских войнах), порог голода — `< 65%` снабжения у `>= 2` дивизий в секторе (или `< 35%` у `>= 1` на редких фронтах), кулдаун `21` день, жёсткий лимит **1 активный `supply_node`** в очереди.
3. **Анти-перетасовка фронтов (`ai_tactical_prudence = 1`, v4):**
   - `REASSIGN_TO_ANOTHER_FRONT_FACTOR (0.50 → 0.20)`, `REASSIGN_TO_ANOTHER_FRONT_IF_IN_COMBAT_FACTOR (0.20 → 0.05)` и `REDEPLOY_DISTANCE_VS_ORDER_SIZE (1.00 → 2.00)` — ИИ резко реже срывает дивизии с фронта и гоняет их по стратегической переброске с обнулением организации.
4. **`AiSmartNaval` (`ai_smart_naval = 1`, НОВОЕ в `v1.4` — нормальные морские десанты и концентрация флота, 15 параметров `NDefines::NAI`):**
   - **Десанты:** `INVASION_TARGET_NO_PORT_FACTOR (0.30 → 0.05)`, `MIN_INVASION_UNITS_READY_TO_EXECUTE (0.25 → 0.60)`, `MIN_INVASION_ORG_FACTOR_TO_EXECUTE (0.40 → 0.65)`, `MIN_INVASION_PLAN_VALUE_TO_EXECUTE (0.30 → 0.45)`, отмена проигрышного штурма берега за **5 дней** вместо 30 (`720ч → 120ч`) при перевесе `1.8x` (вместо `3.5x`), повторно не суицидиться в проваленный порт `120` дней, крупнее группы (`INVASION_COASTAL_PROVS_PER_ORDER 24 → 40`).
   - **Флот (Strike Force):** `NAVY_PREFERED_MAX_SIZE (25 → 60)`, `CAPITAL_TASKFORCE_MAX_CAPITAL_COUNT (12 → 16)`, `SCREEN_TASKFORCE_MAX_SHIP_COUNT (12 → 24)`, `MAX_PATROL_TO_STRIKE_FORCE_RATIO (4.0 → 2.0)`, `STRIKE_FORCE_TARGET_RECALC_DAYS (5 → 3)`.

### Проверенные модули `v1.0` → `v1.4`
- **`AiSmartArmor` (`ai_smart_armor = 1`)** — промышленно-масштабируемый танковый/мех. ИИ (`CAIStrategyStore::GetStrategyValue`, `>= 22` воен. заводов с 1938+).
- **`AiSmartPeace` (`ai_smart_peace = 1`)** — анти-бордергор мирных конференций (`3x..5x` за смежность, `-95%` сухопутные анклавы).
- **`AiSmartAirbase` (`ai_smart_airbase = 1`)** — приоритет фронтовых `air_base` `3.2x` (`0.50 → 1.60`) + перебазирование при покрытии `< 75%`.
- **`QuietLog v2` / `FastAlloc` / `FastModifier` / `CountryGrain` / `CivilWarFix` / `AiThrottleMinors` / `FastSched` / `FastWait` / `FastYield` / `FastEvent` / `NoJobClock` / `Affinity` / `SmoothFPS` / `TargetFPS` / `FastSave` / `NoTelemetry`**.

---

## Конфигурация (`hoi4_accel.ini` и переменные окружения)

| Ключ в `hoi4_accel.ini` | Переменная окружения | По умолчанию | Описание |
| :--- | :--- | :---: | :--- |
| `preset` | `HOI4_ACCEL_PRESET` | `smooth` | `smooth` / `balanced` / `turbo` / `pure` (MP) / `none` |
| `ai_capture_empty` | `HOI4_ACCEL_AI_CAPTURE_EMPTY` | `1` | Нативный захват пустых вражеских клеток (`CMoveCommand`, защита от котлов) |
| `ai_smart_supply` | `HOI4_ACCEL_AI_SMART_SUPPLY` | `1` | Логистика v3: ЖД, моторизация + автопостройка `supply_node` (`>=12` зав., 2-hop BFS) |
| `ai_smart_naval` | `HOI4_ACCEL_AI_SMART_NAVAL` | `1` | Нормальные морские десанты на порты и концентрация Strike Force флота |
| `ai_tactical_prudence` | `HOI4_ACCEL_AI_TACTICAL_PRUDENCE` | `1` | Тактика v4: анти-перетасовка фронтов, сбережение снаряжения, кулаки `AIFC`, патч пустых клеток |
| `ai_smart_armor` | `HOI4_ACCEL_AI_SMART_ARMOR` | `1` | Умный танковый/мех. ИИ (`>=22` воен. заводов, 1938+, апгрейды и шаблоны танков) |
| `ai_smart_peace` | `HOI4_ACCEL_AI_SMART_PEACE` | `1` | Анти-бордергор на мирной конференции (бонус смежности `3x..5x`, штраф анклавам `-95%`) |
| `ai_smart_airbase` | `HOI4_ACCEL_AI_SMART_AIRBASE` | `1` | Умное расширение фронтовых авиабаз (`air_base` prio `3.2x`) и покрытие миссий |
| `ai_throttle_minors` | `HOI4_ACCEL_AI_THROTTLE_MINORS` | `1` | Разгрузка почасового ИИ спящих мирных миноров (`<12` обл.) |
| `fast_alloc` | `HOI4_ACCEL_FAST_ALLOC` | `1` | Lock-free slab + TLS аллокатор (`128 МБ`) для `<= 512 Б` |
| `fast_mod` | `HOI4_ACCEL_FAST_MOD` | `1` | Безветвевой `CMOV` бинарный поиск `CModifier::GetValue` |
| `country_grain` | `HOI4_ACCEL_COUNTRY_GRAIN` | `1` | `chunk=1` для параллельных циклов стран (`17..=220`) |
| `civil_war_fix` | `HOI4_ACCEL_CIVIL_WAR_FIX` | `1` | Устранение фриза при начале гражданских войн |
| `fast_sched` | `HOI4_ACCEL_FAST_SCHED` | `1` | Lock-free планировщик воркеров `PopTaskOrWait` |
| `fast_wait` | `HOI4_ACCEL_FAST_WAIT` | `1` | Lock-free `CTask::Wait` |
| `fast_yield` | `HOI4_ACCEL_FAST_YIELD` | `1` | Перехват циклов `sched_yield` / `SwitchToThread` |
| `fast_event` | `HOI4_ACCEL_FAST_EVENT` | `1` | Прямой `SYS_futex` пула потоков *(только Linux)* |
| `no_job_clock` | `HOI4_ACCEL_NO_JOB_CLOCK` | `1` | Вырезка системных часов на микрозадачах `ExecuteTask` |
| `affinity` | `HOI4_ACCEL_AFFINITY` | `1` | Привязка потоков к физическим ядрам и SMT |
| `quiet_log` | `HOI4_ACCEL_QUIET_LOG` | `1` | Дедупликатор спама `error.log` v2 (нечувствителен к номерам строк `RU_loc.txt`) |
| `fast_save` | `HOI4_ACCEL_FAST_SAVE` | `1` | Асинхронная запись сохранений без фриза диска |
| `no_telemetry` | `HOI4_ACCEL_NO_TELEMETRY` | `1` | Отключение телеметрии Paradox SDK |
| `target_fps` | `HOI4_ACCEL_TARGET_FPS` | от пресета | Целевой FPS при мульти-тике на 5-й скорости (`0` = выкл.) |
| `smooth_fps` | `HOI4_ACCEL_SMOOTH_FPS` | от пресета | Целевой FPS анти-фриза внутри тяжёлых тиков |
| `profile` | `HOI4_ACCEL_PROFILE` | `0` | Запись итогового отчёта по зонам и счётчикам при выходе |

---

## Откат между версиями (`v1.4.0` ↔ `v1.3.0` ↔ `v1.2.0` ↔ `v1.1.0` ↔ `v1.0.0`)

- **Через GitHub Releases (проще всего):** скачайте архив нужного релиза со вкладки **Releases** репозитория и замените `version.dll` (Windows) или `libhoi4_rust_accel.so` (Linux).
- **Точечно без смены версии:** любой модуль (`ai_capture_empty = 0`, `ai_smart_naval = 0`, `ai_smart_armor = 0`, `ai_smart_supply = 0` и т.д.) отключается одной строкой в `hoi4_accel.ini`.
- **Через Git:** `git checkout v1.3.0` (или любую раннюю метку) и пересоберите командами ниже.

## Пересборка обеих версий из исходников

```bash
# Linux (target/release/libhoi4_rust_accel.so)
RUSTFLAGS="-C target-cpu=native" cargo build --release

# Windows (windows/dist/version.dll + windows/dist/hoi4_accel.ini)
bash ./windows/build_win64.sh
```
