# HoI4 Native Engine & AI Accelerator — Версия для Windows (`v1.0-win64`)

Нативный ускоритель движка и оптимизатор ИИ для **Hearts of Iron IV (`hoi4.exe`, Operation Postern v1.20.0.0.8873, 64-бит)** под **Windows 10 / 11 (x86_64)**, а также для запуска Windows-версии `hoi4.exe` через **Proton / Wine**.

Работает как **прозрачный прокси `version.dll`**:
- **Ни один файл игры на диске не изменяется** (`hoi4.exe` остаётся на 100% оригинальным).
- **Не требует параметров запуска в Steam** — Windows автоматически подгружает `version.dll` из папки с `hoi4.exe`, перенаправляя системные вызовы в `C:\Windows\System32\version.dll`.
- **Полная совместимость с модами:** все улучшения движка и ИИ работают из `version.dll` без конфликтов с любыми модами (*Road to 56, Kaiserreich/KaiserreduX, BlackICE, TNO, Millennium Dawn, The Fire Rises* и др.).
- **Безопасность для лаунчера и обновлений:** DLL проверяет имя процесса (`hoi4.exe`), машинные сигнатуры прологов (19/19) и адреса виртуальных таблиц (5/5) перед установкой хуков. Дочерние процессы (`dowser.exe`, `crashreporter.exe`) пропускаются без изменений, а при выходе нового патча HoI4 несовпадающие хуки безопасно отключаются вместо вылета.

---

## Быстрая установка на Windows

