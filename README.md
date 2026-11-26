# hoi4-rust-accel (`v1.3` — Linux & Windows)

Нативный ускоритель движка Clausewitz и нечитерный усилитель ИИ для **Hearts of Iron IV** (`Operation Postern v1.20.0.0.8873`, 64-бит) в двух синхронизированных сборках:
- **Linux (`LD_PRELOAD`, нативный Rust):** `libhoi4_rust_accel.so` + лаунчер `launch_hoi4.sh`
- **Windows (`version.dll` прокси, C++20 MSVC x64 ABI):** `windows/dist/version.dll` (подробная инструкция: `windows/README_WINDOWS.md`)
- **Единый конфиг для обеих ОС:** `hoi4_accel.ini` (либо переменные окружения `HOI4_ACCEL_*`).

### Ключевые гарантии
- **100% неинвазивно:** оригинальные бинарники игры (`hoi4` и `hoi4.exe`) и игровые файлы на диске не изменяются ни на один байт.
- **Без читов и нарушения баланса:** ускоритель не трогает `NMilitary` / `NProduction` / `NCountry` (правила боя и производства одинаковы для игрока и бота) и не рисует боту бесплатные заводы, танки или снаряжение. Все улучшения ИИ работают строго через решения ИИ, нативные команды строительства (`CAddConstructionCommand`) и динамические веса стратегий.
- **Нулевые конфликты с модами:** работает прямо из `.so` / `version.dll` с ваниллой, всеми DLC, сохранениями и модами (*Road to 56, Kaiserreich/KaiserreduX, BlackICE, TNO, Millennium Dawn, The Fire Rises* и др.).
- **Версионность и быстрый откат:** в GitHub Releases доступны все версии (`v1.0.0`, `v1.1.0`, `v1.2.0`, `v1.3.0`), а любой модуль можно мгновенно отключить флагом `= 0` в `hoi4_accel.ini`.

---

## Быстрый запуск

### Windows
Скопируйте `version.dll` и `hoi4_accel.ini` из `windows/dist/` (или из архива релиза `hoi4-accel-v1.3.0-windows-x64.zip`) прямо в папку с `hoi4.exe` и запускайте игру как обычно. Подробнее — в `windows/README_WINDOWS.md`.

### Linux (Steam)
В Steam: **ПКМ по Hearts of Iron IV → Свойства → Параметры запуска**:
```bash
/путь/к/hoi4-accel/launch_hoi4.sh %command%
```
> **Важно:** не оставляйте флаг `-debug` в параметрах запуска при обычной игре — в режиме `-debug` сам движок HoI4 непрерывно пишет мегабайты файлов в `logs/metrics/`, заметно тормозя симуляцию.

---

## Что нового в `v1.3` (21 модуль движка и ИИ)

1. **ПОЛНЫЙ фикс затупа на пустых клетках (`ai_tactical_prudence = 1`, v3):**
   - **Неактивные фронты:** в ванильном движке порог микроатак `MIN_PLAN_VALUE_TO_MICRO_INACTIVE (+0.15)` попадал в мёртвую зону относительно `PLAN_VALUE_TO_EXECUTE (-0.10)`, из-за чего бот полностью отключал захват пустых клеток на пассивных участках фронта. В `v1.3` порог снижен до `-1.00`, а лимит `MAX_MICRO_ATTACKS_PER_ORDER` поднят до `10` (на **занятые** врагом клетки по-прежнему требуется локальный перевес — суицид об окопы не возвращается);
   - **Активные батлпланы:** точечный проверенный патч `CUnitOrder` (`0x2569860` Linux / `0x1573929, 0x1573933` Windows) снимает проверку `too_weak` **исключительно** на ветке нулевого гарнизона (`HasLandUnitsInProvince == 0`), оставляя проверку `CanMoveIntoProvince` (мин. организация, проходимость, отступление) полностью нетронутой.
2. **Нативный автостроитель сухопутных узлов снабжения (`ai_smart_supply = 1`, v2):**
   - В ванильном C++ HoI4 метод `FixSupplyBottlenecks` умеет только улучшать ЖД и никогда не ставит новые сухопутные `supply_node`. В `v1.3` перехват `FixSupplyBottlenecks` (`0x742d40` Linux / `0x13a9fc0` Windows) для воюющих ИИ-стран с промышленностью `>= 20` заводов находит горящие сектора фронта (`>= 3` голодающих дивизий с `< 50%` снабжения), выбирает безопасную тыловую клетку в 1 шаге от передовой, проверяет 3-шаговым BFS отсутствие ближайших хабов/портов и ставит **строго 1 активный `supply_node`** через нативную MP-безопасную команду `CAddConstructionCommand` + `CSession::Post` (с кулдауном 30 дней). Далее ванильный `LinkUnlinkedNodes` сам тянет к нему железную дорогу.
