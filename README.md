# hoi4-rust-accel (`v1.2` — Linux & Windows)

Нативный ускоритель движка Clausewitz и нечитерный усилитель ИИ для **Hearts of Iron IV** (`Operation Postern v1.20.0.0.8873`, 64-бит) в двух синхронизированных сборках:
- **Linux (`LD_PRELOAD`, нативный Rust):** `libhoi4_rust_accel.so` + лаунчер `launch_hoi4.sh`
- **Windows (`version.dll` прокси, C++20 MSVC x64 ABI):** `windows/dist/version.dll` (подробная инструкция: `windows/README_WINDOWS.md`)
- **Единый конфиг для обеих ОС:** `hoi4_accel.ini` (либо переменные окружения `HOI4_ACCEL_*`).

### Ключевые гарантии
- **100% неинвазивно:** оригинальные бинарники игры (`hoi4` и `hoi4.exe`) и игровые файлы на диске не изменяются ни на один байт.
- **Без читов и нарушения баланса:** ускоритель не трогает `NMilitary` / `NProduction` / `NCountry` (правила боя и производства одинаковы для игрока и бота) и не рисует боту бесплатные заводы или снаряжение. Все улучшения ИИ работают строго через весовые коэффициенты принятия решений `NDefines::NAI` / `NAITheatre`, точечный перехват очереди строительства и расчёта смежности на мирных конференциях.
- **Нулевые конфликты с модами:** работает прямо из `.so` / `version.dll` с ваниллой, всеми DLC, сохранениями и модами (*Road to 56, Kaiserreich/KaiserreduX, BlackICE, TNO, Millennium Dawn, The Fire Rises* и др.).
- **Версионность и быстрый откат:** в GitHub Releases доступны все версии (`v1.0.0`, `v1.1.0`, `v1.2.0`), а любой модуль можно мгновенно отключить флагом `= 0` в `hoi4_accel.ini`.

---

## Быстрый запуск

### Windows
Скопируйте `version.dll` и `hoi4_accel.ini` из `windows/dist/` (или из архива релиза `hoi4-accel-v1.2.0-windows-x64.zip`) прямо в папку с `hoi4.exe` и запускайте игру как обычно. Подробнее — в `windows/README_WINDOWS.md`.

### Linux (Steam)
В Steam: **ПКМ по Hearts of Iron IV → Свойства → Параметры запуска**:
```bash
/путь/к/hoi4-accel/launch_hoi4.sh %command%
```
> **Важно:** не оставляйте флаг `-debug` в параметрах запуска при обычной игре — в режиме `-debug` сам движок HoI4 непрерывно пишет мегабайты файлов в `logs/metrics/`, заметно тормозя симуляцию.

---

## Что нового в `v1.2` (20 модулей движка и ИИ)

1. **`AiSmartPeace` (`ai_smart_peace = 1`, НОВОЕ в `v1.2` — анти-бордергор мирных конференций):**
   - точечный перехват `CPeaceWinnerAi::CalcStateActionScore` (`0xeafef0` Linux / `0x1a8ca20` Windows) + чтение `CPeaceConference::IsStateClaimedBy` и списка соседних областей `CStateData::m_NeighborStates`;
   - при аннексии (`take_states`) и марионетках (`puppet`) области, граничащие со своей территорией **или уже заявленными в этой же мирной конференции областями**, получают бонус **`3x..5x`** (чем больше общих границ, тем выше приоритет — бот закрывает «дыры» и строит цельные границы);
   - разрозненные сухопутные анклавы получают штраф **`-95%`** (`0.05x`), удалённые прибрежные — **`-75%`** (`0.25x`, что позволяет морским державам вроде Великобритании/США взять первый плацдарм на побережье, а со следующего хода расширять именно его без разбросанных «клякс»);
   - **национальные корки (`IsCoreOf`) не штрафуются никогда**, чтобы страны свободно возвращали свои исторические анклавы.