Скопируйте два готовых файла из папки [`dist/`](file:///home/gleb/.gemini/antigravity/scratch/hoi4-rust-accel/windows/dist):
1. [`version.dll`](file:///home/gleb/.gemini/antigravity/scratch/hoi4-rust-accel/windows/dist/version.dll)
2. [`hoi4_accel.ini`](file:///home/gleb/.gemini/antigravity/scratch/hoi4-rust-accel/windows/dist/hoi4_accel.ini)

прямо в корневую папку установленной игры рядом с `hoi4.exe` (обычно `...\Steam\steamapps\common\Hearts of Iron IV\`).

Запускайте игру как обычно (через Steam, Paradox Launcher или напрямую `hoi4.exe`).
После старта рядом с `hoi4.exe` появится лог инициализации **`hoi4_accel.log`**, подтверждающий подключение всех модулей `v1.0-win64`.

> **Примечание для Linux + Proton:** если вы запускаете Windows-версию игры в Steam через Proton, помимо помещения `version.dll` и `hoi4_accel.ini` рядом с `hoi4.exe`, укажите в параметрах запуска Steam:
> `WINEDLLOVERRIDES="version=n,b" %command%`
> (на самой Windows этого делать **не нужно**).

---

## Что оптимизировано в `hoi4.exe` (`v1.0-win64`, MSVC x64)

### Новые глубокие оптимизации движка и ИИ в `v1.0`

| Модуль | RVA / VTable в `hoi4.exe` | Что делает |
| :--- | :--- | :--- |
| **`FastAlloc`** | VTable `0x2798570` (`CPdxNewDeleteAllocator`) | Lock-free slab-аллокатор на `128 МБ` (память выделяется страницами по 64 КБ по мере реальной надобности) + Win32 `TlsGetValue` кэш потоков без блокировок `RtlAllocateHeap`/`RtlFreeHeap` для объектов `<= 512 Б` (`CTask`, `CJob`, `CEvent`, `CArray`) |
| **`FastModifier`** | `0x05a63d0` (`CModifier::GetValue`) | Безветвевой бинарный поиск на инструкциях `cmovbe` с точным соблюдением скрытого возврата структуры `CFixedPoint` (`%rdx` → `%rax` в MSVC x64) |
| **`CountryGrain`** | `0x2576ac0` (`CPdxParallelForRange::Init`) | Ставит `chunk_size = 1` для параллельных циклов масштаба стран (`17..=220`), чтобы тяжёлые мажоры (GER/SOV/USA) не блокировали рабочие потоки в хвосте часа/суток |
| **`CivilWarFix`** | `0x1169250` (`CCivilWarSetup::Execute`) + VTable `0x29ea628+0x238` | Устраняет многосекундный фриз при старте гражданских войн: откладывает 100+ промежуточных пересборок карты/побережий/снабжения `CInGameIdler`, mid-tick рендеров и лог-спама внутри цикла передачи областей, выполняя единую чистую пересборку в конце |
| **`AiSmartSupply`** | `.data` `0x33e6124..0x33e7a40` (`NDefines::NAI`) | Проактивная логистика ИИ: проверка узких мест снабжения каждые **24 ч** (вместо 168 ч), апгрейд ЖД при **65%** насыщения узла, повышение приоритета стройки **узлов снабжения (`supply_node`)** с `1.10` до **`4.50`** и **ЖД (`railway`)** с `4.00` до **`8.00`**, глубина поиска узлов `5 → 9`, реакция на кризис снабжения при `25%` вместо `10%` |
| **`AiThrottleMinors`** | VTable `0x27b8c20` (`CCountryAI` `+0x10`, `+0x18`) | Разгружает почасовой ИИ спящих мирных миноров (`< 12` областей, вне войны, не мажор, не игрок): обновление раз в 4 часа со сдвигом по `tag_id` и сохранением точных суточных счётчиков `CAICore` (`+0x10`, `+0x14`) |

### Базовые модули планировщика потоков, кадров и диска

| Модуль | RVA / IAT в `hoi4.exe` | Что исправляет |
| :--- | :--- | :--- |
| **`FastSched`** | `0x255d2a0` (`PopTaskOrWait`) | Убирает холостой atomic-шторм `lock incl/xadd` на `searching_workers` и лишние вызовы таймера при ожидании рабочих потоков (`CEvent` stride `0xa0`) |
| **`FastWait`** | `0x255afb0` (`CTask::SpinWait`) | Заменяет жёсткий цикл вызовов ядра при ожидании задач (`job+0x60`) на энергоэффективный `_mm_pause()` |
| **`FastYield`** | IAT `0x27892c0` (`SwitchToThread`) | Устраняет шторм системных вызовов `SwitchToThread` внутри параллельного барьера `gamestate.hourly` (`0x1ee8cf`) |
| **`NoJobClock`** | `0x255d0f3`, `0x255d186` (`ExecuteTask`) | Вырезает двойной замер системных часов и `lock add` вокруг каждой микрозадачи пула потоков |
| **`SmoothFPS`** | `0x02bacd0` (`ShouldRenderMidTick`) | ~5 нс `RDTSC`-фильтр + анти-фриз рендер 30 FPS (60 FPS при движении камеры) вместо стандартного провала Paradox до 5–13 FPS |
| **`TargetFPS`** | `0x2297b50` + `0x0f88c10` (`UpdateSession`) | Адаптивный мульти-тик на 5-й скорости в рамках бюджета кадра (строго только в одиночной игре и без паузы) |
| **`Affinity`** | `0x255a110` (`WorkerThreadInit`) | Привязка главного потока к CPU 0, основных `Task_N` к физическим ядрам, фоновых `Task_BN` к SMT-потокам (на гибридных P+E процессорах Intel управление автоматически оставляется планировщику Windows 11) |
| **`QuietLog`** | `0x2550050` (`SetLogger`, `g_pLogger` `0x3695af8`) | Lock-free дедупликатор спама `error.log` под 32-байтный MSVC `std::string` (оставляет первые 8 записей каждого типа ошибки) |
| **`FastSave`** | `0x259d9e0` (`CFile::Flush`) | Убирает блокирующий сброс буферов диска `FlushFileBuffers()` при автосохранениях и Ironman |
| **`NoTelemetry`** | IAT `0x2789a90` (`QueueAndSendTelemetry`) | Чисто отключает отправку телеметрии Paradox SDK с корректным вызовом MSVC-деструктора `~TelemetryPayload` (`0x2789a20`) без утечек памяти |

---

## Настройка (`hoi4_accel.ini`)

Откройте `hoi4_accel.ini` любым текстовым редактором:
- `preset = smooth` *(по умолчанию)* — плавный интерфейс (~30+ FPS на 5-й скорости, 60 FPS при движении камеры) + все оптимизации движка и ИИ.
- `preset = balanced` — более агрессивная промотка времени на 5-й скорости (~14–18 FPS).
- `preset = turbo` — максимальная скорость промотки кампании.
- `preset = pure` — все внутренние оптимизации памяти, потоков, ИИ и анти-фриз 30 FPS без мульти-тика.
- `preset = none` — полное отключение всех патчей без удаления файла.
- Любой отдельный модуль `v1.0` (`fast_alloc`, `fast_mod`, `country_grain`, `civil_war_fix`, `ai_smart_supply`, `ai_throttle_minors`) можно индивидуально выключить, раскомментировав строку и поставив `= 0`.
- `profile = 1` — включает сбор статистики по зонам движка и счётчикам всех оптимизаций, дописывая итоговый отчёт `HOI4 ACCEL v1.0-win64 REPORT` в `hoi4_accel.log` при выходе из игры.

## Удаление

Удалите `version.dll` и `hoi4_accel.ini` из папки с `hoi4.exe`.

## Пересборка из исходников (кросс-компиляция на Linux)

```bash
bash ./windows/build_win64.sh
```
Артефакты собираются в `windows/dist/version.dll` и `windows/dist/hoi4_accel.ini` с помощью `x86_64-w64-mingw32-g++` без внешних зависимостей от MinGW DLL.