3. **Умный танковый и механизированный ИИ (`ai_smart_armor = 1`, НОВОЕ в `v1.3`):**
   - Исправлена причина, по которой в 1943–1945 ИИ спамил чистую пехоту и игнорировал танки:
     - `PRODUCTION_EQUIPMENT_SURPLUS_FACTOR` снижен (`1.00 → 0.55`), чтобы ИИ перестал забивать все военные заводы 100%-м запасом винтовок, а `MIN_FIELD_STRENGTH_TO_BUILD_UNITS` смягчён (`0.88 → 0.78`);
     - настроены 9 параметров апгрейда и танковых шаблонов (`UPGRADES_DEFICIT_LIMIT_DAYS 60 → 210`, `UPGRADE_PERCENTAGE_OF_FORCES 0.20 → 0.35`, `UPGRADE_DIVISION_RELUCTANCE 7 → 4`, пороги XP конструктора танков `35 → 20`, `ASSIGN_TANKS_TO_WAR_FRONT 6 → 10`);
     - виртуальный хук `CAIStrategyStore::GetStrategyValue` (`0x434ea90+0x28` Linux / `0x27b8be0+0x20` Windows) с мод-безопасным разрешением токенов через `CToken::FromString` плавно масштабирует `role_ratio` и `template_prio` для `armor` / `mechanized` (и мягко снижает перекос чистой `infantry`) строго в зависимости от реальной военной промышленности (`>= 22 / 35 / 55` военных заводов) и года (`1938+ / 1940+ / 1941+`) — без читов и без перегрузки миноров.

### Проверенные модули `v1.0` → `v1.1` → `v1.2` → `v1.3`
- **`AiSmartPeace` (`ai_smart_peace = 1`)** — анти-бордергор мирных конференций (`3x..5x` за смежность со своими/уже заявленными областями, `-95%` за сухопутные анклавы, свои корки не штрафуются).
- **`AiSmartAirbase` (`ai_smart_airbase = 1`)** — приоритет нужных фронтовых `air_base` поднят в `3.2x` (`0.50 → 1.60`) + перебазирование крыльев при покрытии `< 75%`.
- **`QuietLog v2` (`quiet_log = 1`)** — дедупликатор спама `error.log` (нечувствителен к номерам строк `RU_loc.txt`).
- **`FastAlloc` / `FastModifier` / `CountryGrain` / `CivilWarFix` / `AiThrottleMinors` / `FastSched` / `FastWait` / `FastYield` / `FastEvent` / `NoJobClock` / `Affinity` / `SmoothFPS` / `TargetFPS` / `FastSave` / `NoTelemetry`**.

---

## Конфигурация (`hoi4_accel.ini` и переменные окружения)

| Ключ в `hoi4_accel.ini` | Переменная окружения | По умолчанию | Описание |
| :--- | :--- | :---: | :--- |
| `preset` | `HOI4_ACCEL_PRESET` | `smooth` | `smooth` / `balanced` / `turbo` / `pure` (MP) / `none` |
| `ai_smart_armor` | `HOI4_ACCEL_AI_SMART_ARMOR` | `1` | Умный танковый/мех. ИИ (`>=22` воен. заводов, 1938+, апгрейды и шаблоны танков) |
| `ai_smart_supply` | `HOI4_ACCEL_AI_SMART_SUPPLY` | `1` | Логистика v2: ЖД, моторизация + нативная автопостройка `supply_node` на горящих фронтах |
| `ai_tactical_prudence` | `HOI4_ACCEL_AI_TACTICAL_PRUDENCE` | `1` | Тактика v3: полный фикс пустых клеток, сбережение снаряжения и кулаки `AIFC` |
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

## Откат между версиями (`v1.3.0` ↔ `v1.2.0` ↔ `v1.1.0` ↔ `v1.0.0`)

- **Через GitHub Releases (проще всего):** скачайте архив нужного релиза (`v1.3.0`, `v1.2.0`, `v1.1.0` или `v1.0.0`) со вкладки **Releases** репозитория и замените `version.dll` (Windows) или `libhoi4_rust_accel.so` (Linux).
- **Точечно без смены версии:** любой модуль (`ai_smart_armor = 0`, `ai_smart_supply = 0`, `ai_tactical_prudence = 0`, `ai_smart_peace = 0` и т.д.) отключается одной строкой в `hoi4_accel.ini`.
- **Через Git:** `git checkout v1.2.0` (или `v1.1.0` / `v1.0.0`) и пересоберите командами ниже.

## Пересборка обеих версий из исходников

```bash
# Linux (target/release/libhoi4_rust_accel.so)
RUSTFLAGS="-C target-cpu=native" cargo build --release

# Windows (windows/dist/version.dll + windows/dist/hoi4_accel.ini)
bash ./windows/build_win64.sh
```