2. **`AiTacticalPrudence v2` — фикс «затупа на пустых клетках» + сохранение снаряжения (`ai_tactical_prudence = 1`, 40 параметров `NDefines::NAI`):**
   - **Исправлен затуп перед пустыми клетками:** убрано завышение `PLAN_ATTACK_MIN_STRENGTH_FACTOR_*` (движок `CUnitOrder` проверял этот порог даже перед шагом в **пустую** вражескую клетку!), снижен `ENTRENCHMENT_WEIGHT` (`4.0 → 2.2`) и `AIFC_ACTIVATE_AVG_ORG_RATIO_THRESHOLD` (`0.42 → 0.30`), выставлен `LOCATION_BALANCE_TO_ADVANCE = 0.00` (как в *Sheep AI*);
   - **Конец сжиганию снаряжения в глубокий минус:** задействованы рычаги, которые **не мешают** заходу на пустые клетки (у пустой клетки защита `0`), но жёстко режут мясные штурмы укреплённых позиций и бездумный спам дивизий: `ATTACK_HEAVILY_DEFENDED_LIMIT 0.50 → 0.95`, `PLAN_VALUE_TO_EXECUTE -0.50 → -0.10`, отмена проигрышных боёв за **12 часов** при перевесе `1.15x` (вместо 48 ч / `1.50x`), отмена планов наступления при просадке 45% дивизий ниже 50% прочности, **пауза набора новых дивизий пока армия в поле не укомплектована на 88%**, **стоп учений при снаряжении < 75%** и запас на складах `100%`.
3. **`QuietLog v2` (`quiet_log = 1`):** убрано хеширование номера строки `line` и цифр в пути файла, таблица расширена до `8192` слотов — полностью убивает 100-мегабайтный спам `error.log` от русской локализации (`RU_loc.txt` на 15 000+ разных строках).
4. **Паритет MP-защиты и фикс `launch_hoi4.sh`:** на Linux добавлена авто-проверка `num_players > 1` (автоотключение мульти-тика в мультиплеере как на Windows), а `launch_hoi4.sh` теперь сначала ищет `libhoi4_rust_accel.so` прямо в папке релиза.

### Проверенные модули `v1.0` → `v1.1` → `v1.2`
- **`AiSmartAirbase` (`ai_smart_airbase = 1`)** — приоритет нужных фронтовых `air_base` поднят в `3.2x` (`0.50 → 1.60`) + перебазирование крыльев при покрытии `< 75%`.
- **`AiSmartSupply` (`ai_smart_supply = 1`)** — ежедневная проверка снабжения (`24ч`), апгрейд ЖД при `65%`, моторизация хабов грузовиками (`95%`), приоритет стройки портов/узлов (`4.50`) и ЖД (`8.00`), глубина BFS `5 → 9`.
- **`FastAlloc` / `FastModifier` / `CountryGrain` / `CivilWarFix` / `AiThrottleMinors` / `FastSched` / `FastWait` / `FastYield` / `FastEvent` / `NoJobClock` / `Affinity` / `SmoothFPS` / `TargetFPS` / `FastSave` / `NoTelemetry`**.

---

## Конфигурация (`hoi4_accel.ini` и переменные окружения)

| Ключ в `hoi4_accel.ini` | Переменная окружения | По умолчанию | Описание |
| :--- | :--- | :---: | :--- |
| `preset` | `HOI4_ACCEL_PRESET` | `smooth` | `smooth` / `balanced` / `turbo` / `pure` (MP) / `none` |
| `ai_smart_peace` | `HOI4_ACCEL_AI_SMART_PEACE` | `1` | Анти-бордергор на мирной конференции (бонус смежности `3x..5x`, штраф анклавам `-95%`) |
| `ai_tactical_prudence` | `HOI4_ACCEL_AI_TACTICAL_PRUDENCE` | `1` | Рассудительная тактика v2: свободный захват пустых клеток, сбережение снаряжения и кулаки `AIFC` |
| `ai_smart_airbase` | `HOI4_ACCEL_AI_SMART_AIRBASE` | `1` | Умное расширение фронтовых авиабаз (`air_base` prio `3.2x`) и покрытие миссий |
| `ai_smart_supply` | `HOI4_ACCEL_AI_SMART_SUPPLY` | `1` | Умная постройка ЖД, портов/узлов снабжения и моторизация хабов |
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

## Откат между версиями (`v1.2.0` ↔ `v1.1.0` ↔ `v1.0.0`)

- **Через GitHub Releases (проще всего):** скачайте архив нужного релиза (`v1.2.0`, `v1.1.0` или `v1.0.0`) со вкладки **Releases** репозитория и замените `version.dll` (Windows) или `libhoi4_rust_accel.so` (Linux).
- **Точечно без смены версии:** любой модуль (`ai_smart_peace = 0`, `ai_tactical_prudence = 0`, `ai_smart_airbase = 0` и т.д.) отключается одной строкой в `hoi4_accel.ini`.
- **Через Git:** `git checkout v1.1.0` (или `v1.0.0`) и пересоберите командами ниже.

## Пересборка обеих версий из исходников

```bash
# Linux (target/release/libhoi4_rust_accel.so)
RUSTFLAGS="-C target-cpu=native" cargo build --release

# Windows (windows/dist/version.dll + windows/dist/hoi4_accel.ini)
bash ./windows/build_win64.sh
```
