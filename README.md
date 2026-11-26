# hoi4-rust-accel (`v1.0` — Linux & Windows)

Нативный ускоритель движка Clausewitz и оптимизатор ИИ для **Hearts of Iron IV** (`Operation Postern v1.20.0.0.8873`, 64-бит) в двух синхронизированных сборках:
- **Linux (`LD_PRELOAD`, нативный Rust):** [`libhoi4_rust_accel.so`](file:///home/gleb/.gemini/antigravity/scratch/hoi4-rust-accel/target/release/libhoi4_rust_accel.so) + лаунчер [`launch_hoi4.sh`](file:///home/gleb/.gemini/antigravity/scratch/hoi4-rust-accel/launch_hoi4.sh)
- **Windows (`version.dll` прокси, C++20 MSVC x64 ABI):** [`windows/dist/version.dll`](file:///home/gleb/.gemini/antigravity/scratch/hoi4-rust-accel/windows/dist/version.dll) (подробная инструкция: [`windows/README_WINDOWS.md`](file:///home/gleb/.gemini/antigravity/scratch/hoi4-rust-accel/windows/README_WINDOWS.md))
- **Единый конфиг для обеих ОС:** [`hoi4_accel.ini`](file:///home/gleb/.gemini/antigravity/scratch/hoi4-rust-accel/hoi4_accel.ini) (либо переменные окружения `HOI4_ACCEL_*`).

### Ключевые гарантии
- **100% неинвазивно:** оригинальные бинарники игры (`hoi4` и `hoi4.exe`) и игровые файлы на диске не изменяются ни на один байт.
- **Нулевые конфликты с модами:** все улучшения движка и ИИ работают прямо из `.so` / `version.dll` без отдельных файлов мода — полная совместимость с ваниллой, всеми DLC, сохранениями и крупными модами (*Road to 56, Kaiserreich/KaiserreduX, BlackICE, TNO, Millennium Dawn, The Fire Rises* и др.).
- **Проверка машинных сигнатур:** каждый перехват пролога и слота `vtable` сначала сверяет эталонные байты машинного кода — при обновлении игры несовпадающий модуль безопасно пропустится без вылета.

---

## Быстрый запуск на Linux (Steam)

В Steam: **ПКМ по Hearts of Iron IV → Свойства → Параметры запуска**:

```bash
/home/gleb/.gemini/antigravity/scratch/hoi4-rust-accel/launch_hoi4.sh %command%
```

Все настройки и пресеты (`smooth`, `balanced`, `turbo`, `pure`, `none`, а также включение/отключение отдельных модулей ИИ и движка) можно менять прямо в файле [`hoi4_accel.ini`](file:///home/gleb/.gemini/antigravity/scratch/hoi4-rust-accel/hoi4_accel.ini) без перезаписи параметров запуска Steam!

Для записи подробного отчёта профилировщика зон и счётчиков всех модулей по выходу из игры:
```bash
HOI4_ACCEL_PROFILE=1 HOI4_ACCEL_SAMPLE=1 HOI4_ACCEL_LOG=/tmp/hoi4_v10_profile.log /home/gleb/.gemini/antigravity/scratch/hoi4-rust-accel/launch_hoi4.sh %command%
```

---

## Что входит в `v1.0` (17 модулей движка и ИИ)

### Новое в `v1.0` — глубокая оптимизация памяти, модификаторов, гражданских войн и ИИ
1. **`FastAlloc` (`fast_alloc = 1`, `CPdxNewDeleteAllocator` @ `0x4349510`)** — lock-free slab-арена на `128 МБ` (demand-paged через `mmap`, неиспользуемые страницы занимают `0 КБ` RAM) с ABA-защищённым стеком Трейбера (36-битное поколение) и попоточным `TLS`-магазином для классов `64 / 128 / 256 / 512` байт. Убирает межпоточные блокировки кучи при миллионах аллокаций/освобождений микрозадач `CTask`, `CJob` и буферов `CArray` между главным потоком и воркерами, безопасно передавая старые или крупные (`> 512 Б`) указатели штатному аллокатору.
2. **`FastModifier` (`fast_mod = 1`, `CModifier::GetValue` @ `0x2241b00`)** — включён по умолчанию во всех пресетах: безветвевой бинарный поиск модификаторов на инструкциях `CMOV` (`select_unpredictable`), полностью устраняющий штрафы конвейера CPU на branch misprediction при миллионах обращений к модификаторам страны/дивизий/провинций.
3. **`CivilWarFix` (`civil_war_fix = 1`, `CCivilWarSetup::Execute` @ `0x1bdfa80` + `CInGameIdler` @ `0x4467770+0x240`)** — устраняет многосекундный мёртвый фриз при начале крупных гражданских войн (например, в СССР или Испании). В ванилле передача каждой из 80–120 областей вызывала полную пересборку графического оверлея карты/побережий/снабжения `CInGameIdler` прямо посреди цикла. Теперь промежуточные пересборки, mid-tick рендеры и спам предупреждений в `error.log` коалесцируются и выполняются **ровно 1 раз** сразу после завершения `CCivilWarSetup::Execute`.
4. **`AiSmartSupply` (`ai_smart_supply = 1`, `NDefines::NAI` / `NAITheatre`)** — обучает штатный C++-модуль логистики ИИ (`FixSupplyBottlenecks` + `SupplyHubBuilder`) вовремя устранять голод снабжения на фронте:
   - проверка узких мест снабжения проводится **раз в сутки (`24 ч`)** вместо раза в неделю (`168 ч`);
   - упреждающая прокладка и апгрейд железных дорог (`railway`) включается уже при **65%** загрузки узла (вместо ожидания `85%`);
   - базовый приоритет стройки **узлов снабжения и портов (`supply_node`)** в очереди фабрик повышен с `1.10` до **`4.50`**, а **железных дорог (`railway`)** — с `4.00` до **`8.00`**, чтобы критическая логистика получала топ-15 гражданских фабрик, а не висела в хвосте очереди за военными заводами;
   - глубина BFS-поиска узлов снабжения ТВД расширена с `5` до **`9`**, а порог реакции на кризис снабжения поднят с `10%` до **`25%`**.
   - *Совместимость с модами:* применяется лениво после загрузки `00_defines.lua` и модов через «умное ограничение» (если мод уже выставил ещё более агрессивный параметр, значение мода сохраняется).
5. **`AiThrottleMinors` (`ai_throttle_minors = 1`, `CCountryAI` @ `0x434b3b8`)** — разгружает почасовой параллельный проход `ai_update` от 55+ спящих мирных стран-миноров (Бутан, Тибет, Уругвай, Непал, Панама, Гаити и т.д.). Применяется **строго** только если страна: ИИ (не игрок и не кооп), в полном мире (`IsAtWar == 0`), не мажор (`IsMajor == 0`) и контролирует `< 12` областей. Такие миноры обновляются со сдвигом фазы по `tag_id` раз в 4 часа и обязательно в полночь (`00:00`), при этом внутренние счётчики `CAICore` (`+0x10`, `+0x14`) аккуратно декрементируются, поэтому все суточные тики, фокусы и дипломатия срабатывают час-в-час, а при вступлении в войну страна мгновенно переходит на 100% частоту.

### Проверенные модули ядра планировщика и ввода-вывода (`v0.9` → `v1.0`)
6. **`FastSched` (`0x3666d70`)** — lock-free планировщик воркеров без atomic spin-шторма на `searching_workers`.
7. **`FastWait` (`0x3662b10`)** — ожидание задач `CTask::Wait` на `_mm_pause()` без шторма `sched_yield()`.
8. **`FastYield` (`0x5140d0`)** — перехват внутренних циклов `sched_yield@plt` внутри `gamestate.hourly`.
9. **`SmoothFPS` (`0x20b07a0`)** — анти-фриз фреймпейсер с ~5 нс `RDTSC`-фильтром (30 FPS в тяжёлых тиках, 60 FPS при движении камеры).
10. **`TargetFPS` (`0x203be20` + `0x20ef870`)** — адаптивный мульти-тик на 5-й скорости строго в рамках бюджета кадра.
11. **`CountryGrain` (`0x366b4c0`)** — `chunk_size = 1` для параллельных циклов масштаба стран (`17..=220`).
12. **`Affinity`** — топологическая раскладка потоков по физическим ядрам и SMT-потокам.
13. **`FastEvent`** — синхронизация пула воркеров на прямом Linux `SYS_futex`.
14. **`NoJobClock` (`0x3666c39`, `0x3666cc9`)** — удаление лишних вызовов часов и `lock add` на каждой микрозадаче.
15. **`QuietLog` (`0x361bec0`)** — lock-free дедупликатор спама `error.log` (оставляет первые 8 записей на уникальный шаблон).
16. **`FastSave` (`0x3672b60`)** — отключение блокирующего синхронного `fsync()` при автосохранениях.
17. **`NoTelemetry` (`0x315d2af`)** — чистое отключение отправки телеметрии Paradox SDK.

---

## Конфигурация (`hoi4_accel.ini` и переменные окружения)

Любой параметр можно задать в [`hoi4_accel.ini`](file:///home/gleb/.gemini/antigravity/scratch/hoi4-rust-accel/hoi4_accel.ini) (секция `[hoi4_accel]`) или через переменную окружения (имеет приоритет над `.ini`):

| Ключ в `hoi4_accel.ini` | Переменная окружения | По умолчанию | Описание |
| :--- | :--- | :---: | :--- |
| `preset` | `HOI4_ACCEL_PRESET` | `smooth` | `smooth` / `balanced` / `turbo` / `pure` / `none` |
| `fast_alloc` | `HOI4_ACCEL_FAST_ALLOC` | `1` | Lock-free slab + TLS аллокатор (`128 МБ`) для `<= 512 Б` |
| `fast_mod` | `HOI4_ACCEL_FAST_MOD` | `1` | Безветвевой `CMOV` бинарный поиск `CModifier::GetValue` |
| `country_grain` | `HOI4_ACCEL_COUNTRY_GRAIN` | `1` | `chunk=1` для параллельных циклов стран (`17..=220`) |
| `civil_war_fix` | `HOI4_ACCEL_CIVIL_WAR_FIX` | `1` | Устранение фриза при начале гражданских войн |
| `ai_smart_supply` | `HOI4_ACCEL_AI_SMART_SUPPLY` | `1` | Умная постройка ЖД и узлов снабжения ботами при дефиците |
| `ai_throttle_minors` | `HOI4_ACCEL_AI_THROTTLE_MINORS` | `1` | Разгрузка почасового ИИ спящих мирных миноров (`<12` обл.) |
| `fast_sched` | `HOI4_ACCEL_FAST_SCHED` | `1` | Lock-free планировщик воркеров `PopTaskOrWait` |
| `fast_wait` | `HOI4_ACCEL_FAST_WAIT` | `1` | Lock-free `CTask::Wait` |
| `fast_yield` | `HOI4_ACCEL_FAST_YIELD` | `1` | Перехват циклов `sched_yield` / `SwitchToThread` |
| `fast_event` | `HOI4_ACCEL_FAST_EVENT` | `1` | Прямой `SYS_futex` пула потоков *(только Linux)* |
| `no_job_clock` | `HOI4_ACCEL_NO_JOB_CLOCK` | `1` | Вырезка системных часов на микрозадачах `ExecuteTask` |
| `affinity` | `HOI4_ACCEL_AFFINITY` | `1` | Привязка потоков к физическим ядрам и SMT |
| `quiet_log` | `HOI4_ACCEL_QUIET_LOG` | `1` | Дедупликатор спама `error.log` |
| `fast_save` | `HOI4_ACCEL_FAST_SAVE` | `1` | Асинхронная запись сохранений без фриза диска |
| `no_telemetry` | `HOI4_ACCEL_NO_TELEMETRY` | `1` | Отключение телеметрии Paradox SDK |
| `target_fps` | `HOI4_ACCEL_TARGET_FPS` | от пресета | Целевой FPS при мульти-тике на 5-й скорости (`0` = выкл.) |
| `smooth_fps` | `HOI4_ACCEL_SMOOTH_FPS` | от пресета | Целевой FPS анти-фриза внутри тяжёлых тиков |
| `profile` | `HOI4_ACCEL_PROFILE` | `0` | Запись итогового отчёта по зонам и счётчикам при выходе |

---

## Пересборка обеих версий из исходников

```bash
cd /home/gleb/.gemini/antigravity/scratch/hoi4-rust-accel
# Linux (.so)
RUSTFLAGS="-C target-cpu=native" cargo build --release
# Windows (windows/dist/version.dll + hoi4_accel.ini)
./windows/build_win64.sh
```
