//! hoi4-rust-accel v1.4: Native Rust Engine & AI Accelerator for Hearts of Iron IV (Linux x86_64 v1.20.0.0)
//!
//! Features (non-destructive LD_PRELOAD, configurable via `hoi4_accel.ini` or `HOI4_ACCEL_*` env vars):
//!   1. fast_sched / HOI4_ACCEL_FAST_SCHED=1       : Lock-free zero-bounce Rust worker scheduler (PopTaskOrWait @ 0x3666d70).
//!   2. fast_wait / HOI4_ACCEL_FAST_WAIT=1         : Lock-free CTask::Wait (0x3662b10) without tight sched_yield() traps.
//!   3. fast_yield / HOI4_ACCEL_FAST_YIELD=1       : Patches hoi4 sched_yield@plt (0x5140d0) — eliminates spin-wait syscall storms.
//!   4. smooth_fps / HOI4_ACCEL_SMOOTH_FPS=<fps>   : Mid-Tick Anti-Freeze Frame Pacer (ShouldRenderMidTick @ 0x20b07a0).
//!   5. target_fps / HOI4_ACCEL_TARGET_FPS=<fps>   : Render-Aware Multi-Tick Speed-5 Bursting (UpdateSession @ 0x203be20, auto-disabled in MP).
//!   6. affinity / HOI4_ACCEL_AFFINITY=1           : Topology-aware physical/SMT core pinning for main & worker threads.
//!   7. fast_event / HOI4_ACCEL_FAST_EVENT=1       : Lock-free atomic + Linux SYS_futex worker pool synchronization.
//!   8. no_job_clock / HOI4_ACCEL_NO_JOB_CLOCK=1   : Strips per-microjob system_clock::now() + atomic lock add in ExecuteTask.
//!   9. country_grain / HOI4_ACCEL_COUNTRY_GRAIN=1 : Adaptive chunk=1 for 17..=220 country-scale PdxParallelFor loops.
//!  10. fast_mod / HOI4_ACCEL_FAST_MOD=1           : Branchless CMOV binary search CModifier::GetValue (0x2241b00).
//!  11. fast_alloc / HOI4_ACCEL_FAST_ALLOC=1       : 128 MB lock-free ABA-safe slab + TLS magazine on CPdxNewDeleteAllocator (0x4349510).
//!  12. civil_war_fix / HOI4_ACCEL_CIVIL_WAR_FIX=1 : Coalesces 100+ per-state map/supply overlay rebuilds in CCivilWarSetup::Execute (0x1bdfa80).
//!  13. ai_smart_supply / HOI4_ACCEL_AI_SMART_SUPPLY=1 : Proactive AI railway, motorization & native wartime land `supply_node` auto-builder v3 (FixSupplyBottlenecks @ 0x742d40, 2-hop BFS, >=12 fac).
//!  14. ai_capture_empty / HOI4_ACCEL_AI_CAPTURE_EMPTY=1 : Native MP-safe `CMoveCommand` empty enemy tile capture for idle divisions with pocket-safety filter (0x253e000 + CSession::Post).
//!  15. ai_smart_airbase / HOI4_ACCEL_AI_SMART_AIRBASE=1 : Boosts needed frontline air_base queue prio (0x74f4d0) + coverage rebasing.
//!  16. ai_tactical_prudence / HOI4_ACCEL_AI_TACTICAL_PRUDENCE=1 : Non-cheating Sheep-AI tactical prudence, anti-frontline-shuffling, equipment conservation, AIFC spearheads & empty-tile patch.
//!  17. ai_smart_naval / HOI4_ACCEL_AI_SMART_NAVAL=1 : Stops 1-div portless suicide naval invasions & concentrates Strike Force battle fleets (15 NDefines::NAI).
//!  18. ai_smart_armor / HOI4_ACCEL_AI_SMART_ARMOR=1 : Dynamic industrial-scaled armor/mechanized strategy & tank template/upgrade AI (CAIStrategyStore::GetStrategyValue @ 0x434ea90+0x28).
//!  19. ai_smart_peace / HOI4_ACCEL_AI_SMART_PEACE=1 : Anti-bordergore peace conference scoring (CPeaceWinnerAi::CalcStateActionScore @ 0xeafef0).
//!  20. ai_throttle_minors / HOI4_ACCEL_AI_THROTTLE_MINORS=1 : Staggered hourly AI fast-skip strictly for peaceful non-major AI minors.
//!  21. quiet_log / HOI4_ACCEL_QUIET_LOG=1         : Lock-free error.log spam deduplicator v2 (SetLogger @ 0x361bec0, line-number immune).
//!  22. fast_save / HOI4_ACCEL_FAST_SAVE=1         : Removes blocking synchronous fsync() stalls during saves (0x3672b60).
//!  23. no_telemetry / HOI4_ACCEL_NO_TELEMETRY=1   : Disables Paradox SDK QueueAndSendTelemetry (0x315d2af) cleanly.

use std::cell::UnsafeCell;
use std::ffi::{CStr, c_char, c_int, c_long, c_void};
use std::fs;
use std::io::Write;
use std::sync::atomic::{AtomicBool, AtomicI32, AtomicU8, AtomicU32, AtomicU64, AtomicUsize, Ordering};

// ============================================================================
// Verified PIE offsets for Hearts of Iron IV v1.20.0.0 (BuildID 71806afa707a...)
// ============================================================================
const OFF_SCOPED_TIMER_CTOR: usize = 0x3630b40;
const OFF_SCOPED_TIMER_DTOR: usize = 0x3630cc0;
const OFF_THREAD_PROF_CTOR: usize = 0x362c8f0;
const OFF_THREAD_PROF_DTOR: usize = 0x362c9b0;
const OFF_HW_CONC_MIN4: usize = 0x36615a0;
const OFF_EXEC_TASK_NOW1: usize = 0x3666c39;
const OFF_EXEC_TASK_NOW2: usize = 0x3666cc9;

// CGameApplication::UpdateSession(void* app) called once per frame inside CInGameIdler::Idle
const OFF_SESSION_UPDATE: usize = 0x203be20;
const OFF_GAMESTATE_PTR: usize = 0x45a6a00;
const OFF_GAMESTATE_TICK_CLOCK: usize = 0x20ef870;
const OFF_GS_NUM_PLAYERS: usize = 0x104;
const OFF_GS_STATES_DATA: usize = 0x2c8;
const OFF_GS_STATES_LEN: usize = 0x2d4;
const OFF_GS_GAME_SPEED: usize = 0x4b4;
const OFF_GS_PAUSED: usize = 0x4b8;

// Mid-tick frame pacer: bool CInGameIdler::ShouldRenderMidTick(void* idler, int game_speed)
const OFF_SHOULD_RENDER_MIDTICK: usize = 0x20b07a0;
const OFF_TIMER_GET_SEC: usize = 0x2edf140;

// Error.log spam deduplicator, Fast Save (skip blocking fsync), and Telemetry blocker
const OFF_SET_LOGGER: usize = 0x361bec0;
const OFF_G_LOGGER_PTR: usize = 0x4807e10;
const OFF_FILE_FSYNC: usize = 0x3672b60;
const OFF_TELEMETRY_CALL: usize = 0x315d2af;

// CModifier::GetValue(const CSortedArray* this, uint32_t key) -> i64
const OFF_MODIFIER_GET_VAL: usize = 0x2241b00;
const OFF_G_MODIFIER_DEFS: usize = 0x453ca28;
const OFF_G_FEATURE_FN: usize = 0x45a7638;

// CPdxParallelForRange::Init(this, start, end, mode, max_threads, pool_pair)
const OFF_PARFOR_RANGE_INIT: usize = 0x366b4c0;

// Paradox CEvent / CWorkerWakePool synchronization engine & Worker PopTaskOrWait
const OFF_WAIT_AUTO1: usize = 0x3661b30;
const OFF_WAIT_AUTO2: usize = 0x3661bb0;
const OFF_WAKE_N: usize = 0x3661c50;
const OFF_WAKE_ONE: usize = 0x3661d00;
const OFF_SIGNAL: usize = 0x3661ea0;
const OFF_WAIT_MAN: usize = 0x3661ef0;
const OFF_IS_SIG: usize = 0x3661f70;
const OFF_RESET: usize = 0x3661fb0;
const OFF_TASK_WAIT: usize = 0x3662b10;
const OFF_TRY_RUN_JOB: usize = 0x36669c0;
const OFF_POP_TASK_OR_WAIT: usize = 0x3666d70;
const OFF_SCHED_YIELD_PLT: usize = 0x5140d0;

// v1.0: CPdxNewDeleteAllocator vtable & original virtual methods
const OFF_PDX_ALLOC_VT: usize = 0x4349510;
const OFF_PDX_ALLOC_SLOT: usize = 0x10;
const OFF_PDX_FREE_SLOT: usize = 0x18;
const OFF_PDX_ALLOC_FN: usize = 0x6d2c50;
const OFF_PDX_FREE_FN: usize = 0x6d2c70;

// v1.0: Civil War freeze fix (CCivilWarSetup::Execute + CInGameIdler map/coastal overlay rebuild)
const OFF_CW_SETUP_EXEC: usize = 0x1bdfa80;
const OFF_INGAME_IDLER_VT: usize = 0x4467770;
const OFF_IDLER_MAP_REBUILD_SLOT: usize = 0x240;
const OFF_IDLER_MAP_REBUILD_FN: usize = 0x21aa250;

// v1.0 -> v1.3: NDefines::NAI & NDefines::NAITheatre supply/railway/motorization globals (.bss)
// + Native Wartime Land `supply_node` Auto-Builder (FixSupplyBottlenecks @ 0x742d40)
const OFF_NDEF_PRIO_SUPPLY_BLDG: usize = 0x4594d68; // i64 CFixedPoint64 (x100,000), vanilla 1.10 = 110_000
const OFF_NDEF_PRIO_RAILWAY: usize = 0x4594d78;     // i64 CFixedPoint64 (x100,000), vanilla 4.00 = 400_000
const OFF_NDEF_SUPPLY_FREQ_HRS: usize = 0x4595788;  // i32 hours, vanilla 168 (1 week)
const OFF_NDEF_SUPPLY_SAT_THR: usize = 0x4595798;   // i64 CFixedPoint64 (x100,000), vanilla 0.85 = 85_000
const OFF_NDEF_SUPPLY_MOT_FREQ: usize = 0x45957a8;  // i32 hours, vanilla 52
const OFF_NDEF_DIV_SUPPLY_MOT: usize = 0x4595e48;   // i64 CFixedPoint64 (x100,000), vanilla 0.80 = 80_000
const OFF_NDEF_SUPPLY_MAX_DEPTH: usize = 0x4596458; // i64 CFixedPoint64 (x100,000), vanilla 5.00 = 500_000
const OFF_NDEF_SUPPLY_CRISIS_LIM: usize = 0x4596468;// i64 CFixedPoint64 (x100,000), vanilla 0.10 = 10_000
const OFF_FIX_SUPPLY_BOTTLENECKS: usize = 0x742d40;
const OFF_BASE_AI_GET_TAG: usize = 0x6f0a50;
const OFF_BASE_AI_GET_COUNTRY: usize = 0x6efd10;
const OFF_BLDG_DB_PTR: usize = 0x453d360;
const OFF_BLDG_PROD_LINE_VT: usize = 0x43a13d8;
const OFF_COUNTRY_UNITS_DATA: usize = 0x290;
const OFF_COUNTRY_UNITS_LEN: usize = 0x29c;
const OFF_COUNTRY_CONSTR_PTR: usize = 0xf40;
const OFF_PROV_ID: usize = 0xa4;
const OFF_PROV_STATIC_DATA: usize = 0xd8;
const OFF_PROV_BLDG_CONTAINER: usize = 0x1b0;
const OFF_BLDG_CONT_CTRL_TAG: usize = 0x898e60;
const OFF_BLDG_CONT_CAN_CONSTR: usize = 0x896af0;
const OFF_BLDG_LEVEL_INIT: usize = 0x8933c0;
const OFF_BLDG_LEVEL_GET: usize = 0x892ec0;
const OFF_SBLDG_LOC_CTOR: usize = 0x8963c0;
const OFF_OP_NEW_PLT: usize = 0x513950;
const OFF_ADD_CONSTR_CMD_CTOR: usize = 0x1ce1b20;
const OFF_SESSION_POST: usize = 0x20b14c0;
const ADD_CONSTR_CMD_SIZE: usize = 0x48;

// v1.3: Active-Battleplan Empty-Tile `too_weak` Fix (CUnitOrder @ 0x2569859)
// Unblocks empty-tile capture (`HasLandUnitsInProvince == 0`); CanMoveIntoProvince still runs right after!
const OFF_EMPTY_TILE_SIG: usize = 0x2569859;
const OFF_EMPTY_TILE_PATCH: usize = 0x2569860;

// v1.3: Smart Armored & Mechanized AI (CAIStrategyStore::GetStrategyValue vtable + tank/upgrade NDefines::NAI)
const OFF_STRATEGY_STORE_VT: usize = 0x434ea90;
const OFF_STRATEGY_GET_VAL_SLOT: usize = 0x28;
const OFF_STRATEGY_GET_VAL_FN: usize = 0x85b570;
const OFF_CTOKEN_FROM_STRING: usize = 0x35f9550;
const OFF_NDEF_UPG_DEFICIT_DAYS: usize = 0x4594938;   // i32  (60 -> 210)
const OFF_NDEF_UPG_PCT_FORCES: usize = 0x4593aa8;     // fp64 (20_000 -> 35_000)
const OFF_NDEF_UPG_DIV_RELUCT: usize = 0x4593a98;     // i32  (7 -> 4)
const OFF_NDEF_VAR_XP_MOD_LAND: usize = 0x4592d38;    // i32  (35 -> 20)
const OFF_NDEF_VAR_XP_LEG_LAND: usize = 0x4592d68;    // i32  (35 -> 20)
const OFF_NDEF_VAR_XP_RES_LAND: usize = 0x4592d98;    // i32  (50 -> 30)
const OFF_NDEF_XP_UPG_LAND_EQ: usize = 0x4592f58;     // fp64 (100_000 -> 200_000)
const OFF_NDEF_XP_UPD_LAND_TMPL: usize = 0x4592f48;   // fp64 (200_000 -> 250_000)
const OFF_NDEF_ASSIGN_TANKS_WAR: usize = 0x4594788;   // fp64 (600_000 -> 1_000_000)

// v1.4: Native Empty-Tile Capture (CMoveCommand @ 0x253e000 + CSession::Post)
const OFF_PROV_UNITS_DATA: usize = 0x190;
const OFF_PROV_UNITS_LEN: usize = 0x19c;
const OFF_PROV_CTRL_TAG: usize = 0x1a8;
const OFF_PROV_HAS_UNITS_OF_TYPE: usize = 0xdac220;
const OFF_TAG_GET_INDEX: usize = 0xda62c0;
const OFF_COUNTRY_IS_AT_WAR_WITH: usize = 0xd51480;
const OFF_UNIT_IS_RETREATING: usize = 0x1b55d60;
const OFF_UNIT_CAN_MOVE_INTO: usize = 0x1b6b4d0;
const OFF_MOVE_CMD_CTOR: usize = 0x253e000;
const MOVE_CMD_SIZE: usize = 0x88;

// v1.4: Anti-Frontline-Shuffling, Smart Naval Invasions & Strike Force Concentration (NDefines::NAI)
const OFF_NDEF_REASSIGN_FRONT: usize = 0x45947a8;         // fp64 (50_000 -> 20_000)
const OFF_NDEF_REASSIGN_FRONT_COMBAT: usize = 0x45947b8;  // fp64 (20_000 -> 5_000)
const OFF_NDEF_REDEPLOY_DIST_VS_ORD: usize = 0x45935c8;   // fp64 (100_000 -> 50_000)
const OFF_NDEF_INV_NO_PORT_FAC: usize = 0x4595258;        // fp64 (30_000 -> 5_000)
const OFF_NDEF_INV_MIN_PLAN_VAL: usize = 0x45952f8;       // fp64 (30_000 -> 45_000)
const OFF_NDEF_INV_MIN_ORG_FAC: usize = 0x4595308;        // fp64 (40_000 -> 65_000)
const OFF_NDEF_INV_MIN_UNITS_RDY: usize = 0x4595318;      // fp64 (25_000 -> 60_000)
const OFF_NDEF_INV_UNITS_MIN_PLAN: usize = 0x4595328;     // fp64 (75_000 -> 85_000)
const OFF_NDEF_INV_CANCEL_DISADV: usize = 0x4593f68;      // fp64 (350_000 -> 180_000)
const OFF_NDEF_INV_CANCEL_MIN_HRS: usize = 0x4593f78;     // i32  (720 -> 120)
const OFF_NDEF_INV_FAILED_AVOID: usize = 0x4595288;       // i32  (60 -> 120)
const OFF_NDEF_INV_FAILED_PORT_FAC: usize = 0x45952a8;    // fp64 (66_000 -> 35_000)
const OFF_NDEF_INV_COASTAL_PER_ORD: usize = 0x4593b38;    // i32  (24 -> 40)
const OFF_NDEF_NAVY_PREF_MAX_SIZE: usize = 0x4593b28;     // i32  (25 -> 60)
const OFF_NDEF_CAP_TF_MAX_CAP: usize = 0x4594b08;         // i32  (12 -> 16)
const OFF_NDEF_SCR_TF_MAX_SHIP: usize = 0x4594b18;        // i32  (12 -> 24)
const OFF_NDEF_STRIKE_RECALC_DAYS: usize = 0x45962d8;     // i32  (5 -> 3)
const OFF_NDEF_MAX_PATROL_STRIKE_R: usize = 0x4594cd8;    // f32  (4.0 -> 2.0)

// v1.1: Smart Airbase Expansion (CAIConstruction::GetBuildingQueuePrio + Air Coverage NDefines::NAI)
const OFF_GET_BLDG_QUEUE_PRIO: usize = 0x74f4d0;
const OFF_CONSTR_ITEM_INNER: usize = 0x78;
const OFF_CONSTR_BUILDING_PTR: usize = 0x1c0;
const OFF_BLDG_IS_AIR_BASE: usize = 0x33b;
const OFF_NDEF_WANTED_PLANES_FAC: usize = 0x45938c8; // fp64 (50_000 -> 75_000)
const OFF_NDEF_AIR_COV_STAY_PUT: usize = 0x45954f8;  // fp64 (50_000 -> 75_000)
const OFF_NDEF_OUR_COMBATS_AIR: usize = 0x4594408;   // fp64 (15_500_000 -> 45_000_000)
const OFF_NDEF_FIGHTERS_PER_PLN: usize = 0x4594478;  // fp64 (100_000 -> 200_000)
const OFF_NDEF_DEF_SUPPLY_AIR: usize = 0x4594318;    // fp64 (400_000 -> 900_000)

// v1.1 -> v1.2: Tactical Prudence, Equipment Conservation & AIFC Spearheads (Sheep AI non-cheating NDefines::NAI)
// NOTE: PLAN_ATTACK_MIN_STRENGTH_FACTOR_* are intentionally left untouched at vanilla because CUnitOrder (0x2569859)
// checks that flag even when stepping into undefended empty tiles. Anti-suicide & equipment preservation use
// ATTACK_HEAVILY_DEFENDED_LIMIT, PLAN_VALUE_TO_EXECUTE, STR/ORG_UNIT_WEAK, PLAN_FACTION_WEAK_TO_ABORT,
// CANCEL_COMBAT_* and unit-training/equipment reserve thresholds which never block empty-tile advances!
const OFF_NDEF_ATK_HEAVY_DEF_LIM: usize = 0x4593f38;   // fp64 (50_000 -> 95_000)
const OFF_NDEF_PLAN_VAL_EXECUTE: usize = 0x4593008;    // fp64 signed (-50_000 -> -10_000)
const OFF_NDEF_LOC_BAL_ADVANCE: usize = 0x4592bf8;     // fp64 (> 0 -> 0, Sheep AI: advance cleanly into open tiles)
const OFF_NDEF_STR_UNIT_WEAK: usize = 0x4593548;       // fp64 (30_000 -> 50_000)
const OFF_NDEF_ORG_UNIT_WEAK: usize = 0x4593558;       // fp64 (25_000 -> 35_000)
const OFF_NDEF_PLAN_WEAK_ABORT: usize = 0x45935a8;     // fp64 (65_000 -> 45_000)
const OFF_NDEF_CANCEL_DISADV_RAT: usize = 0x4593f48;   // fp64 (150_000 -> 115_000)
const OFF_NDEF_CANCEL_MIN_DUR_HR: usize = 0x4593f58;   // i32  (48 -> 12)
const OFF_NDEF_DESP_WEAK_STR_LIM: usize = 0x4594a18;   // fp64 (35_000 -> 65_000)
const OFF_NDEF_MIN_FIELD_STR_BLD: usize = 0x4594048;   // fp64 (70_000 -> 88_000: halt division spam during deficit)
const OFF_NDEF_START_TRAIN_EQ: usize = 0x45939f8;      // fp64 (40_000 -> 85_000)
const OFF_NDEF_STOP_TRAIN_EQ: usize = 0x4593a08;       // fp64 (25_000 -> 75_000: stop burning rifles in exercises)
const OFF_NDEF_DEPLOY_MIN_EQ_WAR: usize = 0x4592ab8;   // fp64 (85_000 -> 90_000)
const OFF_NDEF_PROD_EQ_SURPLUS: usize = 0x45929d8;     // fp64 (80_000 -> 100_000)
const OFF_NDEF_FRONT_EVAL_ACC: usize = 0x45951c8;      // fp64 (100_000 -> 150_000)
const OFF_NDEF_FRONT_EVAL_AIR: usize = 0x45951d8;      // fp64 (100_000 -> 200_000)
const OFF_NDEF_FRONT_EVAL_SUPLCK: usize = 0x45951e8;   // fp64 (100_000 -> 150_000)
const OFF_NDEF_TERRAIN_IMPORT: usize = 0x4594778;      // fp64 (1_000_000)
const OFF_NDEF_TANKS_MOUNTAINS: usize = 0x4594758;     // fp64 signed (-> -1_500_000)
const OFF_NDEF_TANKS_JUNGLE: usize = 0x4594768;        // fp64 signed (-> -2_000_000)
const OFF_NDEF_ENTRENCH_WEIGHT: usize = 0x45934d8;     // fp64 (200_000 -> 220_000: gentle, avoids freezing advances)
const OFF_NDEF_ENCIRCLE_DISC_HRS: usize = 0x4593af8;   // i32  (72 -> 24)
const OFF_NDEF_BULGE_LOW_CUTOFF: usize = 0x45940e8;    // fp64 (95_000 -> 120_000)
const OFF_NDEF_BULGE_HIGH_CUTOFF: usize = 0x45940d8;   // fp64 (150_000 -> 180_000)
const OFF_NDEF_CUTOFF_EDGE_PROX: usize = 0x45940f8;    // i32  (2 -> 1)
const OFF_NDEF_MAX_MICRO_ATKS: usize = 0x4593c78;      // i32  (3 -> 5)
const OFF_NDEF_MICRO_INACT_VAL: usize = 0x4593f88;     // fp64 (25_000 -> 15_000)
const OFF_NDEF_AIFC_UPD_DAYS: usize = 0x4595ec8;       // i32  (5 -> 4)
const OFF_NDEF_AIFC_UNIT_RATIO: usize = 0x4595f18;     // fp64 (15_000 -> 22_000)
const OFF_NDEF_AIFC_ACTIVATE_ORG: usize = 0x4595f48;   // fp64 (20_000 -> 30_000)
const OFF_NDEF_AIFC_NUDGE_DAYS: usize = 0x4595f78;     // i32  (15 -> 10)
const OFF_NDEF_AIFC_HARDNESS: usize = 0x4595fd8;       // fp64 (30_000_000 -> 80_000_000)
const OFF_NDEF_AIFC_ARMOR: usize = 0x4595fb8;          // fp64 (3_000_000 -> 8_000_000)
const OFF_NDEF_AIFC_SOFT_ATK: usize = 0x4595f98;       // fp64 (600_000 -> 1_200_000)
const OFF_NDEF_AIFC_HARD_ATK: usize = 0x4595fa8;       // fp64 (800_000 -> 1_600_000)
const OFF_NDEF_AIFC_EXPERIENCE: usize = 0x4596048;     // fp64 (30_000_000 -> 50_000_000)
const OFF_NDEF_AIFC_TGT_SUPPLY: usize = 0x4596068;     // fp64 (2_000_000 -> 3_000_000)
const OFF_NDEF_AIFC_TGT_PORT: usize = 0x4596078;       // fp64 (1_000_000 -> 1_500_000)
const OFF_NDEF_AIFC_TGT_PORT_LVL: usize = 0x4596088;   // fp64 (100_000 -> 200_000)
const OFF_NDEF_AIFC_COST_MTN: usize = 0x4596128;       // fp64 (300_000 -> 400_000)

// v1.2: Smart Peace Conference Anti-Bordergore (CPeaceWinnerAi::CalcStateActionScore + CPeaceConference::IsStateClaimedBy)
const OFF_PEACE_CALC_STATE_SCORE: usize = 0xeafef0;
const OFF_PEACE_IS_STATE_CLAIMED: usize = 0xe8af30;
const OFF_STATE_DATA_PTR: usize = 0x20;
const OFF_STATE_CORES_DATA: usize = 0xb0;
const OFF_STATE_CORES_LEN: usize = 0xbc;
const OFF_STATE_OWNER_TAG: usize = 0xc8;
const OFF_STATEDATA_NEIGHBORS_DATA: usize = 0x108;
const OFF_STATEDATA_NEIGHBORS_LEN: usize = 0x114;
const OFF_STATEDATA_IS_COASTAL: usize = 0x155;
const PEACE_ACT_TAKE_STATES: i32 = 0x30cf;
const PEACE_ACT_GIVE_STATES: i32 = 0x29e6;
const PEACE_ACT_PUPPET: i32 = 0x30d1;
const OFF_NDEF_PEACE_FOLD_TURNS: usize = 0x45957f8;    // i32  (2 -> 1)
const OFF_NDEF_PEACE_TIE_DIST: usize = 0x4595818;      // fp64 (100_000 -> 400_000)

// v1.0: Peaceful minor AI hourly throttling (CCountryAI vtable & CCountry field offsets)
const OFF_COUNTRY_AI_VT: usize = 0x434b3b8;
const OFF_COUNTRY_AI_PASS1_SLOT: usize = 0x18;
const OFF_COUNTRY_AI_PASS1_FN: usize = 0x6efbd0;
const OFF_COUNTRY_AI_PASS2_SLOT: usize = 0x20;
const OFF_COUNTRY_AI_PASS2_FN: usize = 0x6f0a40;
const OFF_COUNTRY_HAS_HUMAN: usize = 0xd3d620;
const OFF_DIPLO_IS_AT_WAR: usize = 0xa4f520;
const OFF_GS_CDATE_HOURS: usize = 0x460; // gs + 0x458 + 0x08
const OFF_COUNTRY_TAG_ID: usize = 0x08;
const OFF_COUNTRY_NUM_STATES: usize = 0x484;
const OFF_COUNTRY_DIPLO_PTR: usize = 0xf60;
const OFF_COUNTRY_IS_MAJOR: usize = 0x141a;

const HOI4_TEXT_START: usize = 0x5141c0;
const HOI4_TEXT_END: usize = 0x5141c0 + 0x316f52e;

const SIG_PUSH_RBP_4157_4156: [u8; 8] = [0x55, 0x48, 0x89, 0xe5, 0x41, 0x57, 0x41, 0x56];
const SIG_DTOR_5350_4883: [u8; 8] = [0x55, 0x48, 0x89, 0xe5, 0x53, 0x50, 0x48, 0x83];
const SIG_HW_CONC_MIN4: [u8; 8] = [0x55, 0x48, 0x89, 0xe5, 0xe8, 0xe7, 0x0e, 0xeb];
const SIG_EXEC_TASK_NOW1: [u8; 5] = [0xe8, 0x62, 0xc0, 0xea, 0xfc];
const SIG_EXEC_TASK_NOW2: [u8; 5] = [0xe8, 0xd2, 0xbf, 0xea, 0xfc];
const SIG_SIGNAL_ISSIG: [u8; 8] = [0x55, 0x48, 0x89, 0xe5, 0x41, 0x56, 0x53, 0x48];
const SIG_RESET: [u8; 8] = [0x55, 0x48, 0x89, 0xe5, 0x53, 0x50, 0x48, 0x89];
const SIG_SESSION_UPDATE: [u8; 14] = [
    0x55, 0x48, 0x89, 0xe5, 0x41, 0x57, 0x41, 0x56, 0x41, 0x55, 0x41, 0x54, 0x53, 0x50,
];
const SIG_TICK_CLOCK: [u8; 10] = [
    0x55, 0x48, 0x89, 0xe5, 0x41, 0x57, 0x41, 0x56, 0x53, 0x50,
];
const SIG_SHOULD_RENDER_MIDTICK: [u8; 17] = [
    0x55, 0x48, 0x89, 0xe5, 0x41, 0x56, 0x53, 0x48, 0x83, 0xec, 0x10, 0x41, 0x89, 0xf6, 0x48,
    0x89, 0xfb,
];
const SIG_SET_LOGGER: [u8; 10] = [0x55, 0x48, 0x89, 0xe5, 0x48, 0x89, 0xf8, 0x48, 0x8b, 0x3d];
const SIG_FILE_FSYNC: [u8; 14] = [
    0x55, 0x48, 0x89, 0xe5, 0x41, 0x56, 0x53, 0x8b, 0x1f, 0x89, 0xdf, 0xbe, 0x03, 0x00,
];
const SIG_TELEMETRY_CALL: [u8; 5] = [0xe8, 0xbc, 0x61, 0x3b, 0xfd];
const SIG_PARFOR_RANGE: [u8; 14] = [
    0x55, 0x48, 0x89, 0xe5, 0x41, 0x57, 0x41, 0x56, 0x41, 0x55, 0x41, 0x54, 0x53, 0x50,
];
const SIG_MODIFIER_GET_VAL: [u8; 17] = [
    0x55, 0x48, 0x89, 0xe5, 0x41, 0x57, 0x41, 0x56, 0x53, 0x50, 0x48, 0x8b, 0x1f, 0x48, 0x63,
    0x4f, 0x0c,
];
const SIG_POP_TASK_OR_WAIT: [u8; 20] = [
    0x55, 0x48, 0x89, 0xe5, 0x41, 0x57, 0x41, 0x56, 0x41, 0x55, 0x41, 0x54, 0x53, 0x50, 0x49,
    0x89, 0xf7, 0x49, 0x89, 0xfc,
];
const SIG_TASK_WAIT: [u8; 16] = [
    0x55, 0x48, 0x89, 0xe5, 0x53, 0x50, 0x48, 0x89, 0xfb, 0x48, 0x8b, 0x7f, 0x18, 0x48, 0x85,
    0xff,
];
const SIG_SCHED_YIELD_PLT: [u8; 16] = [
    0xff, 0x25, 0x92, 0x8f, 0xff, 0x03, 0x68, 0x0a, 0x02, 0x00, 0x00, 0xe9, 0x40, 0xdf, 0xff,
    0xff,
];
const SIG_CW_SETUP_EXEC: [u8; 20] = [
    0x55, 0x48, 0x89, 0xe5, 0x41, 0x57, 0x41, 0x56, 0x41, 0x55, 0x41, 0x54, 0x53, 0x48, 0x81,
    0xec, 0x78, 0x03, 0x00, 0x00,
];
const SIG_GET_BLDG_QUEUE_PRIO: [u8; 20] = [
    0x55, 0x48, 0x89, 0xe5, 0x41, 0x57, 0x41, 0x56, 0x41, 0x55, 0x41, 0x54, 0x53, 0x48, 0x81,
    0xec, 0xd8, 0x00, 0x00, 0x00,
];
const SIG_PEACE_CALC_STATE_SCORE: [u8; 20] = [
    0x55, 0x48, 0x89, 0xe5, 0x41, 0x57, 0x41, 0x56, 0x41, 0x55, 0x41, 0x54, 0x53, 0x48, 0x81,
    0xec, 0xf8, 0x00, 0x00, 0x00,
];
const SIG_PEACE_IS_STATE_CLAIMED: [u8; 8] = [0x55, 0x48, 0x89, 0xe5, 0x48, 0x8b, 0x8f, 0xf0];
const SIG_FIX_SUPPLY_BOTTLENECKS: [u8; 17] = [
    0x55, 0x48, 0x89, 0xe5, 0x41, 0x57, 0x41, 0x56, 0x41, 0x55, 0x41, 0x54, 0x53, 0x48, 0x83,
    0xec, 0x68,
];
const SIG_EMPTY_TILE_BRANCH: [u8; 12] = [
    0x80, 0xbd, 0x40, 0xff, 0xff, 0xff, 0x00, 0x75, 0xde, 0x49, 0x8b, 0x06,
];

// ============================================================================
// Raw Linux libc / syscalls (zero external crate dependencies)
// ============================================================================
const PROT_READ: c_int = 1;
const PROT_WRITE: c_int = 2;
const PROT_EXEC: c_int = 4;
const MAP_PRIVATE: c_int = 0x02;
const MAP_ANONYMOUS: c_int = 0x20;
const CLOCK_MONOTONIC: c_int = 1;
const CLOCK_PROCESS_CPUTIME_ID: c_int = 2;
const RTLD_NEXT: *mut c_void = (-1isize) as *mut c_void;

const SYS_GETTID: c_long = 186;
const SYS_FUTEX: c_long = 202;
const SYS_SCHED_SETAFFINITY: c_long = 203;
const FUTEX_WAIT_PRIVATE: c_long = 128;
const FUTEX_WAKE_PRIVATE: c_long = 129;

const SIGPROF: c_int = 27;
const SA_SIGINFO: c_int = 4;
const SA_RESTART: c_int = 0x1000_0000;
const ITIMER_PROF: c_int = 2;
const UCONTEXT_RIP_OFF: usize = 168;

#[repr(C)]
struct Timespec {
    tv_sec: i64,
    tv_nsec: i64,
}

#[repr(C)]
struct Timeval {
    tv_sec: i64,
    tv_usec: i64,
}

#[repr(C)]
struct Itimerval {
    it_interval: Timeval,
    it_value: Timeval,
}

#[repr(C)]
struct Sigaction {
    sa_sigaction: usize,
    sa_flags: usize,
    sa_restorer: usize,
    sa_mask: [u64; 16],
}

#[repr(C)]
struct DlPhdrInfo {
    dlpi_addr: usize,
    dlpi_name: *const c_char,
    dlpi_phdr: *const c_void,
    dlpi_phnum: u16,
}

#[repr(C)]
struct DlInfo {
    dli_fname: *const c_char,
    dli_fbase: *mut c_void,
    dli_sname: *const c_char,
    dli_saddr: *mut c_void,
}

type DlIterateCb = unsafe extern "C" fn(*mut DlPhdrInfo, usize, *mut c_void) -> c_int;
type PthreadSetnameFn = unsafe extern "C" fn(usize, *const c_char) -> c_int;
type SessionUpdateFn = unsafe extern "C" fn(*mut u8);
type TickClockFn = unsafe extern "C" fn(*mut u8, f32);
type IdlerBoolMethodFn = unsafe extern "C" fn(*const u8) -> u8;
type TimerGetSecFn = unsafe extern "C" fn(*const u8) -> f64;
type LoggerDtorFn = unsafe extern "C" fn(*mut u8);
type LoggerWriteFn = unsafe extern "C" fn(*mut u8, c_int, *const StdString, c_int, *const StdString);

#[repr(C)]
struct StdString {
    ptr: *const u8,
    len: usize,
}
type ParforRangeInitFn =
    unsafe extern "C" fn(*mut u8, c_int, c_int, c_int, c_int, *const u8);
type ModifierGetValFn = unsafe extern "C" fn(*const u8, u32) -> i64;
type FeatureMaskFn = unsafe extern "C" fn() -> u32;
type PoolWakeFn = unsafe extern "C" fn(*const u8, c_int);
type EventWaitFn = unsafe extern "C" fn(*mut u8);
type TryRunJobFn = unsafe extern "C" fn(*mut u8) -> u8;
type PdxAllocFn = unsafe extern "C" fn(*mut u8, usize, usize) -> *mut u8;
type PdxFreeFn = unsafe extern "C" fn(*mut u8, *mut u8);
type CwSetupExecFn = unsafe extern "C" fn(*mut u8, *mut u8, u8);
type IdlerMapRebuildFn = unsafe extern "C" fn(*mut u8) -> u8;
type GetBldgQueuePrioFn = unsafe extern "C" fn(*mut u8, *const u8) -> i64;
type PeaceCalcStateScoreFn = unsafe extern "C" fn(
    *const u8,
    *const i32,
    *const i32,
    *const i32,
    i32,
    i32,
    usize,
    usize,
    usize,
) -> i64;
type PeaceIsStateClaimedFn = unsafe extern "C" fn(*const u8, i32, *const i32) -> u8;
type CountryAiPassFn = unsafe extern "C" fn(*mut u8, usize, usize);
type OwnerAccessCountryFn = unsafe extern "C" fn(*mut u8) -> *mut u8;
type CountryHasHumanFn = unsafe extern "C" fn(*mut u8) -> u8;
type DiploIsAtWarFn = unsafe extern "C" fn(*const u8, u8) -> u8;
type FixSupplyBottlenecksFn = unsafe extern "C" fn(*mut u8);
type BaseAiResolveFn = unsafe extern "C" fn(*mut u8) -> *mut u8;
type BaseAiGetTagFn = unsafe extern "C" fn(*mut u8) -> *const u32;
type GsGetProvinceFn = unsafe extern "C" fn(*const u8, i32) -> *const u8;
type BldgContCtrlTagFn = unsafe extern "C" fn(*const u8) -> *const u32;
type BldgContCanConstrFn = unsafe extern "C" fn(*const u8, *const u8, u8) -> u8;
type BldgLevelInitFn = unsafe extern "C" fn(*mut [u64; 4], *const u8, *const u8);
type BldgLevelGetFn = unsafe extern "C" fn(*mut [u64; 4]) -> i32;
type SBldgLocCtorFn = unsafe extern "C" fn(*mut [u64; 4], *const u8, *const u8);
type OpNewFn = unsafe extern "C" fn(usize) -> *mut u8;
type AddConstrCmdCtorFn = unsafe extern "C" fn(*mut u8, u8, *const u32, *const [u64; 4], i32);
type SessionPostFn = unsafe extern "C" fn(*mut u8, *const u32);
type GetStrategyValueFn = unsafe extern "C" fn(*const u8, i32, i32, i32) -> i32;

#[repr(C)]
struct LinStrView {
    len: u64,
    ptr: *const u8,
}
type CTokenFromStringFn = unsafe extern "C" fn(*const LinStrView) -> i32;
type ProvHasUnitsOfTypeFn = unsafe extern "C" fn(*const u8, i32) -> u8;
type TagGetIndexFn = unsafe extern "C" fn(*const i32) -> i32;
type CountryIsAtWarWithFn = unsafe extern "C" fn(*const u8, *const [i32; 2]) -> u8;
type UnitAsLandFn = unsafe extern "C" fn(*const u8) -> *const u8;
type UnitBoolFn = unsafe extern "C" fn(*const u8) -> u8;
type UnitGetOrgRatioFn = unsafe extern "C" fn(*const u8) -> i64;
type UnitCanMoveIntoFn = unsafe extern "C" fn(*const u8, *const u8) -> u8;
type MoveCmdCtorFn =
    unsafe extern "C" fn(*mut u8, *const u8, i32, u8, i32, u8, u8, u8, u8, u8);
type CmdCanExecFn = unsafe extern "C" fn(*mut u8, u8) -> u8;
type CmdDtorFn = unsafe extern "C" fn(*mut u8);

unsafe extern "C" {
    fn mmap(
        addr: *mut c_void,
        length: usize,
        prot: c_int,
        flags: c_int,
        fd: c_int,
        offset: isize,
    ) -> *mut c_void;
    fn mprotect(addr: *mut c_void, len: usize, prot: c_int) -> c_int;
    fn clock_gettime(clk_id: c_int, tp: *mut Timespec) -> c_int;
    fn dl_iterate_phdr(cb: DlIterateCb, data: *mut c_void) -> c_int;
    fn dladdr(addr: *const c_void, info: *mut DlInfo) -> c_int;
    fn dlsym(handle: *mut c_void, symbol: *const c_char) -> *mut c_void;
    fn syscall(num: c_long, ...) -> c_long;
    fn sched_yield() -> c_int;
    fn sigaction(signum: c_int, act: *const Sigaction, oldact: *mut Sigaction) -> c_int;
    fn setitimer(which: c_int, new_value: *const Itimerval, old_value: *mut Itimerval) -> c_int;
    fn pthread_self() -> usize;
    fn _exit(status: c_int) -> !;
}

#[inline(always)]
fn now_ns() -> u64 {
    let mut ts = Timespec {
        tv_sec: 0,
        tv_nsec: 0,
    };
    unsafe {
        clock_gettime(CLOCK_MONOTONIC, &mut ts);
    }
    (ts.tv_sec as u64)
        .wrapping_mul(1_000_000_000)
        .wrapping_add(ts.tv_nsec as u64)
}

#[inline(always)]
fn now_cpu_ns() -> u64 {
    let mut ts = Timespec {
        tv_sec: 0,
        tv_nsec: 0,
    };
    unsafe {
        clock_gettime(CLOCK_PROCESS_CPUTIME_ID, &mut ts);
    }
    (ts.tv_sec as u64)
        .wrapping_mul(1_000_000_000)
        .wrapping_add(ts.tv_nsec as u64)
}

#[inline(always)]
fn rdtsc() -> u64 {
    unsafe { core::arch::x86_64::_rdtsc() }
}

#[inline(always)]
fn current_tid() -> i32 {
    unsafe { syscall(SYS_GETTID) as i32 }
}

fn pin_current_thread_to_cpu(cpu_id: usize) -> bool {
    if cpu_id >= 128 {
        return false;
    }
    let mut mask: [u64; 2] = [0, 0];
    mask[cpu_id / 64] = 1u64 << (cpu_id % 64);
    unsafe {
        syscall(
            SYS_SCHED_SETAFFINITY,
            0usize,
            std::mem::size_of_val(&mask),
            mask.as_ptr(),
        ) == 0
    }
}

#[inline(always)]
fn futex_wait(word: &AtomicU32, expected: u32) {
    unsafe {
        syscall(
            SYS_FUTEX,
            word as *const AtomicU32 as *const u32,
            FUTEX_WAIT_PRIVATE,
            expected as c_long,
            0usize,
            0usize,
            0usize,
        );
    }
}

#[inline(always)]
fn futex_wake_all(word: &AtomicU32) {
    unsafe {
        syscall(
            SYS_FUTEX,
            word as *const AtomicU32 as *const u32,
            FUTEX_WAKE_PRIVATE,
            i32::MAX as c_long,
            0usize,
            0usize,
            0usize,
        );
    }
}

// ============================================================================
// Global State, Zone Profiler, and SIGPROF RIP Sampler
// ============================================================================
const TABLE_CAP: usize = 2048;
const SAMPLE_BUCKETS: usize = 8192;

struct ZoneSlot {
    name_ptr: AtomicUsize,
    calls: AtomicU64,
    total_ns: AtomicU64,
    max_ns: AtomicU64,
    main_ns: AtomicU64,
    worker_ns: AtomicU64,
}

impl ZoneSlot {
    const fn new() -> Self {
        Self {
            name_ptr: AtomicUsize::new(0),
            calls: AtomicU64::new(0),
            total_ns: AtomicU64::new(0),
            max_ns: AtomicU64::new(0),
            main_ns: AtomicU64::new(0),
            worker_ns: AtomicU64::new(0),
        }
    }
}

struct SampleSlot {
    key: AtomicUsize,
    main_hits: AtomicU64,
    worker_hits: AtomicU64,
}

impl SampleSlot {
    const fn new() -> Self {
        Self {
            key: AtomicUsize::new(0),
            main_hits: AtomicU64::new(0),
            worker_hits: AtomicU64::new(0),
        }
    }
}

static ZONES: [ZoneSlot; TABLE_CAP] = [const { ZoneSlot::new() }; TABLE_CAP];
static SAMPLES: [SampleSlot; SAMPLE_BUCKETS] = [const { SampleSlot::new() }; SAMPLE_BUCKETS];

static PIE_BASE: AtomicUsize = AtomicUsize::new(0);
static MAIN_TID: AtomicI32 = AtomicI32::new(0);
static MAIN_PTHREAD: AtomicUsize = AtomicUsize::new(0);
static FAST_TASK_WAIT_ENABLED: AtomicBool = AtomicBool::new(false);
static FAST_YIELD_ENABLED: AtomicBool = AtomicBool::new(false);
static YIELD_ELIDED: AtomicU64 = AtomicU64::new(0);
static HOURLY_PTR: AtomicUsize = AtomicUsize::new(0);
static TOTAL_HOURS: AtomicU64 = AtomicU64::new(0);
static BENCH_WARMUP_HOURS: AtomicU64 = AtomicU64::new(24);
static BENCH_MAX_HOURS: AtomicU64 = AtomicU64::new(0);
static BENCH_START_NS: AtomicU64 = AtomicU64::new(0);
static BENCH_START_CPU_NS: AtomicU64 = AtomicU64::new(0);
static BENCH_RECORDING: AtomicBool = AtomicBool::new(false);
static BENCH_FINISHED: AtomicBool = AtomicBool::new(false);
static COLLECT_ZONES: AtomicBool = AtomicBool::new(false);
static PROFILE_STARTED_SIM: AtomicBool = AtomicBool::new(false);

static OVERRIDE_THREADS: AtomicI32 = AtomicI32::new(0);
static AFFINITY_ENABLED: AtomicBool = AtomicBool::new(false);
static WORKER_PIN_COUNTER: AtomicUsize = AtomicUsize::new(0);
static BG_PIN_COUNTER: AtomicUsize = AtomicUsize::new(0);
static TOPO_WORKER_CPUS: [AtomicU32; 64] = [const { AtomicU32::new(0) }; 64];
static TOPO_WORKER_LEN: AtomicUsize = AtomicUsize::new(0);
static TOPO_BG_CPUS: [AtomicU32; 64] = [const { AtomicU32::new(0) }; 64];
static TOPO_BG_LEN: AtomicUsize = AtomicUsize::new(0);
static ACTIVE_FOR_HOI4: AtomicBool = AtomicBool::new(false);
static SAMPLE_ENABLED: AtomicBool = AtomicBool::new(false);
static TOTAL_SAMPLES_MAIN: AtomicU64 = AtomicU64::new(0);
static TOTAL_SAMPLES_WORK: AtomicU64 = AtomicU64::new(0);

// Fast-event & Lock-free Worker Scheduler tuning
static FAST_EVENT_ENABLED: AtomicBool = AtomicBool::new(false);
static FAST_SCHED_ENABLED: AtomicBool = AtomicBool::new(false);
static SCHED_WATCHER_POLLS: AtomicU64 = AtomicU64::new(0);
static NO_JOB_CLOCK_ENABLED: AtomicBool = AtomicBool::new(false);
static SPIN_ITERS: AtomicU32 = AtomicU32::new(0);
static FAST_WAKE_SPUN: AtomicU64 = AtomicU64::new(0);
static FAST_WAKE_FUTEX: AtomicU64 = AtomicU64::new(0);

// Breakthrough 1: Render-Aware Multi-Tick Speed-5 Bursting (UpdateSession hook)
static ORIG_SESSION_UPDATE: AtomicUsize = AtomicUsize::new(0);
static TARGET_FRAME_NS: AtomicU64 = AtomicU64::new(0);
static MAX_BURST_TICKS: AtomicU64 = AtomicU64::new(16);
static FRAMES_RENDERED: AtomicU64 = AtomicU64::new(0);
static ACTIVE_SIM_FRAMES: AtomicU64 = AtomicU64::new(0);
static ACTIVE_SIM_NS: AtomicU64 = AtomicU64::new(0);
static BURST_EXTRA_TICKS: AtomicU64 = AtomicU64::new(0);
static PREV_SESSION_END_NS: AtomicU64 = AtomicU64::new(0);
static LAST_FRAME_START_NS: AtomicU64 = AtomicU64::new(0);
static EMA_RENDER_NS: AtomicU64 = AtomicU64::new(8_000_000);
static EMA_TICK_NS: AtomicU64 = AtomicU64::new(14_000_000);
static WORST_FRAME_NS: AtomicU64 = AtomicU64::new(0);
static IN_BURST: AtomicBool = AtomicBool::new(false);

// Breakthrough 4: Anti-Freeze Mid-Tick Frame Pacer (ShouldRenderMidTick @ 0x20b07a0)
static SMOOTH_FPS: AtomicU64 = AtomicU64::new(0);
static MID_TICK_CHECKS: AtomicU64 = AtomicU64::new(0);
static MID_TICK_FAST_SKIP: AtomicU64 = AtomicU64::new(0);
static MID_TICK_RENDERS: AtomicU64 = AtomicU64::new(0);
static LAST_MID_CHECK_TSC: AtomicU64 = AtomicU64::new(0);
static TSC_PER_MS: AtomicU64 = AtomicU64::new(0);
static TSC_CAL_NS: AtomicU64 = AtomicU64::new(0);
static TSC_CAL_TSC: AtomicU64 = AtomicU64::new(0);

// Breakthrough 5: Lock-Free error.log Spam Deduplicator v2 (SetLogger @ 0x361bec0)
const LOG_DEDUP_CAP: usize = 8192;
const LOG_VT_CAP: usize = 16;
struct LogDedupSlot {
    hash: AtomicU64,
    count: AtomicU32,
}
impl LogDedupSlot {
    const fn new() -> Self {
        Self {
            hash: AtomicU64::new(0),
            count: AtomicU32::new(0),
        }
    }
}
static LOG_DEDUP: [LogDedupSlot; LOG_DEDUP_CAP] = [const { LogDedupSlot::new() }; LOG_DEDUP_CAP];
static LOG_VT_KEYS: [AtomicUsize; LOG_VT_CAP] = [const { AtomicUsize::new(0) }; LOG_VT_CAP];
static LOG_VT_ORIG: [AtomicUsize; LOG_VT_CAP] = [const { AtomicUsize::new(0) }; LOG_VT_CAP];
static QUIET_LOG_ENABLED: AtomicBool = AtomicBool::new(false);
static LOG_ALLOWED: AtomicU64 = AtomicU64::new(0);
static LOG_SUPPRESSED: AtomicU64 = AtomicU64::new(0);

// Breakthrough 6 & 7: Fast Save (skip blocking fsync) & Telemetry Blocker
static FAST_SAVE_ENABLED: AtomicBool = AtomicBool::new(false);
static FSYNC_SKIPPED: AtomicU64 = AtomicU64::new(0);
static NO_TELEMETRY_ENABLED: AtomicBool = AtomicBool::new(false);

// Breakthrough 2: Pure Rust Zen-3 Branchless CModifier::GetValue
static ORIG_MODIFIER_GET_VAL: AtomicUsize = AtomicUsize::new(0);
static VERIFY_MOD_REMAINING: AtomicU64 = AtomicU64::new(0);
static VERIFY_MOD_MISMATCHES: AtomicU64 = AtomicU64::new(0);
static FAST_MOD_ENABLED: AtomicBool = AtomicBool::new(false);

// Breakthrough 3: Adaptive Country-Loop Grain (chunk=1 only for 17..=220 country-scale loops)
static ORIG_PARFOR_RANGE_INIT: AtomicUsize = AtomicUsize::new(0);
static COUNTRY_GRAIN_ENABLED: AtomicBool = AtomicBool::new(false);
static PARFOR_REBALANCED: AtomicU64 = AtomicU64::new(0);
static PARFOR_TOTAL: AtomicU64 = AtomicU64::new(0);

// v1.0 Module 11: Lock-Free Slab + TLS Magazine Allocator (CPdxNewDeleteAllocator)
static FAST_ALLOC_ENABLED: AtomicBool = AtomicBool::new(false);
static ORIG_PDX_ALLOC: AtomicUsize = AtomicUsize::new(0);
static ORIG_PDX_FREE: AtomicUsize = AtomicUsize::new(0);
static SLAB_ARENA_BASE: AtomicUsize = AtomicUsize::new(0);
static SLAB_BUMP_OFF: AtomicUsize = AtomicUsize::new(0);
static SLAB_GLOBAL_FREE: [AtomicU64; 4] = [const { AtomicU64::new(0) }; 4];
static SLAB_ALLOC_HITS: AtomicU64 = AtomicU64::new(0);
static SLAB_FREE_HITS: AtomicU64 = AtomicU64::new(0);
static SLAB_FALLBACKS: AtomicU64 = AtomicU64::new(0);

// v1.0 Module 12: Civil War Freeze Fix (CCivilWarSetup::Execute + CInGameIdler map rebuild coalescer)
static CIVIL_WAR_FIX_ENABLED: AtomicBool = AtomicBool::new(false);
static ORIG_CW_SETUP_EXEC: AtomicUsize = AtomicUsize::new(0);
static ORIG_IDLER_MAP_REBUILD: AtomicUsize = AtomicUsize::new(0);
static IN_CIVIL_WAR_DEPTH: AtomicU32 = AtomicU32::new(0);
static DEFERRED_CW_IDLER: AtomicUsize = AtomicUsize::new(0);
static CW_REBUILDS_COALESCED: AtomicU64 = AtomicU64::new(0);

// v1.0 -> v1.3 Module 13: Smart AI Supply, Railway & Native Wartime `supply_node` Auto-Builder
static AI_SMART_SUPPLY_ENABLED: AtomicBool = AtomicBool::new(false);
static AI_SMART_SUPPLY_APPLIED: AtomicBool = AtomicBool::new(false);
static ORIG_FIX_SUPPLY_BOTTLENECKS: AtomicUsize = AtomicUsize::new(0);
static SUPPLY_NODE_BLDG_PTR: AtomicUsize = AtomicUsize::new(0);
static NAVAL_BASE_BLDG_PTR: AtomicUsize = AtomicUsize::new(0);
static BLDG_DB_RESOLVED: AtomicBool = AtomicBool::new(false);
static SUPPLY_HUB_LAST_HR: [AtomicU32; 512] = [const { AtomicU32::new(0) }; 512];
static AI_SUPPLY_HUBS_QUEUED: AtomicU64 = AtomicU64::new(0);

// v1.4 Module 14: Native Empty-Tile Capture (CMoveCommand + CSession::Post, pocket-safe)
static AI_CAPTURE_EMPTY_ENABLED: AtomicBool = AtomicBool::new(false);
static AI_EMPTY_TILES_CAPTURED: AtomicU64 = AtomicU64::new(0);

// v1.4 Module 17: Smart Naval Invasions & Strike Force Concentration
static AI_SMART_NAVAL_ENABLED: AtomicBool = AtomicBool::new(false);
static AI_SMART_NAVAL_APPLIED: AtomicBool = AtomicBool::new(false);

// v1.1 Module 14: Smart Airbase Expansion (CAIConstruction::GetBuildingQueuePrio + Air Coverage NDefines)
static AI_SMART_AIRBASE_ENABLED: AtomicBool = AtomicBool::new(false);
static AI_SMART_AIRBASE_DEFINES_APPLIED: AtomicBool = AtomicBool::new(false);
static ORIG_GET_BLDG_QUEUE_PRIO: AtomicUsize = AtomicUsize::new(0);
static AIRBASE_PRIO_BOOSTS: AtomicU64 = AtomicU64::new(0);

// v1.1 -> v1.3 Module 15: Tactical Prudence, Equipment Conservation, AIFC Spearheads & Empty-Tile Total Fix
static AI_TACTICAL_PRUDENCE_ENABLED: AtomicBool = AtomicBool::new(false);
static AI_TACTICAL_PRUDENCE_APPLIED: AtomicBool = AtomicBool::new(false);
static EMPTY_TILE_FIX_APPLIED: AtomicBool = AtomicBool::new(false);

// v1.3 Module 16: Smart Armored & Mechanized AI (CAIStrategyStore::GetStrategyValue + Tank NDefines)
static AI_SMART_ARMOR_ENABLED: AtomicBool = AtomicBool::new(false);
static AI_SMART_ARMOR_DEFINES_APPLIED: AtomicBool = AtomicBool::new(false);
static ORIG_GET_STRATEGY_VALUE: AtomicUsize = AtomicUsize::new(0);
static TOK_ARMOR: AtomicI32 = AtomicI32::new(-1);
static TOK_MECHANIZED: AtomicI32 = AtomicI32::new(-1);
static TOK_INFANTRY: AtomicI32 = AtomicI32::new(-1);
static AI_ARMOR_STRATEGY_BOOSTS: AtomicU64 = AtomicU64::new(0);

// v1.2 Module 17: Smart Peace Conference Anti-Bordergore (CPeaceWinnerAi::CalcStateActionScore)
static AI_SMART_PEACE_ENABLED: AtomicBool = AtomicBool::new(false);
static AI_SMART_PEACE_DEFINES_APPLIED: AtomicBool = AtomicBool::new(false);
static ORIG_PEACE_CALC_STATE_SCORE: AtomicUsize = AtomicUsize::new(0);
static PEACE_BORDERGORE_ADJUSTS: AtomicU64 = AtomicU64::new(0);

// v1.0 Module 14: Dormant Peaceful Minor AI Hourly Throttling
static AI_THROTTLE_MINORS_ENABLED: AtomicBool = AtomicBool::new(false);
static ORIG_COUNTRY_AI_PASS1: AtomicUsize = AtomicUsize::new(0);
static ORIG_COUNTRY_AI_PASS2: AtomicUsize = AtomicUsize::new(0);
static MINOR_SKIP_BITMAP: [AtomicU64; 8] = [const { AtomicU64::new(0) }; 8];
static MINOR_AI_SKIPPED: AtomicU64 = AtomicU64::new(0);
static MINOR_AI_EXECUTED: AtomicU64 = AtomicU64::new(0);

#[inline(always)]
fn hash_usize(p: usize) -> usize {
    let mut x = p.wrapping_mul(0x9e3779b97f4a7c15usize);
    x ^= x >> 30;
    x = x.wrapping_mul(0xbf58476d1ce4e5b9usize);
    x ^ (x >> 27)
}

// ============================================================================
// Breakthrough 1: Render-Aware Multi-Tick Speed-5 Bursting (UpdateSession hook)
// ============================================================================
// Called once per rendered frame from CInGameIdler::Idle (0x2199fee).
// Tracks actual render duration (time spent outside UpdateSession) and EMA tick duration
// so Speed-5 bursting stays strictly within the requested FPS budget (e.g. >= 24-30 FPS
// in `smooth` mode, or maximum throughput in `balanced` / `turbo`).
unsafe extern "C" fn hook_session_update(app: *mut u8) {
    let orig_ptr = ORIG_SESSION_UPDATE.load(Ordering::Relaxed);
    let orig: SessionUpdateFn = unsafe { std::mem::transmute(orig_ptr) };

    let frame_start = now_ns();
    let pie_for_state = PIE_BASE.load(Ordering::Relaxed);
    let mut is_unpaused_sim = false;
    if pie_for_state != 0 {
        let gs = unsafe { *((pie_for_state + OFF_GAMESTATE_PTR) as *const *mut u8) };
        if !gs.is_null() {
            is_unpaused_sim = unsafe { *gs.add(OFF_GS_PAUSED) } == 0;
            if AI_SMART_SUPPLY_ENABLED.load(Ordering::Relaxed)
                && !AI_SMART_SUPPLY_APPLIED.load(Ordering::Relaxed)
            {
                unsafe { apply_ai_smart_supply_once(pie_for_state) };
            }
            if AI_SMART_AIRBASE_ENABLED.load(Ordering::Relaxed)
                && !AI_SMART_AIRBASE_DEFINES_APPLIED.load(Ordering::Relaxed)
            {
                unsafe { apply_ai_smart_airbase_once(pie_for_state) };
            }
            if AI_TACTICAL_PRUDENCE_ENABLED.load(Ordering::Relaxed)
                && !AI_TACTICAL_PRUDENCE_APPLIED.load(Ordering::Relaxed)
            {
                unsafe { apply_ai_tactical_prudence_once(pie_for_state) };
            }
            if AI_SMART_ARMOR_ENABLED.load(Ordering::Relaxed)
                && !AI_SMART_ARMOR_DEFINES_APPLIED.load(Ordering::Relaxed)
            {
                unsafe { apply_ai_smart_armor_once(pie_for_state) };
            }
            if AI_SMART_NAVAL_ENABLED.load(Ordering::Relaxed)
                && !AI_SMART_NAVAL_APPLIED.load(Ordering::Relaxed)
            {
                unsafe { apply_ai_smart_naval_once(pie_for_state) };
            }
            if AI_SMART_PEACE_ENABLED.load(Ordering::Relaxed)
                && !AI_SMART_PEACE_DEFINES_APPLIED.load(Ordering::Relaxed)
            {
                unsafe { apply_ai_smart_peace_once(pie_for_state) };
            }
        }
    }
    // Measure real frame-to-frame interval and real render duration (outside UpdateSession)
    let prev_start = LAST_FRAME_START_NS.swap(frame_start, Ordering::Relaxed);
    if prev_start > 0 && frame_start > prev_start {
        let fdt = frame_start - prev_start;
        if is_unpaused_sim && fdt < 2_000_000_000 {
            ACTIVE_SIM_FRAMES.fetch_add(1, Ordering::Relaxed);
            ACTIVE_SIM_NS.fetch_add(fdt, Ordering::Relaxed);
        }
        if fdt < 2_000_000_000 {
            let mut cur_w = WORST_FRAME_NS.load(Ordering::Relaxed);
            while fdt > cur_w {
                match WORST_FRAME_NS.compare_exchange_weak(
                    cur_w,
                    fdt,
                    Ordering::Relaxed,
                    Ordering::Relaxed,
                ) {
                    Ok(_) => break,
                    Err(v) => cur_w = v,
                }
            }
        }
    }
    let prev_end = PREV_SESSION_END_NS.load(Ordering::Relaxed);
    if prev_end > 0 && frame_start > prev_end {
        let rdt = (frame_start - prev_end).min(100_000_000);
        let old_r = EMA_RENDER_NS.load(Ordering::Relaxed);
        let new_r = (old_r * 3 + rdt) / 4;
        EMA_RENDER_NS.store(new_r, Ordering::Relaxed);
    }

    let budget_ns = TARGET_FRAME_NS.load(Ordering::Relaxed);
    if budget_ns == 0 || IN_BURST.swap(true, Ordering::AcqRel) {
        unsafe { orig(app) };
        FRAMES_RENDERED.fetch_add(1, Ordering::Relaxed);
        PREV_SESSION_END_NS.store(now_ns(), Ordering::Relaxed);
        return;
    }

    let h0 = TOTAL_HOURS.load(Ordering::Relaxed);
    let t0 = now_ns();
    unsafe { orig(app) };
    let t1 = now_ns();
    let mut h_prev = TOTAL_HOURS.load(Ordering::Relaxed);

    if h_prev > h0 {
        let tick_dt = t1.saturating_sub(t0).min(500_000_000);
        let old_t = EMA_TICK_NS.load(Ordering::Relaxed);
        let mut avg_tick = (old_t * 3 + tick_dt) / 4;
        EMA_TICK_NS.store(avg_tick, Ordering::Relaxed);

        // Reserve actual measured render time so total frame time (sim + render) honors target FPS!
        let render_est = EMA_RENDER_NS.load(Ordering::Relaxed);
        let sim_budget_ns = budget_ns.saturating_sub(render_est);

        let pie = PIE_BASE.load(Ordering::Relaxed);
        let tick_clock: TickClockFn =
            unsafe { std::mem::transmute(pie + OFF_GAMESTATE_TICK_CLOCK) };
        let max_burst = MAX_BURST_TICKS.load(Ordering::Relaxed);
        let mut extra = 0u64;

        while extra < max_burst {
            let elapsed = now_ns().saturating_sub(frame_start);
            // Predict whether another tick will keep us inside the target frame budget
            let predicted_end = if budget_ns <= 55_000_000 {
                // Smooth FPS mode (>= 18 FPS): be strict so we don't overshoot below target FPS
                elapsed.saturating_add((avg_tick * 3) / 4)
            } else {
                // Throughput mode (balanced / turbo): fill the budget aggressively
                elapsed
            };
            if predicted_end >= sim_budget_ns {
                break;
            }
            let gs = unsafe { *((pie + OFF_GAMESTATE_PTR) as *const *mut u8) };
            if gs.is_null() {
                break;
            }
            let speed = unsafe { *(gs.add(OFF_GS_GAME_SPEED) as *const i32) };
            let paused = unsafe { *gs.add(OFF_GS_PAUSED) };
            let num_players = unsafe { *(gs.add(OFF_GS_NUM_PLAYERS) as *const i32) };
            // Strictly Speed 5 (index 4), unpaused (0), and singleplayer (num_players <= 1)!
            if speed != 4 || paused != 0 || num_players > 1 {
                break;
            }
            let bt0 = now_ns();
            unsafe { tick_clock(gs, 0.0f32) };
            unsafe { orig(app) };
            let bt1 = now_ns();
            let h_now = TOTAL_HOURS.load(Ordering::Relaxed);
            if h_now <= h_prev {
                break;
            }
            avg_tick = (avg_tick * 3 + bt1.saturating_sub(bt0).min(500_000_000)) / 4;
            EMA_TICK_NS.store(avg_tick, Ordering::Relaxed);
            h_prev = h_now;
            extra += 1;
        }
        if extra > 0 {
            BURST_EXTRA_TICKS.fetch_add(extra, Ordering::Relaxed);
        }
    }

    FRAMES_RENDERED.fetch_add(1, Ordering::Relaxed);
    PREV_SESSION_END_NS.store(now_ns(), Ordering::Relaxed);
    IN_BURST.store(false, Ordering::Release);
}

// ============================================================================
// Breakthrough 4: Anti-Freeze Mid-Tick Frame Pacer (ShouldRenderMidTick @ 0x20b07a0)
// ============================================================================
// Paradox calls this at 63 checkpoints inside gamestate.hourly and gamestate.daily.
// Stock formula hardcodes 20 FPS on Speed 5 and multiplies frame interval by 1.5x-4.0x
// whenever render > 12-25ms, causing 5-13 FPS stutters in late game / heavy mods.
// Our Rust replacement removes the harsh degradation penalty and holds steady smooth_fps
// (and 60 FPS while panning the camera or on Speed 1-2).
unsafe extern "C" fn hook_should_render_mid_tick(idler: *const u8, game_speed: c_int) -> u8 {
    if IN_CIVIL_WAR_DEPTH.load(Ordering::Relaxed) > 0 {
        return 0;
    }
    let tsc = rdtsc();
    let tsc_per_ms = TSC_PER_MS.load(Ordering::Relaxed);
    if tsc_per_ms > 0 {
        let next_tsc = LAST_MID_CHECK_TSC.load(Ordering::Relaxed);
        if tsc < next_tsc {
            MID_TICK_FAST_SKIP.fetch_add(1, Ordering::Relaxed);
            return 0;
        }
        // Default cooldown: 2 ms between full checks (8x finer than 60 FPS, 16x finer than 30 FPS)
        LAST_MID_CHECK_TSC.store(tsc.wrapping_add(tsc_per_ms * 2), Ordering::Relaxed);
    } else {
        // One-time lazy calibration of invariant TSC frequency over >= 10 ms (zero startup delay)
        let cal_ns = TSC_CAL_NS.load(Ordering::Relaxed);
        let now = now_ns();
        if cal_ns == 0 {
            TSC_CAL_TSC.store(tsc, Ordering::Relaxed);
            TSC_CAL_NS.store(now, Ordering::Relaxed);
        } else {
            let dt_ns = now.saturating_sub(cal_ns);
            if dt_ns >= 10_000_000 {
                let dt_tsc = tsc.saturating_sub(TSC_CAL_TSC.load(Ordering::Relaxed));
                let per_ms = ((dt_tsc as u128 * 1_000_000) / dt_ns as u128) as u64;
                if per_ms >= 500_000 && per_ms <= 20_000_000 {
                    TSC_PER_MS.store(per_ms, Ordering::Relaxed);
                    LAST_MID_CHECK_TSC.store(tsc.wrapping_add(per_ms * 2), Ordering::Relaxed);
                }
            }
        }
    }

    MID_TICK_CHECKS.fetch_add(1, Ordering::Relaxed);
    if idler.is_null() {
        return 0;
    }
    let vtable = unsafe { *(idler as *const *const usize) };
    let is_async_ready: IdlerBoolMethodFn = unsafe { std::mem::transmute(*vtable.add(0x3c0 / 8)) };
    if unsafe { is_async_ready(idler) } == 0 {
        return 0;
    }

    let is_cam_moving: IdlerBoolMethodFn = unsafe { std::mem::transmute(*vtable.add(0x310 / 8)) };
    let cam_moving = unsafe { is_cam_moving(idler) } != 0;

    let smooth = SMOOTH_FPS.load(Ordering::Relaxed).max(1) as f64;
    let target_dt = if cam_moving || game_speed <= 1 {
        1.0 / 60.0
    } else {
        1.0 / smooth
    };

    // Gentle self-protection only if GPU render itself is very slow (never 2x-4x Paradox cliff!)
    let last_render_dur = unsafe { *(idler.add(0x1c) as *const f32) } as f64;
    let threshold = target_dt.max(last_render_dur * 1.15) - 0.001;

    let timer_obj = unsafe { *(idler.add(0x50) as *const *const u8) };
    if timer_obj.is_null() {
        return 0;
    }
    let pie = PIE_BASE.load(Ordering::Relaxed);
    let get_sec: TimerGetSecFn = unsafe { std::mem::transmute(pie + OFF_TIMER_GET_SEC) };
    let now_sec = unsafe { get_sec(timer_obj) };
    let last_render_sec = unsafe { *(idler.add(0x2c) as *const f32) } as f64;
    let elapsed = now_sec - last_render_sec;

    if elapsed > threshold {
        MID_TICK_RENDERS.fetch_add(1, Ordering::Relaxed);
        if tsc_per_ms > 0 {
            // After a mid-tick render, another frame cannot be needed for at least 8 ms
            LAST_MID_CHECK_TSC.store(rdtsc().wrapping_add(tsc_per_ms * 8), Ordering::Relaxed);
        }
        1
    } else {
        0
    }
}

// ============================================================================
// Breakthrough 5: Lock-Free error.log Spam Deduplicator v2 (SetLogger @ 0x361bec0)
// ============================================================================
// Collapses repeated script/localisation/mod errors (such as Paradox's 100 MB+
// RU_loc.txt Invalid Scope spam across 15,000+ script line numbers) while preserving
// the first 8 entries per unique error pattern so real startup/crash diagnostics remain intact.
fn fnv1a_log_sig(level: c_int, file: *const StdString, msg: *const StdString) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    let mix = |h: &mut u64, b: u8| {
        *h ^= b as u64;
        *h = h.wrapping_mul(0x100000001b3);
    };
    for b in level.to_le_bytes() {
        mix(&mut h, b);
    }
    // NOTE: Do NOT hash `line` — HoI4 passes script file line numbers (e.g. 1..87000+ in RU_loc.txt)
    // in `line`, which would otherwise explode unique patterns and overflow the dedup table!
    if !file.is_null() {
        let f = unsafe { &*file };
        if !f.ptr.is_null() {
            let n = f.len.min(128);
            let slice = unsafe { std::slice::from_raw_parts(f.ptr, n) };
            let mut kept = 0usize;
            for &b in slice {
                if b.is_ascii_digit() {
                    continue;
                }
                mix(&mut h, b);
                kept += 1;
                if kept >= 64 {
                    break;
                }
            }
        }
    }
    if !msg.is_null() {
        let m = unsafe { &*msg };
        if !m.ptr.is_null() {
            let n = m.len.min(160);
            let slice = unsafe { std::slice::from_raw_parts(m.ptr, n) };
            let mut kept = 0usize;
            for &b in slice {
                // Skip ASCII digits so RU_loc.txt:87095 / :87103 / :87111 collapse cleanly!
                if b.is_ascii_digit() {
                    continue;
                }
                mix(&mut h, b);
                kept += 1;
                if kept >= 96 {
                    break;
                }
            }
        }
    }
    if h == 0 { 1 } else { h }
}

fn should_allow_log(sig: u64, level: c_int) -> bool {
    const MAX_PER_PATTERN: u32 = 8;
    let mut idx = (sig as usize) & (LOG_DEDUP_CAP - 1);
    for _ in 0..32 {
        let slot = &LOG_DEDUP[idx];
        let existing = slot.hash.load(Ordering::Relaxed);
        if existing == sig
            || (existing == 0
                && slot
                    .hash
                    .compare_exchange(0, sig, Ordering::Relaxed, Ordering::Relaxed)
                    .is_ok())
        {
            let prev = slot.count.fetch_add(1, Ordering::Relaxed);
            return prev < MAX_PER_PATTERN;
        }
        idx = (idx + 1) & (LOG_DEDUP_CAP - 1);
    }
    // If table probe window saturates, still allow low-level critical/info (< 3) but block warning/error floods
    level < 3
}

unsafe extern "C" fn hook_logger_write(
    this: *mut u8,
    level: c_int,
    file: *const StdString,
    line: c_int,
    msg: *const StdString,
) {
    if this.is_null() {
        return;
    }
    let vtable = unsafe { *(this as *const usize) };
    let mut orig_fn: usize = 0;
    for i in 0..LOG_VT_CAP {
        if LOG_VT_KEYS[i].load(Ordering::Acquire) == vtable {
            orig_fn = LOG_VT_ORIG[i].load(Ordering::Relaxed);
            break;
        }
    }
    if orig_fn == 0 {
        return;
    }

    // Suppress transient script/state warning spam while bulk Civil War transfers are mid-flight
    if IN_CIVIL_WAR_DEPTH.load(Ordering::Relaxed) > 0 && level >= 2 {
        LOG_SUPPRESSED.fetch_add(1, Ordering::Relaxed);
        return;
    }

    // Deduplicate repetitive error/warning spam (line-number immune across script files)
    let sig = fnv1a_log_sig(level, file, msg);
    if !should_allow_log(sig, level) {
        LOG_SUPPRESSED.fetch_add(1, Ordering::Relaxed);
        return;
    }

    LOG_ALLOWED.fetch_add(1, Ordering::Relaxed);
    let real_write: LoggerWriteFn = unsafe { std::mem::transmute(orig_fn) };
    unsafe { real_write(this, level, file, line, msg) };
}

unsafe fn patch_rodata_ptr(addr: usize, new_val: usize) -> bool {
    let page_size = 4096usize;
    let start_page = addr & !(page_size - 1);
    let end_page = (addr + std::mem::size_of::<usize>() + page_size - 1) & !(page_size - 1);
    let len = end_page - start_page;
    unsafe {
        if mprotect(start_page as *mut c_void, len, PROT_READ | PROT_WRITE) != 0 {
            return false;
        }
        std::ptr::write_unaligned(addr as *mut usize, new_val);
        let _ = mprotect(start_page as *mut c_void, len, PROT_READ);
    }
    true
}

unsafe fn wrap_logger_vtable(logger: *mut u8) {
    if logger.is_null() {
        return;
    }
    let vtable = unsafe { *(logger as *const usize) };
    if vtable == 0 {
        return;
    }
    let write_slot_addr = vtable + 0x10;
    let orig_write = unsafe { *(write_slot_addr as *const usize) };
    let hook_addr = hook_logger_write as *const () as usize;
    if orig_write == 0 || orig_write == hook_addr {
        return;
    }
    for i in 0..LOG_VT_CAP {
        let cur = LOG_VT_KEYS[i].load(Ordering::Acquire);
        if cur == vtable {
            return;
        }
        if cur == 0
            && LOG_VT_KEYS[i]
                .compare_exchange(0, vtable, Ordering::AcqRel, Ordering::Acquire)
                .is_ok()
        {
            LOG_VT_ORIG[i].store(orig_write, Ordering::Release);
            let _ = unsafe { patch_rodata_ptr(write_slot_addr, hook_addr) };
            return;
        }
    }
}

// Pure Rust replacement for Paradox SetLogger(CLogger* new_logger) @ 0x361bec0
unsafe extern "C" fn hook_set_logger(new_logger: *mut u8) {
    if !new_logger.is_null() {
        unsafe { wrap_logger_vtable(new_logger) };
    }
    let pie = PIE_BASE.load(Ordering::Relaxed);
    let slot = (pie + OFF_G_LOGGER_PTR) as *mut *mut u8;
    let old = unsafe { *slot };
    unsafe { *slot = new_logger };
    if !old.is_null() && old != new_logger {
        let vt = unsafe { *(old as *const *const usize) };
        if !vt.is_null() {
            let dtor_addr = unsafe { *vt.add(1) };
            if dtor_addr != 0 {
                let dtor: LoggerDtorFn = unsafe { std::mem::transmute(dtor_addr) };
                unsafe { dtor(old) };
            }
        }
    }
}

// ============================================================================
// Breakthrough 6: Fast Save (Skip blocking synchronous fsync @ 0x3672b60)
// ============================================================================
unsafe extern "C" fn hook_fast_fsync(_file_obj: *mut u8) -> u8 {
    FSYNC_SKIPPED.fetch_add(1, Ordering::Relaxed);
    1
}

// ============================================================================
// Breakthrough 2: Pure Rust Branchless Zen-3 CModifier::GetValue (0x2241b00)
// ============================================================================
#[repr(C)]
struct ModifierEntry {
    key: u32,
    _pad: u32,
    value: i64,
}

#[inline(always)]
unsafe fn fast_modifier_get_value_impl(arr: *const u8, key: u32) -> i64 {
    let len = unsafe { *(arr.add(0x0c) as *const i32) };
    if len <= 0 {
        return 0;
    }
    let data = unsafe { *(arr as *const *const ModifierEntry) };
    let mut n = len as usize;
    let mut base = data;

    // Pure branchless binary search: compiles to `cmp + cmov` on Zen 3 -> ZERO branch mispredictions!
    while n > 1 {
        let half = n >> 1;
        let mid = unsafe { base.add(half) };
        let mid_key = unsafe { (*mid).key };
        // Guaranteed CMOV on x86_64 Zen 3 -> zero branch mispredictions
        base = std::hint::select_unpredictable(mid_key <= key, mid, base);
        n -= half;
    }

    let found = unsafe { &*base };
    if found.key != key {
        return 0;
    }

    // Exact match to 0x2241b81..0x2241bdd
    let pie = PIE_BASE.load(Ordering::Relaxed);
    let defs = unsafe { *((pie + OFF_G_MODIFIER_DEFS) as *const *const u8) };
    let def_off = (key as i32 as isize) * 0x78;
    let def = unsafe { defs.offset(def_off) };
    let mask = unsafe { *(def.add(0x68) as *const u32) };
    if mask != 0 {
        let fn_ptr = unsafe { *((pie + OFF_G_FEATURE_FN) as *const usize) };
        if fn_ptr == 0 {
            return 0;
        }
        let feature_fn: FeatureMaskFn = unsafe { std::mem::transmute(fn_ptr) };
        if (unsafe { feature_fn() } & mask) == 0 {
            return 0;
        }
    }

    let val = found.value;
    let flags = unsafe { *def.add(0x60) };
    if (flags & 0x10) != 0 {
        val.clamp(0, 100_000)
    } else {
        val
    }
}

unsafe extern "C" fn hook_modifier_get_value_pure(arr: *const u8, key: u32) -> i64 {
    unsafe { fast_modifier_get_value_impl(arr, key) }
}

unsafe extern "C" fn hook_modifier_get_value_verify(arr: *const u8, key: u32) -> i64 {
    let res = unsafe { fast_modifier_get_value_impl(arr, key) };
    let rem = VERIFY_MOD_REMAINING.load(Ordering::Relaxed);
    if rem > 0 {
        if VERIFY_MOD_REMAINING
            .try_update(Ordering::Relaxed, Ordering::Relaxed, |v| {
                if v > 0 { Some(v - 1) } else { None }
            })
            .is_ok()
        {
            let orig_ptr = ORIG_MODIFIER_GET_VAL.load(Ordering::Relaxed);
            let orig: ModifierGetValFn = unsafe { std::mem::transmute(orig_ptr) };
            let expected = unsafe { orig(arr, key) };
            if res != expected {
                let m = VERIFY_MOD_MISMATCHES.fetch_add(1, Ordering::Relaxed) + 1;
                if m <= 5 {
                    eprintln!(
                        "[hoi4-rust-accel] MODIFIER MISMATCH key={} rust={} orig={}",
                        key, res, expected
                    );
                }
            }
        }
    }
    res
}

// ============================================================================
// Breakthrough 3: Adaptive Country-Loop Grain (eliminate major-nation stragglers)
// ============================================================================
unsafe extern "C" fn hook_parfor_range_init(
    this: *mut u8,
    start: c_int,
    end: c_int,
    mode: c_int,
    max_threads: c_int,
    pool_pair: *const u8,
) {
    let orig_ptr = ORIG_PARFOR_RANGE_INIT.load(Ordering::Relaxed);
    let orig: ParforRangeInitFn = unsafe { std::mem::transmute(orig_ptr) };
    unsafe { orig(this, start, end, mode, max_threads, pool_pair) };

    PARFOR_TOTAL.fetch_add(1, Ordering::Relaxed);
    if COUNTRY_GRAIN_ENABLED.load(Ordering::Relaxed) && !this.is_null() {
        let n = end - start;
        // Strictly country-scale loops (17..=220 items, e.g. 75-200 nations where majors
        // like GER/SOV/USA take 10x longer than minors). Leave large province/unit loops alone!
        if (17..=220).contains(&n) {
            let chunk_ptr = unsafe { this.add(0x18) as *mut i32 };
            let orig_chunk = unsafe { *chunk_ptr };
            if orig_chunk > 1 {
                unsafe { *chunk_ptr = 1 };
                PARFOR_REBALANCED.fetch_add(1, Ordering::Relaxed);
            }
        }
    }
}

// ============================================================================
// v1.0 Module 11: Lock-Free ABA-Safe Slab + TLS Magazine Allocator
// Wraps CPdxNewDeleteAllocator (vtable @ 0x4349510) for <= 512B, align <= 16B
// ============================================================================
const SLAB_ARENA_SIZE: usize = 128 * 1024 * 1024; // 128 MB demand-paged virtual arena
const SLAB_PAGE_SIZE: usize = 65536;              // 64 KB aligned slab pages
const SLAB_MAGIC: u32 = 0x484f_4934;              // "HOI4"
const SLAB_OFF_MASK: u64 = 0x0fff_ffff;           // 28-bit byte offset in arena (< 256 MB)
const SLAB_GEN_INC: u64 = 1u64 << 28;             // 36-bit ABA generation counter
const SLAB_CLASS_BYTES: [usize; 4] = [64, 128, 256, 512];
const TLS_MAG_CAP: usize = 64;

#[repr(C, align(64))]
struct SlabPageHdr {
    magic: u32,
    class_idx: u32,
    _pad: [u64; 7],
}

struct TlsSlabMag {
    counts: [usize; 4],
    slots: [[*mut u8; TLS_MAG_CAP]; 4],
}

impl TlsSlabMag {
    const fn new() -> Self {
        Self {
            counts: [0; 4],
            slots: [[std::ptr::null_mut(); TLS_MAG_CAP]; 4],
        }
    }
}

impl Drop for TlsSlabMag {
    fn drop(&mut self) {
        let arena = SLAB_ARENA_BASE.load(Ordering::Acquire);
        if arena == 0 {
            return;
        }
        for cls in 0..4 {
            let n = self.counts[cls];
            for i in 0..n {
                let p = self.slots[cls][i];
                if !p.is_null() {
                    unsafe { slab_push_global(arena, cls, p) };
                }
            }
            self.counts[cls] = 0;
        }
    }
}

thread_local! {
    static TLS_SLAB: UnsafeCell<TlsSlabMag> = const { UnsafeCell::new(TlsSlabMag::new()) };
}

fn init_slab_arena() -> bool {
    let raw = unsafe {
        mmap(
            std::ptr::null_mut(),
            SLAB_ARENA_SIZE + SLAB_PAGE_SIZE,
            PROT_READ | PROT_WRITE,
            MAP_PRIVATE | MAP_ANONYMOUS,
            -1,
            0,
        )
    };
    if raw.is_null() || raw as isize == -1 {
        return false;
    }
    let aligned = ((raw as usize) + (SLAB_PAGE_SIZE - 1)) & !(SLAB_PAGE_SIZE - 1);
    SLAB_BUMP_OFF.store(0, Ordering::Release);
    SLAB_ARENA_BASE.store(aligned, Ordering::Release);
    true
}

#[inline(always)]
unsafe fn slab_push_global(arena: usize, cls: usize, ptr: *mut u8) {
    let off = (ptr as usize).wrapping_sub(arena) as u32;
    let list = &SLAB_GLOBAL_FREE[cls];
    let mut head = list.load(Ordering::Acquire);
    loop {
        unsafe { *(ptr as *mut u32) = (head & SLAB_OFF_MASK) as u32 };
        let next_head = ((head & !SLAB_OFF_MASK).wrapping_add(SLAB_GEN_INC)) | (off as u64);
        match list.compare_exchange_weak(head, next_head, Ordering::AcqRel, Ordering::Acquire) {
            Ok(_) => break,
            Err(cur) => head = cur,
        }
    }
}

#[inline(always)]
unsafe fn slab_pop_global(arena: usize, cls: usize) -> *mut u8 {
    let list = &SLAB_GLOBAL_FREE[cls];
    let mut head = list.load(Ordering::Acquire);
    loop {
        let off = (head & SLAB_OFF_MASK) as usize;
        if off == 0 {
            return std::ptr::null_mut();
        }
        let ptr = (arena + off) as *mut u8;
        let next_off = unsafe { *(ptr as *const u32) } as u64;
        let next_head = ((head & !SLAB_OFF_MASK).wrapping_add(SLAB_GEN_INC)) | next_off;
        match list.compare_exchange_weak(head, next_head, Ordering::AcqRel, Ordering::Acquire) {
            Ok(_) => return ptr,
            Err(cur) => head = cur,
        }
    }
}

unsafe fn slab_carve_page(arena: usize, cls: usize, mag: Option<&mut TlsSlabMag>) -> *mut u8 {
    let mut cur = SLAB_BUMP_OFF.load(Ordering::Relaxed);
    let page_off = loop {
        if cur + SLAB_PAGE_SIZE > SLAB_ARENA_SIZE {
            return std::ptr::null_mut();
        }
        match SLAB_BUMP_OFF.compare_exchange_weak(
            cur,
            cur + SLAB_PAGE_SIZE,
            Ordering::AcqRel,
            Ordering::Relaxed,
        ) {
            Ok(v) => break v,
            Err(v) => cur = v,
        }
    };

    let page_ptr = (arena + page_off) as *mut u8;
    let hdr = page_ptr as *mut SlabPageHdr;
    unsafe {
        (*hdr).magic = SLAB_MAGIC;
        (*hdr).class_idx = cls as u32;
    }

    let bsize = SLAB_CLASS_BYTES[cls];
    let first_block = unsafe { page_ptr.add(64) };
    let mut off = 64 + bsize;
    if let Some(m) = mag {
        while off + bsize <= SLAB_PAGE_SIZE && m.counts[cls] < (TLS_MAG_CAP / 2) {
            let idx = m.counts[cls];
            m.slots[cls][idx] = unsafe { page_ptr.add(off) };
            m.counts[cls] = idx + 1;
            off += bsize;
        }
    }
    while off + bsize <= SLAB_PAGE_SIZE {
        unsafe { slab_push_global(arena, cls, page_ptr.add(off)) };
        off += bsize;
    }
    first_block
}

unsafe extern "C" fn hook_pdx_alloc(self_ptr: *mut u8, size: usize, align: usize) -> *mut u8 {
    if align <= 16 && size <= 512 {
        let arena = SLAB_ARENA_BASE.load(Ordering::Relaxed);
        if arena != 0 {
            let cls = if size <= 64 {
                0
            } else if size <= 128 {
                1
            } else if size <= 256 {
                2
            } else {
                3
            };
            let block = TLS_SLAB
                .try_with(|cell| {
                    let m = unsafe { &mut *cell.get() };
                    let n = m.counts[cls];
                    if n > 0 {
                        m.counts[cls] = n - 1;
                        return m.slots[cls][n - 1];
                    }
                    // Refill up to 16 blocks from global lock-free Treiber stack
                    let first = unsafe { slab_pop_global(arena, cls) };
                    if !first.is_null() {
                        for _ in 0..15 {
                            let extra = unsafe { slab_pop_global(arena, cls) };
                            if extra.is_null() {
                                break;
                            }
                            let idx = m.counts[cls];
                            m.slots[cls][idx] = extra;
                            m.counts[cls] = idx + 1;
                        }
                        return first;
                    }
                    unsafe { slab_carve_page(arena, cls, Some(m)) }
                })
                .unwrap_or_else(|_| {
                    let p = unsafe { slab_pop_global(arena, cls) };
                    if !p.is_null() {
                        p
                    } else {
                        unsafe { slab_carve_page(arena, cls, None) }
                    }
                });

            if !block.is_null() {
                SLAB_ALLOC_HITS.fetch_add(1, Ordering::Relaxed);
                return block;
            }
        }
    }

    SLAB_FALLBACKS.fetch_add(1, Ordering::Relaxed);
    let orig: PdxAllocFn = unsafe { std::mem::transmute(ORIG_PDX_ALLOC.load(Ordering::Relaxed)) };
    unsafe { orig(self_ptr, size, align) }
}

unsafe extern "C" fn hook_pdx_free(self_ptr: *mut u8, ptr: *mut u8) {
    if ptr.is_null() {
        return;
    }
    let arena = SLAB_ARENA_BASE.load(Ordering::Relaxed);
    let rel = (ptr as usize).wrapping_sub(arena);
    if arena != 0 && rel < SLAB_ARENA_SIZE {
        let hdr = ((ptr as usize) & !(SLAB_PAGE_SIZE - 1)) as *const SlabPageHdr;
        let magic = unsafe { (*hdr).magic };
        let cls = unsafe { (*hdr).class_idx } as usize;
        if magic == SLAB_MAGIC && cls < 4 {
            SLAB_FREE_HITS.fetch_add(1, Ordering::Relaxed);
            let handled = TLS_SLAB.try_with(|cell| {
                let m = unsafe { &mut *cell.get() };
                let mut n = m.counts[cls];
                if n >= TLS_MAG_CAP {
                    let keep = TLS_MAG_CAP / 2;
                    for i in keep..n {
                        unsafe { slab_push_global(arena, cls, m.slots[cls][i]) };
                    }
                    n = keep;
                }
                m.slots[cls][n] = ptr;
                m.counts[cls] = n + 1;
            });
            if handled.is_err() {
                unsafe { slab_push_global(arena, cls, ptr) };
            }
            return;
        }
    }

    let orig: PdxFreeFn = unsafe { std::mem::transmute(ORIG_PDX_FREE.load(Ordering::Relaxed)) };
    unsafe { orig(self_ptr, ptr) };
}

// ============================================================================
// v1.0 Module 12: Civil War Freeze Fix
// Coalesces 100+ redundant CInGameIdler map/coastal/supply overlay rebuilds
// during CCivilWarSetup::Execute (0x1bdfa80) into a single clean rebuild on exit
// ============================================================================
unsafe extern "C" fn hook_idler_map_rebuild(self_ptr: *mut u8) -> u8 {
    if IN_CIVIL_WAR_DEPTH.load(Ordering::Acquire) > 0 {
        if !self_ptr.is_null() {
            DEFERRED_CW_IDLER.store(self_ptr as usize, Ordering::Release);
        }
        CW_REBUILDS_COALESCED.fetch_add(1, Ordering::Relaxed);
        return 1;
    }
    let orig: IdlerMapRebuildFn =
        unsafe { std::mem::transmute(ORIG_IDLER_MAP_REBUILD.load(Ordering::Relaxed)) };
    unsafe { orig(self_ptr) }
}

unsafe extern "C" fn hook_cw_setup_exec(self_ptr: *mut u8, out_tag: *mut u8, flag: u8) {
    IN_CIVIL_WAR_DEPTH.fetch_add(1, Ordering::AcqRel);
    let orig: CwSetupExecFn =
        unsafe { std::mem::transmute(ORIG_CW_SETUP_EXEC.load(Ordering::Relaxed)) };
    unsafe { orig(self_ptr, out_tag, flag) };

    if IN_CIVIL_WAR_DEPTH.fetch_sub(1, Ordering::AcqRel) == 1 {
        let deferred_idler = DEFERRED_CW_IDLER.swap(0, Ordering::AcqRel);
        let orig_rebuild_ptr = ORIG_IDLER_MAP_REBUILD.load(Ordering::Relaxed);
        if deferred_idler != 0 && orig_rebuild_ptr != 0 {
            let rebuild: IdlerMapRebuildFn = unsafe { std::mem::transmute(orig_rebuild_ptr) };
            unsafe { rebuild(deferred_idler as *mut u8) };
        }
    }
}

// ============================================================================
// v1.0 -> v1.3 Module 13: Smart AI Supply, Railways & Native Wartime Land
// `supply_node` Auto-Builder (FixSupplyBottlenecks @ 0x742d40)
// Applied lazily once g_pGameState != null (after 00_defines.lua & mods load)
// ============================================================================
unsafe fn resolve_supply_buildings_once(pie: usize) {
    if BLDG_DB_RESOLVED
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Relaxed)
        .is_err()
    {
        return;
    }
    let db = unsafe { *((pie + OFF_BLDG_DB_PTR) as *const *const u8) };
    if db.is_null() {
        BLDG_DB_RESOLVED.store(false, Ordering::Release);
        return;
    }
    let names = unsafe { *(db.add(0x88) as *const *const u8) };
    let names_len = unsafe { *(db.add(0x94) as *const i32) };
    let bldgs = unsafe { *(db.add(0x70) as *const *const *const u8) };
    let bldgs_len = unsafe { *(db.add(0x7c) as *const i32) };
    if names.is_null() || bldgs.is_null() || names_len <= 0 || bldgs_len <= 1 {
        BLDG_DB_RESOLVED.store(false, Ordering::Release);
        return;
    }
    let cap = (names_len as usize).min(128);
    let bcap = bldgs_len as usize;
    for i in 0..cap {
        if i + 1 >= bcap {
            break;
        }
        let entry = unsafe { names.add(i * 0x28) };
        let sptr = unsafe { *(entry as *const *const u8) };
        let slen = unsafe { *(entry.add(0x08) as *const usize) };
        if sptr.is_null() || slen == 0 || slen > 64 {
            continue;
        }
        let bytes = unsafe { std::slice::from_raw_parts(sptr, slen) };
        let bptr = unsafe { *bldgs.add(i + 1) } as usize;
        if bptr == 0 {
            continue;
        }
        if bytes == b"supply_node" {
            SUPPLY_NODE_BLDG_PTR.store(bptr, Ordering::Release);
        } else if bytes == b"naval_base" {
            NAVAL_BASE_BLDG_PTR.store(bptr, Ordering::Release);
        }
    }
}

#[inline(always)]
unsafe fn get_bldg_level_in_prov(pie: usize, bldg: *const u8, container: *const u8) -> i32 {
    if bldg.is_null() || container.is_null() {
        return 0;
    }
    let init_fn: BldgLevelInitFn = unsafe { std::mem::transmute(pie + OFF_BLDG_LEVEL_INIT) };
    let get_fn: BldgLevelGetFn = unsafe { std::mem::transmute(pie + OFF_BLDG_LEVEL_GET) };
    let mut handle = [0u64; 4];
    unsafe { init_fn(&mut handle, bldg, container) };
    unsafe { get_fn(&mut handle) }
}

unsafe fn prov_borders_non_own_land(
    pie: usize,
    gs: *const u8,
    get_prov: GsGetProvinceFn,
    prov: *const u8,
    tag_id: u32,
) -> bool {
    let sp = unsafe { *(prov.add(OFF_PROV_STATIC_DATA) as *const *const u8) };
    if sp.is_null() {
        return false;
    }
    let adj_ptr = unsafe { *(sp.add(0x70) as *const *const u8) };
    let adj_len = unsafe { *(sp.add(0x7c) as *const i32) };
    if adj_ptr.is_null() || adj_len <= 0 {
        return false;
    }
    let get_ctrl: BldgContCtrlTagFn = unsafe { std::mem::transmute(pie + OFF_BLDG_CONT_CTRL_TAG) };
    let cap = (adj_len as usize).min(32);
    for j in 0..cap {
        let adj = unsafe { adj_ptr.add(j * 0x30) };
        if unsafe { *adj.add(0x10) } == 4 {
            continue; // impassable
        }
        let nb_id = unsafe { *(adj.add(0x08) as *const i32) };
        if nb_id <= 0 {
            continue;
        }
        let nb_p = unsafe { get_prov(gs, nb_id) };
        if nb_p.is_null() {
            continue;
        }
        let nb_sp = unsafe { *(nb_p.add(OFF_PROV_STATIC_DATA) as *const *const u8) };
        if nb_sp.is_null() || (unsafe { *nb_sp.add(0xd2) } & 1) == 0 {
            continue;
        }
        let nb_ctrl = unsafe { get_ctrl(nb_p.add(OFF_PROV_BLDG_CONTAINER)) };
        if !nb_ctrl.is_null() {
            let cid = unsafe { *nb_ctrl };
            if cid != 0 && cid != tag_id {
                return true;
            }
        }
    }
    false
}

unsafe fn has_hub_or_port_within_2_hops(
    pie: usize,
    gs: *const u8,
    get_prov: GsGetProvinceFn,
    start_prov: *const u8,
    supply_node: *const u8,
    naval_base: *const u8,
) -> bool {
    let mut visited = [0i32; 64];
    let mut vis_len = 0usize;
    let mut queue_p = [std::ptr::null::<u8>(); 64];
    let mut queue_d = [0u8; 64];
    let mut q_head = 0usize;
    let mut q_tail = 0usize;

    let start_id = unsafe { *(start_prov.add(OFF_PROV_ID) as *const i32) };
    visited[vis_len] = start_id;
    vis_len += 1;
    queue_p[q_tail] = start_prov;
    queue_d[q_tail] = 0;
    q_tail += 1;

    while q_head < q_tail {
        let cur_p = queue_p[q_head];
        let cur_d = queue_d[q_head];
        q_head += 1;

        let cont = unsafe { cur_p.add(OFF_PROV_BLDG_CONTAINER) };
        if unsafe { get_bldg_level_in_prov(pie, supply_node, cont) } > 0 {
            return true;
        }
        if !naval_base.is_null() && unsafe { get_bldg_level_in_prov(pie, naval_base, cont) } > 0 {
            return true;
        }
        if cur_d >= 2 {
            continue;
        }
        let sp = unsafe { *(cur_p.add(OFF_PROV_STATIC_DATA) as *const *const u8) };
        if sp.is_null() {
            continue;
        }
        let adj_ptr = unsafe { *(sp.add(0x70) as *const *const u8) };
        let adj_len = unsafe { *(sp.add(0x7c) as *const i32) };
        if adj_ptr.is_null() || adj_len <= 0 {
            continue;
        }
        let cap = (adj_len as usize).min(32);
        for j in 0..cap {
            let adj = unsafe { adj_ptr.add(j * 0x30) };
            if unsafe { *adj.add(0x10) } == 4 {
                continue;
            }
            let nb_id = unsafe { *(adj.add(0x08) as *const i32) };
            if nb_id <= 0 {
                continue;
            }
            let mut seen = false;
            for k in 0..vis_len {
                if visited[k] == nb_id {
                    seen = true;
                    break;
                }
            }
            if seen || vis_len >= 64 || q_tail >= 64 {
                continue;
            }
            let nb_p = unsafe { get_prov(gs, nb_id) };
            if nb_p.is_null() {
                continue;
            }
            let nb_sp = unsafe { *(nb_p.add(OFF_PROV_STATIC_DATA) as *const *const u8) };
            if nb_sp.is_null() || (unsafe { *nb_sp.add(0xd2) } & 1) == 0 {
                continue;
            }
            visited[vis_len] = nb_id;
            vis_len += 1;
            queue_p[q_tail] = nb_p;
            queue_d[q_tail] = cur_d + 1;
            q_tail += 1;
        }
    }
    false
}

unsafe fn try_queue_wartime_supply_hub(ai_constr: *mut u8) {
    let pie = PIE_BASE.load(Ordering::Relaxed);
    if pie == 0 || ai_constr.is_null() {
        return;
    }
    let supply_node = SUPPLY_NODE_BLDG_PTR.load(Ordering::Acquire) as *const u8;
    if supply_node.is_null() {
        return;
    }
    let naval_base = NAVAL_BASE_BLDG_PTR.load(Ordering::Acquire) as *const u8;

    let gs = unsafe { *((pie + OFF_GAMESTATE_PTR) as *const *const u8) };
    if gs.is_null() {
        return;
    }
    let gs_vt = unsafe { *(gs as *const *const usize) };
    if gs_vt.is_null() {
        return;
    }
    let get_prov_addr = unsafe { *gs_vt.add(0x50 / 8) };
    if get_prov_addr == 0 {
        return;
    }
    let get_prov: GsGetProvinceFn = unsafe { std::mem::transmute(get_prov_addr) };

    // Resolve base_ai, tag_ptr, and country from CAIConstruction*
    let obj = unsafe { *(ai_constr.add(0x08) as *const *mut u8) };
    if obj.is_null() {
        return;
    }
    let obj_vt = unsafe { *(obj as *const *const usize) };
    if obj_vt.is_null() {
        return;
    }
    let get_base_ai_addr = unsafe { *obj_vt.add(0x10 / 8) };
    if get_base_ai_addr == 0 {
        return;
    }
    let get_base_ai: BaseAiResolveFn = unsafe { std::mem::transmute(get_base_ai_addr) };
    let base_ai = unsafe { get_base_ai(obj) };
    if base_ai.is_null() {
        return;
    }
    let get_tag: BaseAiGetTagFn = unsafe { std::mem::transmute(pie + OFF_BASE_AI_GET_TAG) };
    let get_country: BaseAiResolveFn = unsafe { std::mem::transmute(pie + OFF_BASE_AI_GET_COUNTRY) };
    let tag_ptr = unsafe { get_tag(base_ai) };
    let country = unsafe { get_country(base_ai) };
    if tag_ptr.is_null() || country.is_null() {
        return;
    }
    let tag_id = unsafe { *tag_ptr };
    if tag_id == 0 {
        return;
    }

    // Never interfere with human-controlled nations; only build emergency hubs while at war
    let has_human: CountryHasHumanFn = unsafe { std::mem::transmute(pie + OFF_COUNTRY_HAS_HUMAN) };
    if unsafe { has_human(country) } != 0 {
        return;
    }
    let diplo = unsafe { *(country.add(OFF_COUNTRY_DIPLO_PTR) as *const *const u8) };
    if diplo.is_null() {
        return;
    }
    let is_at_war: DiploIsAtWarFn = unsafe { std::mem::transmute(pie + OFF_DIPLO_IS_AT_WAR) };
    if unsafe { is_at_war(diplo, 0) } == 0 {
        return;
    }

    // Require sufficient industrial base (>= 12 total factories) and ZERO active supply_node in queue
    let constr = unsafe { *(country.add(OFF_COUNTRY_CONSTR_PTR) as *const *const u8) };
    if constr.is_null() {
        return;
    }
    let total_fac = unsafe { *(constr.add(0x2d0) as *const i64) } / 100_000;
    if total_fac < 12 {
        return;
    }

    // Per-tag 21-day (504h) cooldown prevents double-posting before queue updates
    let raw_date = unsafe { *(gs.add(OFF_GS_CDATE_HOURS) as *const i32) };
    let cur_hours = raw_date.wrapping_sub(0x029c55c0) as u32;
    let cd_slot = &SUPPLY_HUB_LAST_HR[(tag_id as usize) & 511];
    let last_hr = cd_slot.load(Ordering::Relaxed);
    if last_hr != 0 && cur_hours.wrapping_sub(last_hr) < 504 {
        return;
    }

    // Strictly max 1 active supply_node in country's construction queue at a time
    let q_ptr = unsafe { *(constr.add(0x70) as *const *const *const u8) };
    let q_len = unsafe { *(constr.add(0x7c) as *const i32) };
    let prod_line_vt = pie + OFF_BLDG_PROD_LINE_VT;
    if !q_ptr.is_null() && q_len > 0 {
        let cap = (q_len as usize).min(256);
        for i in 0..cap {
            let item = unsafe { *q_ptr.add(i) };
            if item.is_null() {
                continue;
            }
            if unsafe { *(item as *const usize) } == prod_line_vt {
                let inner = unsafe { *(item.add(OFF_CONSTR_ITEM_INNER) as *const *const u8) };
                if !inner.is_null() {
                    let bldg = unsafe { *(inner.add(OFF_CONSTR_BUILDING_PTR) as *const *const u8) };
                    if bldg == supply_node {
                        return;
                    }
                }
            }
        }
    }

    // Scan country's own land divisions for genuinely starving frontline clusters (< 65% supply)
    let units_ptr = unsafe { *(country.add(OFF_COUNTRY_UNITS_DATA) as *const *const *const u8) };
    let units_len = unsafe { *(country.add(OFF_COUNTRY_UNITS_LEN) as *const i32) };
    if units_ptr.is_null() || units_len < 1 {
        return;
    }
    let get_ctrl: BldgContCtrlTagFn = unsafe { std::mem::transmute(pie + OFF_BLDG_CONT_CTRL_TAG) };
    let mut starv_ids = [0i32; 64];
    let mut starv_ptrs = [std::ptr::null::<u8>(); 64];
    let mut starv_cnts = [0u16; 64];
    let mut starv_min_ratio = [100_000i64; 64];
    let mut starv_len = 0usize;

    let ucap = (units_len as usize).min(2048);
    for i in 0..ucap {
        let unit = unsafe { *units_ptr.add(i) };
        if unit.is_null() {
            continue;
        }
        let supply_needed = unsafe { *(unit.add(0x38) as *const i64) };
        let supply_ratio = unsafe { *(unit.add(0x40) as *const i64) };
        if supply_needed <= 0 || supply_ratio >= 65_000 {
            continue;
        }
        let prov = unsafe { *(unit.add(0x1f0) as *const *const u8) };
        if prov.is_null() {
            continue;
        }
        let prov_id = unsafe { *(prov.add(OFF_PROV_ID) as *const i32) };
        if prov_id <= 0 {
            continue;
        }
        let sp = unsafe { *(prov.add(OFF_PROV_STATIC_DATA) as *const *const u8) };
        if sp.is_null() || (unsafe { *sp.add(0xd2) } & 1) == 0 {
            continue;
        }
        let ctrl = unsafe { get_ctrl(prov.add(OFF_PROV_BLDG_CONTAINER)) };
        if ctrl.is_null() || unsafe { *ctrl } != tag_id {
            continue;
        }
        let mut found = false;
        for k in 0..starv_len {
            if starv_ids[k] == prov_id {
                starv_cnts[k] = starv_cnts[k].saturating_add(1);
                if supply_ratio < starv_min_ratio[k] {
                    starv_min_ratio[k] = supply_ratio;
                }
                found = true;
                break;
            }
        }
        if !found && starv_len < 64 {
            starv_ids[starv_len] = prov_id;
            starv_ptrs[starv_len] = prov;
            starv_cnts[starv_len] = 1;
            starv_min_ratio[starv_len] = supply_ratio;
            starv_len += 1;
        }
    }
    if starv_len == 0 {
        return;
    }

    // Pick worst starving sector (>= 2 strained divisions, or >= 1 critically starving < 35% in sparse fronts)
    let mut best_idx = usize::MAX;
    let mut best_score = 0u32;
    for i in 0..starv_len {
        let mut score = starv_cnts[i] as u32;
        let sp = unsafe { *(starv_ptrs[i].add(OFF_PROV_STATIC_DATA) as *const *const u8) };
        if !sp.is_null() {
            let adj_ptr = unsafe { *(sp.add(0x70) as *const *const u8) };
            let adj_len = unsafe { *(sp.add(0x7c) as *const i32) };
            if !adj_ptr.is_null() && adj_len > 0 {
                let acap = (adj_len as usize).min(32);
                for j in 0..acap {
                    let adj = unsafe { adj_ptr.add(j * 0x30) };
                    if unsafe { *adj.add(0x10) } == 4 {
                        continue;
                    }
                    let nb_id = unsafe { *(adj.add(0x08) as *const i32) };
                    for k in 0..starv_len {
                        if starv_ids[k] == nb_id {
                            score += starv_cnts[k] as u32;
                            break;
                        }
                    }
                }
            }
        }
        if score > best_score {
            best_score = score;
            best_idx = i;
        }
    }
    if best_idx == usize::MAX || (best_score < 2 && starv_min_ratio[best_idx] >= 35_000) {
        return;
    }

    let worst_prov = starv_ptrs[best_idx];
    let can_constr: BldgContCanConstrFn =
        unsafe { std::mem::transmute(pie + OFF_BLDG_CONT_CAN_CONSTR) };

    // Prefer a safe 1-hop friendly rear province if the starving province sits on the active frontline
    let mut cands = [std::ptr::null::<u8>(); 2];
    let mut n_cands = 0usize;
    if unsafe { prov_borders_non_own_land(pie, gs, get_prov, worst_prov, tag_id) } {
        let sp = unsafe { *(worst_prov.add(OFF_PROV_STATIC_DATA) as *const *const u8) };
        if !sp.is_null() {
            let adj_ptr = unsafe { *(sp.add(0x70) as *const *const u8) };
            let adj_len = unsafe { *(sp.add(0x7c) as *const i32) };
            if !adj_ptr.is_null() && adj_len > 0 {
                let acap = (adj_len as usize).min(32);
                for j in 0..acap {
                    let adj = unsafe { adj_ptr.add(j * 0x30) };
                    if unsafe { *adj.add(0x10) } == 4 {
                        continue;
                    }
                    let nb_id = unsafe { *(adj.add(0x08) as *const i32) };
                    if nb_id <= 0 {
                        continue;
                    }
                    let nb_p = unsafe { get_prov(gs, nb_id) };
                    if nb_p.is_null() {
                        continue;
                    }
                    let nb_sp = unsafe { *(nb_p.add(OFF_PROV_STATIC_DATA) as *const *const u8) };
                    if nb_sp.is_null() || (unsafe { *nb_sp.add(0xd2) } & 1) == 0 {
                        continue;
                    }
                    let nb_cont = unsafe { nb_p.add(OFF_PROV_BLDG_CONTAINER) };
                    let nb_ctrl = unsafe { get_ctrl(nb_cont) };
                    if nb_ctrl.is_null() || unsafe { *nb_ctrl } != tag_id {
                        continue;
                    }
                    if !unsafe { prov_borders_non_own_land(pie, gs, get_prov, nb_p, tag_id) }
                        && unsafe { can_constr(nb_cont, supply_node, 0) } != 0
                    {
                        cands[n_cands] = nb_p;
                        n_cands += 1;
                        break;
                    }
                }
            }
        }
    }
    cands[n_cands] = worst_prov;
    n_cands += 1;

    let mut chosen = std::ptr::null::<u8>();
    for ci in 0..n_cands {
        let cand = cands[ci];
        let cont = unsafe { cand.add(OFF_PROV_BLDG_CONTAINER) };
        if unsafe { can_constr(cont, supply_node, 0) } == 0 {
            continue;
        }
        if unsafe { has_hub_or_port_within_2_hops(pie, gs, get_prov, cand, supply_node, naval_base) }
        {
            continue;
        }
        chosen = cand;
        break;
    }
    if chosen.is_null() {
        return;
    }

    // Claim 21-day cooldown atomically before emitting the native construction command
    if cd_slot
        .compare_exchange(last_hr, cur_hours.max(1), Ordering::AcqRel, Ordering::Relaxed)
        .is_err()
    {
        return;
    }

    // Emit native MP-sync-safe CAddConstructionCommand via HOI4's own operator new + CSession::Post
    let cont = unsafe { chosen.add(OFF_PROV_BLDG_CONTAINER) };
    let loc_ctor: SBldgLocCtorFn = unsafe { std::mem::transmute(pie + OFF_SBLDG_LOC_CTOR) };
    let op_new: OpNewFn = unsafe { std::mem::transmute(pie + OFF_OP_NEW_PLT) };
    let cmd_ctor: AddConstrCmdCtorFn = unsafe { std::mem::transmute(pie + OFF_ADD_CONSTR_CMD_CTOR) };
    let post_cmd: SessionPostFn = unsafe { std::mem::transmute(pie + OFF_SESSION_POST) };

    let mut loc_buf = [0u64; 4];
    unsafe { loc_ctor(&mut loc_buf, cont, supply_node) };
    let cmd = unsafe { op_new(ADD_CONSTR_CMD_SIZE) };
    if cmd.is_null() {
        return;
    }
    unsafe { cmd_ctor(cmd, 1, tag_ptr, &loc_buf, 1) };
    unsafe { post_cmd(cmd, tag_ptr) };

    let total_queued = AI_SUPPLY_HUBS_QUEUED.fetch_add(1, Ordering::Relaxed) + 1;
    if total_queued <= 16 {
        let chosen_id = unsafe { *(chosen.add(OFF_PROV_ID) as *const i32) };
        eprintln!(
            "[hoi4-rust-accel] AI queued wartime supply_node #{} (tag_id={}, prov={}, starving_sector={}, fac={})",
            total_queued, tag_id, chosen_id, best_score, total_fac
        );
    }
}

// ============================================================================
// v1.4 Module 14: Native Empty-Tile Capture (CMoveCommand + CSession::Post)
// Fixes frontline-balancing stalls in civil wars, Siberia, and Africa by
// ordering idle, healthy (>= 35% org) land divisions into adjacent EMPTY enemy
// provinces — strictly pocket-safe (>= 2 friendly divs in cur_prov OR zero
// enemy-occupied provinces bordering the empty target).
// ============================================================================
#[inline(always)]
unsafe fn is_enemy_tag_lin(pie: usize, country: *const u8, other_tag: i32) -> bool {
    if other_tag <= 0 {
        return false;
    }
    let get_idx: TagGetIndexFn = unsafe { std::mem::transmute(pie + OFF_TAG_GET_INDEX) };
    let is_war: CountryIsAtWarWithFn =
        unsafe { std::mem::transmute(pie + OFF_COUNTRY_IS_AT_WAR_WITH) };
    let idx = unsafe { get_idx(&other_tag) };
    let pair = [other_tag, idx];
    unsafe { is_war(country, &pair) != 0 }
}

unsafe fn try_capture_empty_enemy_tiles(ai_constr: *mut u8) {
    let pie = PIE_BASE.load(Ordering::Relaxed);
    if pie == 0 || ai_constr.is_null() {
        return;
    }
    let gs = unsafe { *((pie + OFF_GAMESTATE_PTR) as *const *const u8) };
    if gs.is_null() {
        return;
    }
    let gs_vt = unsafe { *(gs as *const *const usize) };
    if gs_vt.is_null() {
        return;
    }
    let get_prov_addr = unsafe { *gs_vt.add(0x50 / 8) };
    if get_prov_addr == 0 {
        return;
    }
    let get_prov: GsGetProvinceFn = unsafe { std::mem::transmute(get_prov_addr) };

    let obj = unsafe { *(ai_constr.add(0x08) as *const *mut u8) };
    if obj.is_null() {
        return;
    }
    let obj_vt = unsafe { *(obj as *const *const usize) };
    if obj_vt.is_null() {
        return;
    }
    let get_base_ai_addr = unsafe { *obj_vt.add(0x10 / 8) };
    if get_base_ai_addr == 0 {
        return;
    }
    let base_ai = unsafe { (std::mem::transmute::<usize, BaseAiResolveFn>(get_base_ai_addr))(obj) };
    if base_ai.is_null() {
        return;
    }
    let tag_ptr =
        unsafe { (std::mem::transmute::<usize, BaseAiGetTagFn>(pie + OFF_BASE_AI_GET_TAG))(base_ai) };
    let country = unsafe {
        (std::mem::transmute::<usize, BaseAiResolveFn>(pie + OFF_BASE_AI_GET_COUNTRY))(base_ai)
    };
    if tag_ptr.is_null() || country.is_null() {
        return;
    }
    let tag_id = unsafe { *tag_ptr };
    if tag_id == 0 {
        return;
    }

    let has_human: CountryHasHumanFn = unsafe { std::mem::transmute(pie + OFF_COUNTRY_HAS_HUMAN) };
    if unsafe { has_human(country) } != 0 {
        return;
    }
    let diplo = unsafe { *(country.add(OFF_COUNTRY_DIPLO_PTR) as *const *const u8) };
    if diplo.is_null() {
        return;
    }
    let is_at_war: DiploIsAtWarFn = unsafe { std::mem::transmute(pie + OFF_DIPLO_IS_AT_WAR) };
    if unsafe { is_at_war(diplo, 0) } == 0 {
        return;
    }

    let units_ptr = unsafe { *(country.add(OFF_COUNTRY_UNITS_DATA) as *const *const *const u8) };
    let units_len = unsafe { *(country.add(OFF_COUNTRY_UNITS_LEN) as *const i32) };
    if units_ptr.is_null() || units_len <= 0 {
        return;
    }

    let has_units: ProvHasUnitsOfTypeFn =
        unsafe { std::mem::transmute(pie + OFF_PROV_HAS_UNITS_OF_TYPE) };
    let is_retreating: UnitBoolFn = unsafe { std::mem::transmute(pie + OFF_UNIT_IS_RETREATING) };
    let can_move_into: UnitCanMoveIntoFn =
        unsafe { std::mem::transmute(pie + OFF_UNIT_CAN_MOVE_INTO) };
    let op_new: OpNewFn = unsafe { std::mem::transmute(pie + OFF_OP_NEW_PLT) };
    let move_ctor: MoveCmdCtorFn = unsafe { std::mem::transmute(pie + OFF_MOVE_CMD_CTOR) };
    let post_cmd: SessionPostFn = unsafe { std::mem::transmute(pie + OFF_SESSION_POST) };

    const MAX_CAPTURES_PER_DAY: usize = 6;
    let mut targeted_provs = [0i32; MAX_CAPTURES_PER_DAY];
    let mut moved_from_provs = [0i32; MAX_CAPTURES_PER_DAY];
    let mut moved_cnt = 0usize;

    let ucap = (units_len as usize).min(1024);
    for i in 0..ucap {
        if moved_cnt >= MAX_CAPTURES_PER_DAY {
            break;
        }
        let unit = unsafe { *units_ptr.add(i) };
        if unit.is_null() {
            continue;
        }
        // Skip units currently in combat or already moving
        if unsafe { *(unit.add(0x1e8) as *const usize) } != 0 {
            continue;
        }
        if unsafe { *(unit.add(0x1b4) as *const i32) } > 0 {
            continue;
        }
        let unit_vt = unsafe { *(unit as *const *const usize) };
        if unit_vt.is_null() {
            continue;
        }
        let as_land: UnitAsLandFn = unsafe { std::mem::transmute(*unit_vt.add(0x88 / 8)) };
        let land = unsafe { as_land(unit) };
        if land.is_null() || unsafe { is_retreating(land) } != 0 {
            continue;
        }
        let land_vt = unsafe { *(land as *const *const usize) };
        if land_vt.is_null() {
            continue;
        }
        let get_org: UnitGetOrgRatioFn = unsafe { std::mem::transmute(*land_vt.add(0x168 / 8)) };
        if unsafe { get_org(land) } < 35_000 {
            continue;
        }

        let cur_prov = unsafe { *(unit.add(0x1f0) as *const *const u8) };
        if cur_prov.is_null() {
            continue;
        }
        let cur_id = unsafe { *(cur_prov.add(OFF_PROV_ID) as *const i32) };
        let cur_sp = unsafe { *(cur_prov.add(OFF_PROV_STATIC_DATA) as *const *const u8) };
        if cur_id <= 0 || cur_sp.is_null() || (unsafe { *cur_sp.add(0xd2) } & 1) == 0 {
            continue;
        }

        // Count own units standing in cur_prov minus any already ordered away this tick
        let pu_ptr = unsafe { *(cur_prov.add(OFF_PROV_UNITS_DATA) as *const *const *const u8) };
        let pu_len = unsafe { *(cur_prov.add(OFF_PROV_UNITS_LEN) as *const i32) };
        let mut friendly_here: i32 = 0;
        if !pu_ptr.is_null() && pu_len > 0 {
            let pcap = (pu_len as usize).min(64);
            for k in 0..pcap {
                let u = unsafe { *pu_ptr.add(k) };
                if !u.is_null() && unsafe { *(u.add(0x1e0) as *const u32) } == tag_id {
                    friendly_here += 1;
                }
            }
        }
        for k in 0..moved_cnt {
            if moved_from_provs[k] == cur_id {
                friendly_here -= 1;
            }
        }

        let adj_ptr = unsafe { *(cur_sp.add(0x70) as *const *const u8) };
        let adj_len = unsafe { *(cur_sp.add(0x7c) as *const i32) };
        if adj_ptr.is_null() || adj_len <= 0 {
            continue;
        }
        let acap = (adj_len as usize).min(32);
        for j in 0..acap {
            let adj = unsafe { adj_ptr.add(j * 0x30) };
            if unsafe { *adj.add(0x10) } == 4 {
                continue;
            }
            let nb_id = unsafe { *(adj.add(0x08) as *const i32) };
            if nb_id <= 0 {
                continue;
            }
            let mut already_tgt = false;
            for k in 0..moved_cnt {
                if targeted_provs[k] == nb_id {
                    already_tgt = true;
                    break;
                }
            }
            if already_tgt {
                continue;
            }
            let nb = unsafe { get_prov(gs, nb_id) };
            if nb.is_null() {
                continue;
            }
            let nb_sp = unsafe { *(nb.add(OFF_PROV_STATIC_DATA) as *const *const u8) };
            if nb_sp.is_null() || (unsafe { *nb_sp.add(0xd2) } & 1) == 0 {
                continue;
            }
            let nb_ctrl = unsafe { *(nb.add(OFF_PROV_CTRL_TAG) as *const i32) };
            if nb_ctrl <= 0 || nb_ctrl as u32 == tag_id {
                continue;
            }
            if !unsafe { is_enemy_tag_lin(pie, country, nb_ctrl) } {
                continue;
            }
            // Strictly EMPTY enemy province (zero land units!)
            if unsafe { has_units(nb, 1) } != 0 {
                continue;
            }
            if unsafe { can_move_into(land, nb) } == 0 {
                continue;
            }

            // Pocket-safety check: if only 1 friendly div in cur_prov, ensure no enemy-occupied
            // land province borders the empty target (prevents lone divisions snaking into pockets)
            if friendly_here < 2 {
                let mut enemy_adj = false;
                let nb_adj_ptr = unsafe { *(nb_sp.add(0x70) as *const *const u8) };
                let nb_adj_len = unsafe { *(nb_sp.add(0x7c) as *const i32) };
                if !nb_adj_ptr.is_null() && nb_adj_len > 0 {
                    let ncap = (nb_adj_len as usize).min(32);
                    for m in 0..ncap {
                        let na = unsafe { nb_adj_ptr.add(m * 0x30) };
                        if unsafe { *na.add(0x10) } == 4 {
                            continue;
                        }
                        let nn_id = unsafe { *(na.add(0x08) as *const i32) };
                        if nn_id <= 0 || nn_id == cur_id {
                            continue;
                        }
                        let nn = unsafe { get_prov(gs, nn_id) };
                        if nn.is_null() {
                            continue;
                        }
                        let nn_sp = unsafe { *(nn.add(OFF_PROV_STATIC_DATA) as *const *const u8) };
                        if nn_sp.is_null() || (unsafe { *nn_sp.add(0xd2) } & 1) == 0 {
                            continue;
                        }
                        let nn_ctrl = unsafe { *(nn.add(OFF_PROV_CTRL_TAG) as *const i32) };
                        if nn_ctrl > 0
                            && nn_ctrl as u32 != tag_id
                            && unsafe { has_units(nn, 1) } != 0
                            && unsafe { is_enemy_tag_lin(pie, country, nn_ctrl) }
                        {
                            enemy_adj = true;
                            break;
                        }
                    }
                }
                if enemy_adj {
                    continue;
                }
            }

            // Construct, validate via CanExecute(0), and post native MP-safe CMoveCommand
            let cmd = unsafe { op_new(MOVE_CMD_SIZE) };
            if cmd.is_null() {
                break;
            }
            unsafe { move_ctor(cmd, unit, nb_id, 1, 2, 0, 0, 0, 0, 0) };
            let cmd_vt = unsafe { *(cmd as *const *const usize) };
            let can_exec: CmdCanExecFn = unsafe { std::mem::transmute(*cmd_vt.add(0x50 / 8)) };
            if unsafe { can_exec(cmd, 0) } != 0 {
                unsafe { post_cmd(cmd, tag_ptr) };
                targeted_provs[moved_cnt] = nb_id;
                moved_from_provs[moved_cnt] = cur_id;
                moved_cnt += 1;
                let tot = AI_EMPTY_TILES_CAPTURED.fetch_add(1, Ordering::Relaxed) + 1;
                if tot <= 16 {
                    eprintln!(
                        "[hoi4-rust-accel] AI captured empty enemy tile #{} (tag_id={}, {} -> {}, friendly_here={})",
                        tot, tag_id, cur_id, nb_id, friendly_here
                    );
                }
                break;
            } else {
                let dtor: CmdDtorFn = unsafe { std::mem::transmute(*cmd_vt.add(0x08 / 8)) };
                unsafe { dtor(cmd) };
            }
        }
    }
}

unsafe extern "C" fn hook_fix_supply_bottlenecks(ai_constr: *mut u8) {
    let orig: FixSupplyBottlenecksFn =
        unsafe { std::mem::transmute(ORIG_FIX_SUPPLY_BOTTLENECKS.load(Ordering::Relaxed)) };
    unsafe { orig(ai_constr) };
    if AI_SMART_SUPPLY_ENABLED.load(Ordering::Relaxed)
        && AI_SMART_SUPPLY_APPLIED.load(Ordering::Acquire)
    {
        unsafe { try_queue_wartime_supply_hub(ai_constr) };
    }
    if AI_CAPTURE_EMPTY_ENABLED.load(Ordering::Relaxed) {
        unsafe { try_capture_empty_enemy_tiles(ai_constr) };
    }
}

unsafe fn apply_ai_smart_supply_once(pie: usize) {
    if AI_SMART_SUPPLY_APPLIED
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Relaxed)
        .is_err()
    {
        return;
    }
    unsafe { resolve_supply_buildings_once(pie) };
    unsafe {
        let freq_ptr = (pie + OFF_NDEF_SUPPLY_FREQ_HRS) as *mut i32;
        let sat_ptr = (pie + OFF_NDEF_SUPPLY_SAT_THR) as *mut i64;
        let mot_freq_ptr = (pie + OFF_NDEF_SUPPLY_MOT_FREQ) as *mut i32;
        let div_mot_ptr = (pie + OFF_NDEF_DIV_SUPPLY_MOT) as *mut i64;
        let prio_hub_ptr = (pie + OFF_NDEF_PRIO_SUPPLY_BLDG) as *mut i64;
        let prio_rail_ptr = (pie + OFF_NDEF_PRIO_RAILWAY) as *mut i64;
        let depth_ptr = (pie + OFF_NDEF_SUPPLY_MAX_DEPTH) as *mut i64;
        let crisis_ptr = (pie + OFF_NDEF_SUPPLY_CRISIS_LIM) as *mut i64;

        let old_freq = *freq_ptr;
        let old_sat = *sat_ptr;
        let old_mfreq = *mot_freq_ptr;
        let old_dmot = *div_mot_ptr;
        let old_hub = *prio_hub_ptr;
        let old_rail = *prio_rail_ptr;
        let old_depth = *depth_ptr;
        let old_crisis = *crisis_ptr;

        // Sanity guard: only touch if defines look initialized (> 0)
        if old_freq > 0 && old_sat > 0 && old_hub > 0 && old_rail > 0 {
            if old_freq > 24 {
                *freq_ptr = 24; // Check supply bottlenecks daily (24h) instead of weekly (168h)
            }
            if old_sat > 65_000 {
                *sat_ptr = 65_000; // Proactively upgrade strained railways at 65% node saturation (vanilla 85%)
            }
            if old_mfreq > 24 {
                *mot_freq_ptr = 24; // Evaluate supply motorization daily (24h) instead of 52h
            }
            if old_dmot > 0 && old_dmot < 95_000 {
                *div_mot_ptr = 95_000; // Proactively motorize hubs with trucks at 95% supply (vanilla 80%)
            }
            if old_hub < 450_000 {
                *prio_hub_ptr = 450_000; // Boost Supply Hub / Port construction prio 1.10 -> 4.50
            }
            if old_rail < 800_000 {
                *prio_rail_ptr = 800_000; // Boost Railway construction prio 4.00 -> 8.00
            }
            if old_depth > 0 && old_depth < 900_000 {
                *depth_ptr = 900_000; // Expand AI supply-node BFS search depth 5 -> 9
            }
            if old_crisis > 0 && old_crisis < 25_000 {
                *crisis_ptr = 25_000; // React to low-supply fronts at 25% supply instead of 10%
            }
            eprintln!(
                "[hoi4-rust-accel] AI Smart Supply v2 active: freq={}h->{}h, sat={:.2}->{:.2}, mot_ratio={:.2}->{:.2}, hub_prio={:.2}->{:.2}, rail_prio={:.2}->{:.2}, bfs_depth={:.0}->{:.0}, supply_node_ptr={:#x}",
                old_freq,
                *freq_ptr,
                old_sat as f64 / 100_000.0,
                *sat_ptr as f64 / 100_000.0,
                old_dmot as f64 / 100_000.0,
                *div_mot_ptr as f64 / 100_000.0,
                old_hub as f64 / 100_000.0,
                *prio_hub_ptr as f64 / 100_000.0,
                old_rail as f64 / 100_000.0,
                *prio_rail_ptr as f64 / 100_000.0,
                old_depth as f64 / 100_000.0,
                *depth_ptr as f64 / 100_000.0,
                SUPPLY_NODE_BLDG_PTR.load(Ordering::Relaxed),
            );
        }
    }
}

// ============================================================================
// v1.1 Module 14: Smart Airbase Expansion
// Boosts queued `air_base` priority in CAIConstruction::GetBuildingQueuePrio (0x74f4d0)
// from UNSPECIFIED (0.50) by 3.2x (-> 1.60) so needed frontline airbases actually
// receive civilian factories ahead of mil/civ factories (0.70/0.80), without disturbing
// bunkers/silos/AA (0.50) or emergency supply hubs/railways (4.50/8.00).
// ============================================================================
unsafe extern "C" fn hook_get_bldg_queue_prio(self_ptr: *mut u8, item: *const u8) -> i64 {
    let orig: GetBldgQueuePrioFn =
        unsafe { std::mem::transmute(ORIG_GET_BLDG_QUEUE_PRIO.load(Ordering::Relaxed)) };
    let prio = unsafe { orig(self_ptr, item) };
    if prio > 0 && !item.is_null() {
        let constr = unsafe { *(item.add(OFF_CONSTR_ITEM_INNER) as *const *const u8) };
        if !constr.is_null() {
            let bldg = unsafe { *(constr.add(OFF_CONSTR_BUILDING_PTR) as *const *const u8) };
            if !bldg.is_null() && unsafe { *bldg.add(OFF_BLDG_IS_AIR_BASE) } != 0 {
                const MAX_SAFE_PRIO: i64 = 2_000_000_000_000_000;
                let boosted = if prio < (MAX_SAFE_PRIO / 16) {
                    (prio * 16) / 5
                } else {
                    MAX_SAFE_PRIO
                };
                AIRBASE_PRIO_BOOSTS.fetch_add(1, Ordering::Relaxed);
                return boosted;
            }
        }
    }
    prio
}

unsafe fn apply_ai_smart_airbase_once(pie: usize) {
    if AI_SMART_AIRBASE_DEFINES_APPLIED
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Relaxed)
        .is_err()
    {
        return;
    }
    let mut updated = 0usize;
    let mut raise_fp64 = |off: usize, target: i64| {
        let p = (pie + off) as *mut i64;
        let cur = unsafe { *p };
        if cur > 0 && cur < target {
            unsafe { *p = target };
            updated += 1;
        }
    };
    raise_fp64(OFF_NDEF_WANTED_PLANES_FAC, 75_000);     // 0.50 -> 0.75
    raise_fp64(OFF_NDEF_AIR_COV_STAY_PUT, 75_000);      // 0.50 -> 0.75 (Sheep AI: rebase closer when coverage < 75%)
    raise_fp64(OFF_NDEF_OUR_COMBATS_AIR, 45_000_000);   // 155 -> 450 (focus air & base demand on active combats)
    raise_fp64(OFF_NDEF_FIGHTERS_PER_PLN, 200_000);     // 1.0 -> 2.0
    raise_fp64(OFF_NDEF_DEF_SUPPLY_AIR, 900_000);       // 4.0 -> 9.0 (Sheep AI: protect supply hubs from logistics bombing)
    eprintln!(
        "[hoi4-rust-accel] AI Smart Airbase active: air_base queue prio 3.2x (0.50->1.60) + {}/5 air-coverage defines tuned",
        updated
    );
}

// ============================================================================
// v1.1 -> v1.2 Module 15: Tactical Prudence, Equipment Conservation & AIFC Spearheads
// Strictly adjusts NDefines::NAI decision weights (zero player/combat cheats).
// Never raises PLAN_ATTACK_MIN_STRENGTH so empty-tile advances are never blocked!
// ============================================================================
unsafe fn apply_ai_tactical_prudence_once(pie: usize) {
    if AI_TACTICAL_PRUDENCE_APPLIED
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Relaxed)
        .is_err()
    {
        return;
    }
    let sanity_def = unsafe { *((pie + OFF_NDEF_ATK_HEAVY_DEF_LIM) as *const i64) };
    let sanity_enc = unsafe { *((pie + OFF_NDEF_ENCIRCLE_DISC_HRS) as *const i32) };
    if sanity_def <= 0 || sanity_enc <= 0 {
        return;
    }

    let raise_fp64 = |off: usize, min_target: i64| -> usize {
        let p = (pie + off) as *mut i64;
        if unsafe { *p } < min_target {
            unsafe { *p = min_target };
            1
        } else {
            0
        }
    };
    let lower_fp64 = |off: usize, max_target: i64| -> usize {
        let p = (pie + off) as *mut i64;
        if unsafe { *p } > max_target {
            unsafe { *p = max_target };
            1
        } else {
            0
        }
    };
    let raise_i32 = |off: usize, min_target: i32| -> usize {
        let p = (pie + off) as *mut i32;
        if unsafe { *p } < min_target {
            unsafe { *p = min_target };
            1
        } else {
            0
        }
    };
    let lower_i32 = |off: usize, max_target: i32| -> usize {
        let p = (pie + off) as *mut i32;
        if unsafe { *p } > max_target {
            unsafe { *p = max_target };
            1
        } else {
            0
        }
    };

    let mut changed = 0usize;
    // 1. Stop suicidal attacks into defended stacks & abort losing meatgrinders in 12h (never blocks empty tiles!)
    changed += raise_fp64(OFF_NDEF_ATK_HEAVY_DEF_LIM, 95_000);   // 0.50 -> 0.95 (refuse bad odds vs fortified/entrenched tiles; empty tile def=0!)
    changed += raise_fp64(OFF_NDEF_PLAN_VAL_EXECUTE, -15_000);   // -0.50 -> -0.15 (stop launching hopeless front-wide suicide plans)
    changed += lower_fp64(OFF_NDEF_LOC_BAL_ADVANCE, 0);          // >0   -> 0.00 (Sheep AI: step into empty/weak tiles without hesitation)
    changed += raise_fp64(OFF_NDEF_STR_UNIT_WEAK, 50_000);       // 0.30 -> 0.50 (mark under-equipped divisions weak before they melt)
    changed += raise_fp64(OFF_NDEF_ORG_UNIT_WEAK, 35_000);       // 0.25 -> 0.35
    changed += lower_fp64(OFF_NDEF_PLAN_WEAK_ABORT, 45_000);     // 0.65 -> 0.45 (abort offensive plans once 45% of army drops below 50% str)
    changed += lower_fp64(OFF_NDEF_CANCEL_DISADV_RAT, 115_000);  // 1.50 -> 1.15 (abort losing combats fast)
    changed += lower_i32(OFF_NDEF_CANCEL_MIN_DUR_HR, 12);        // 48h  -> 12h  (save guns instead of grinding 2 days into forts)
    changed += raise_fp64(OFF_NDEF_DESP_WEAK_STR_LIM, 65_000);   // 0.35 -> 0.65

    // 2. Equipment Stockpile Discipline (prevent deep negative rifle/equipment death spiral without starving tank factories!)
    changed += raise_fp64(OFF_NDEF_MIN_FIELD_STR_BLD, 78_000);   // 0.70 -> 0.78 (v1.3: softened from 0.88 so mid-war tank training continues)
    changed += raise_fp64(OFF_NDEF_START_TRAIN_EQ, 85_000);      // 0.40 -> 0.85
    changed += raise_fp64(OFF_NDEF_STOP_TRAIN_EQ, 75_000);       // 0.25 -> 0.75 (stop exercising & burning equipment when stockpile < 75%)
    changed += raise_fp64(OFF_NDEF_DEPLOY_MIN_EQ_WAR, 90_000);   // 0.85 -> 0.90 (avoid deploying half-equipped divisions into combat)
    changed += lower_fp64(OFF_NDEF_PROD_EQ_SURPLUS, 55_000);     // 0.80 -> 0.55 (v1.3: stop hoarding 100% surplus rifles, freeing mils for armor!)

    // 3. Smarter front assessment & terrain discipline (gentle entrenchment weight so divisions stay fluid)
    changed += raise_fp64(OFF_NDEF_FRONT_EVAL_ACC, 150_000);     // 1.00 -> 1.50 (Sheep AI)
    changed += raise_fp64(OFF_NDEF_FRONT_EVAL_AIR, 200_000);     // 1.00 -> 2.00 (Sheep AI: respect air superiority)
    changed += raise_fp64(OFF_NDEF_FRONT_EVAL_SUPLCK, 150_000);  // 1.00 -> 1.50 (Sheep AI: avoid overstacking low-supply pockets)
    changed += raise_fp64(OFF_NDEF_TERRAIN_IMPORT, 1_000_000);   // >= 10.0
    changed += lower_fp64(OFF_NDEF_TANKS_MOUNTAINS, -1_500_000); // <= -15.0 (Sheep AI: keep tanks out of mountains)
    changed += lower_fp64(OFF_NDEF_TANKS_JUNGLE, -2_000_000);    // <= -20.0 (Sheep AI: keep tanks out of jungles)
    changed += raise_fp64(OFF_NDEF_ENTRENCH_WEIGHT, 220_000);    // 2.0  -> 2.2  (v1.2+: lowered from 4.0 to stop gluing divisions in place)

    // 4. Daily pocket/bulge detection & UNBLOCKED empty-tile micro capture
    changed += lower_i32(OFF_NDEF_ENCIRCLE_DISC_HRS, 24);        // 72h  -> 24h (daily encirclement discovery)
    changed += raise_fp64(OFF_NDEF_BULGE_LOW_CUTOFF, 120_000);   // 0.95 -> 1.20 (Sheep AI)
    changed += raise_fp64(OFF_NDEF_BULGE_HIGH_CUTOFF, 180_000);  // 1.50 -> 1.80 (Sheep AI)
    changed += lower_i32(OFF_NDEF_CUTOFF_EDGE_PROX, 1);          // 2    -> 1    (Sheep AI)
    changed += raise_i32(OFF_NDEF_MAX_MICRO_ATKS, 10);           // 3    -> 10   (v1.3: grab all open gaps along inactive/active fronts)
    changed += lower_fp64(OFF_NDEF_MICRO_INACT_VAL, -100_000);   // 0.25 -> -1.00 (v1.3: kills dead-zone against PLAN_VALUE_TO_EXECUTE; occupied tiles still require superiority!)

    // 5. Armored/Shock Force Concentration (AIFC — Sheep AI / Discord AI spearheads)
    changed += lower_i32(OFF_NDEF_AIFC_UPD_DAYS, 4);             // 5d   -> 4d
    changed += raise_fp64(OFF_NDEF_AIFC_UNIT_RATIO, 22_000);     // 0.15 -> 0.22 (22% shock spearhead allocation)
    changed += raise_fp64(OFF_NDEF_AIFC_ACTIVATE_ORG, 30_000);   // 0.20 -> 0.30 (v1.2+: balanced from 0.42 so spearheads actually roll)
    changed += lower_i32(OFF_NDEF_AIFC_NUDGE_DAYS, 10);          // 15d  -> 10d
    changed += raise_fp64(OFF_NDEF_AIFC_HARDNESS, 80_000_000);   // 300  -> 800  (Sheep AI: pack armor/mech into spearheads)
    changed += raise_fp64(OFF_NDEF_AIFC_ARMOR, 8_000_000);       // 30   -> 80   (Sheep AI)
    changed += raise_fp64(OFF_NDEF_AIFC_SOFT_ATK, 1_200_000);    // 6    -> 12   (Sheep AI)
    changed += raise_fp64(OFF_NDEF_AIFC_HARD_ATK, 1_600_000);    // 8    -> 16   (Sheep AI)
    changed += raise_fp64(OFF_NDEF_AIFC_EXPERIENCE, 50_000_000); // 300  -> 500  (Sheep AI)
    changed += raise_fp64(OFF_NDEF_AIFC_TGT_SUPPLY, 3_000_000);  // 20   -> 30   (Sheep AI: aim spearheads at enemy supply hubs)
    changed += raise_fp64(OFF_NDEF_AIFC_TGT_PORT, 1_500_000);    // 10   -> 15   (Sheep AI)
    changed += raise_fp64(OFF_NDEF_AIFC_TGT_PORT_LVL, 200_000);  // 1    -> 2    (Sheep AI)
    changed += raise_fp64(OFF_NDEF_AIFC_COST_MTN, 400_000);      // 3.0  -> 4.0  (Sheep AI: route spearheads around mountains)

    // 6. Anti-Frontline-Shuffling (v1.4.1: stop ripping divisions across fronts and zeroing org!)
    changed += lower_fp64(OFF_NDEF_REASSIGN_FRONT, 20_000);        // 0.50 -> 0.20
    changed += lower_fp64(OFF_NDEF_REASSIGN_FRONT_COMBAT, 5_000);  // 0.20 -> 0.05 (never yank active combat divs)
    changed += lower_fp64(OFF_NDEF_REDEPLOY_DIST_VS_ORD, 50_000);  // 1.00 -> 0.50 (require path > 2x order_size before strat-redeploy; safe against path.len==1 underflow)

    eprintln!(
        "[hoi4-rust-accel] AI Tactical Prudence v4 active ({}/43 NAI defines tuned; empty_tile_patch={}, anti_shuffle=1)",
        changed,
        EMPTY_TILE_FIX_APPLIED.load(Ordering::Relaxed)
    );
}

// ============================================================================
// v1.4 Module 17: Smart Naval Invasions & Strike Force Concentration
// Stops 1-2 div portless suicide landings and encourages concentrated battle fleets.
// ============================================================================
unsafe fn apply_ai_smart_naval_once(pie: usize) {
    if AI_SMART_NAVAL_APPLIED
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Relaxed)
        .is_err()
    {
        return;
    }
    let mut changed = 0usize;
    let raise_fp64 = |off: usize, target: i64| -> usize {
        let p = (pie + off) as *mut i64;
        if unsafe { *p } > 0 && unsafe { *p } < target {
            unsafe { *p = target };
            1
        } else {
            0
        }
    };
    let lower_fp64 = |off: usize, target: i64| -> usize {
        let p = (pie + off) as *mut i64;
        if unsafe { *p } > target {
            unsafe { *p = target };
            1
        } else {
            0
        }
    };
    let raise_i32 = |off: usize, target: i32| -> usize {
        let p = (pie + off) as *mut i32;
        if unsafe { *p } > 0 && unsafe { *p } < target {
            unsafe { *p = target };
            1
        } else {
            0
        }
    };
    let lower_i32 = |off: usize, target: i32| -> usize {
        let p = (pie + off) as *mut i32;
        if unsafe { *p } > target {
            unsafe { *p = target };
            1
        } else {
            0
        }
    };

    // 1. Smart Naval Invasions (target ports, require >= 60% ready & 65% org, abort suicide beaches in 5d)
    changed += lower_fp64(OFF_NDEF_INV_NO_PORT_FAC, 5_000);       // 0.30 -> 0.05 (strongly avoid portless suicide landings)
    changed += raise_fp64(OFF_NDEF_INV_MIN_PLAN_VAL, 45_000);     // 0.30 -> 0.45
    changed += raise_fp64(OFF_NDEF_INV_MIN_ORG_FAC, 65_000);      // 0.40 -> 0.65
    changed += raise_fp64(OFF_NDEF_INV_MIN_UNITS_RDY, 60_000);    // 0.25 -> 0.60 (never launch with 25% of planned force)
    changed += raise_fp64(OFF_NDEF_INV_UNITS_MIN_PLAN, 85_000);   // 0.75 -> 0.85
    changed += lower_fp64(OFF_NDEF_INV_CANCEL_DISADV, 180_000);   // 3.50 -> 1.80
    changed += lower_i32(OFF_NDEF_INV_CANCEL_MIN_HRS, 120);       // 720h -> 120h (5 days instead of 30)
    changed += raise_i32(OFF_NDEF_INV_FAILED_AVOID, 120);         // 60d  -> 120d
    changed += lower_fp64(OFF_NDEF_INV_FAILED_PORT_FAC, 35_000);  // 0.66 -> 0.35
    changed += raise_i32(OFF_NDEF_INV_COASTAL_PER_ORD, 40);       // 24   -> 40 (concentrate into larger landings)

    // 2. Strike Force & Battle Fleet Concentration
    changed += raise_i32(OFF_NDEF_NAVY_PREF_MAX_SIZE, 60);        // 25 -> 60
    changed += raise_i32(OFF_NDEF_CAP_TF_MAX_CAP, 16);            // 12 -> 16
    changed += raise_i32(OFF_NDEF_SCR_TF_MAX_SHIP, 24);           // 12 -> 24
    changed += lower_i32(OFF_NDEF_STRIKE_RECALC_DAYS, 3);         // 5d -> 3d
    let p_patrol = (pie + OFF_NDEF_MAX_PATROL_STRIKE_R) as *mut f32;
    if unsafe { *p_patrol } > 2.0f32 {
        unsafe { *p_patrol = 2.0f32 }; // 4.0 -> 2.0 (keep more ships in main Strike Force)
        changed += 1;
    }

    eprintln!(
        "[hoi4-rust-accel] AI Smart Naval active: {}/15 invasion & strike-force defines tuned",
        changed
    );
}

// ============================================================================
// v1.3 Module 16: Smart Armored & Mechanized AI (Zero Cheats, Industrial-Scaled)
// 1. Tunes 9 NDefines::NAI tank template/upgrade/XP weights so AI actually designs,
//    upgrades, and deploys modern armor instead of spamming 1945 pure infantry.
// 2. Hooks CAIStrategyStore::GetStrategyValue (vtable 0x434ea90+0x28) to scale
//    `role_ratio` (0x10) & `template_prio` (0x15) dynamically by military factory
//    count (>= 22 mils) and calendar year (>= 1938), mod-safely via CToken::FromString.
// ============================================================================
unsafe fn resolve_token_lin(pie: usize, name: &[u8]) -> i32 {
    let from_str: CTokenFromStringFn = unsafe { std::mem::transmute(pie + OFF_CTOKEN_FROM_STRING) };
    let sv = LinStrView {
        len: name.len() as u64,
        ptr: name.as_ptr(),
    };
    unsafe { from_str(&sv) }
}

unsafe fn apply_ai_smart_armor_once(pie: usize) {
    if AI_SMART_ARMOR_DEFINES_APPLIED
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Relaxed)
        .is_err()
    {
        return;
    }
    // Resolve mod-safe CToken IDs once on the main thread
    let t_arm = unsafe { resolve_token_lin(pie, b"armor") };
    let t_mech = unsafe { resolve_token_lin(pie, b"mechanized") };
    let t_inf = unsafe { resolve_token_lin(pie, b"infantry") };
    if t_arm > 0 {
        TOK_ARMOR.store(t_arm, Ordering::Release);
    }
    if t_mech > 0 {
        TOK_MECHANIZED.store(t_mech, Ordering::Release);
    }
    if t_inf > 0 {
        TOK_INFANTRY.store(t_inf, Ordering::Release);
    }

    let mut changed = 0usize;
    let raise_fp64 = |off: usize, target: i64| -> usize {
        let p = (pie + off) as *mut i64;
        if unsafe { *p } > 0 && unsafe { *p } < target {
            unsafe { *p = target };
            1
        } else {
            0
        }
    };
    let raise_i32 = |off: usize, target: i32| -> usize {
        let p = (pie + off) as *mut i32;
        if unsafe { *p } > 0 && unsafe { *p } < target {
            unsafe { *p = target };
            1
        } else {
            0
        }
    };
    let lower_i32 = |off: usize, target: i32| -> usize {
        let p = (pie + off) as *mut i32;
        if unsafe { *p } > target {
            unsafe { *p = target };
            1
        } else {
            0
        }
    };

    changed += raise_i32(OFF_NDEF_UPG_DEFICIT_DAYS, 210);        // 60 -> 210 (allow upgrading divisions to med/heavy/modern tanks!)
    changed += raise_fp64(OFF_NDEF_UPG_PCT_FORCES, 35_000);      // 0.20 -> 0.35
    changed += lower_i32(OFF_NDEF_UPG_DIV_RELUCT, 4);            // 7 -> 4
    changed += lower_i32(OFF_NDEF_VAR_XP_MOD_LAND, 20);          // 35 -> 20 (NSB tank designer XP threshold)
    changed += lower_i32(OFF_NDEF_VAR_XP_LEG_LAND, 20);          // 35 -> 20
    changed += lower_i32(OFF_NDEF_VAR_XP_RES_LAND, 30);          // 50 -> 30
    changed += raise_fp64(OFF_NDEF_XP_UPG_LAND_EQ, 200_000);     // 1.00 -> 2.00
    changed += raise_fp64(OFF_NDEF_XP_UPD_LAND_TMPL, 250_000);   // 2.00 -> 2.50
    changed += raise_fp64(OFF_NDEF_ASSIGN_TANKS_WAR, 1_000_000); // 6.00 -> 10.00

    eprintln!(
        "[hoi4-rust-accel] AI Smart Armor active: {}/9 tank/upgrade defines tuned | tokens(armor={}, mech={}, inf={})",
        changed, t_arm, t_mech, t_inf
    );
}

unsafe extern "C" fn hook_get_strategy_value(
    store: *const u8,
    stype: i32,
    id: i32,
    target: i32,
) -> i32 {
    let orig: GetStrategyValueFn =
        unsafe { std::mem::transmute(ORIG_GET_STRATEGY_VALUE.load(Ordering::Relaxed)) };
    let val = unsafe { orig(store, stype, id, target) };

    // Ultra-fast exit for all non-role_ratio / non-template_prio / targeted queries
    if (stype != 0x10 && stype != 0x15) || target != 0 || store.is_null() {
        return val;
    }
    let t_arm = TOK_ARMOR.load(Ordering::Acquire);
    let t_mech = TOK_MECHANIZED.load(Ordering::Acquire);
    let t_inf = TOK_INFANTRY.load(Ordering::Acquire);
    if id != t_arm && id != t_mech && id != t_inf {
        return val;
    }

    let pie = PIE_BASE.load(Ordering::Relaxed);
    if pie == 0 {
        return val;
    }
    // Verify store is embedded inside CCountryAI (+0x68) before casting
    let ai_ptr = unsafe { store.sub(0x68) as *mut u8 };
    if unsafe { *(ai_ptr as *const usize) } != pie + OFF_COUNTRY_AI_VT {
        return val;
    }

    let gs = unsafe { *((pie + OFF_GAMESTATE_PTR) as *const *const u8) };
    if gs.is_null() {
        return val;
    }
    let raw_date = unsafe { *(gs.add(OFF_GS_CDATE_HOURS) as *const i32) };
    let year = raw_date.wrapping_sub(0x029c55c0) / 8760;
    if year < 1938 {
        return val;
    }

    let country = unsafe { get_country_from_ai(ai_ptr) };
    if country.is_null() {
        return val;
    }
    let has_human: CountryHasHumanFn = unsafe { std::mem::transmute(pie + OFF_COUNTRY_HAS_HUMAN) };
    if unsafe { has_human(country) } != 0 {
        return val;
    }
    let constr = unsafe { *(country.add(OFF_COUNTRY_CONSTR_PTR) as *const *const u8) };
    if constr.is_null() {
        return val;
    }
    let total_fac = unsafe { *(constr.add(0x2d0) as *const i64) } / 100_000;
    let civ_fac = unsafe { *(constr.add(0x310) as *const i32) } as i64;
    let dock_fac = unsafe { *(constr.add(0x318) as *const i32) } as i64;
    let mil = total_fac - civ_fac - dock_fac;
    if mil < 22 {
        return val;
    }

    let mut delta: i32 = 0;
    if stype == 0x10 {
        // role_ratio
        if id == t_arm && val > -40 {
            delta = if mil >= 55 && year >= 1940 {
                30
            } else if mil >= 35 && year >= 1939 {
                22
            } else {
                14
            };
        } else if id == t_mech && mil >= 50 && year >= 1941 && val > -40 {
            delta = 12;
        } else if id == t_inf && mil >= 40 && year >= 1940 && val > 55 {
            delta = -15;
        }
    } else if stype == 0x15 {
        // template_prio
        if id == t_arm && val > -40 {
            delta = 40;
        } else if id == t_mech && mil >= 50 && year >= 1941 && val > -40 {
            delta = 25;
        }
    }

    if delta != 0 {
        AI_ARMOR_STRATEGY_BOOSTS.fetch_add(1, Ordering::Relaxed);
        val.saturating_add(delta)
    } else {
        val
    }
}

// ============================================================================
// v1.2 Module 16: Smart Peace Conference Anti-Bordergore
// Hooks CPeaceWinnerAi::CalcStateActionScore (0xeafef0) so AI strongly prefers
// contiguous states adjacent to its own territory or states already claimed
// earlier in the same peace conference (3x..5x), and avoids inland enclaves (-95%)
// and scattered coastal snipes (-75% until beachhead expands), while never
// penalizing a country's own cores!
// ============================================================================
unsafe fn adjust_peace_bordergore_score(
    pie: usize,
    conf: *const u8,
    taker: *const i32,
    negotiator: *const i32,
    state_id: i32,
    action_type: i32,
    score: i64,
) -> i64 {
    if score <= 0 || pie == 0 || conf.is_null() || taker.is_null() || state_id < 1 {
        return score;
    }
    let is_take = action_type == PEACE_ACT_TAKE_STATES || action_type == PEACE_ACT_GIVE_STATES;
    let is_puppet = action_type == PEACE_ACT_PUPPET;
    if !is_take && !is_puppet {
        return score;
    }
    let taker_tag = unsafe { *taker };
    if taker_tag <= 0 {
        return score;
    }
    let gs = unsafe { *((pie + OFF_GAMESTATE_PTR) as *const *const u8) };
    if gs.is_null() {
        return score;
    }
    let n_states = unsafe { *(gs.add(OFF_GS_STATES_LEN) as *const i32) };
    if state_id >= n_states {
        return score;
    }
    let states_arr = unsafe { *(gs.add(OFF_GS_STATES_DATA) as *const *const *const u8) };
    if states_arr.is_null() {
        return score;
    }
    let state = unsafe { *states_arr.add(state_id as usize) };
    if state.is_null() {
        return score;
    }

    // Never penalize a nation reclaiming its own core state (e.g. East Prussia / Alsace / Warsaw)
    let core_len = unsafe { *(state.add(OFF_STATE_CORES_LEN) as *const i32) };
    if core_len > 0 {
        let cores = unsafe { *(state.add(OFF_STATE_CORES_DATA) as *const *const i32) };
        if !cores.is_null() {
            let cap = (core_len as usize).min(64);
            for i in 0..cap {
                if unsafe { *cores.add(i) } == taker_tag {
                    return score;
                }
            }
        }
    }

    let state_data = unsafe { *(state.add(OFF_STATE_DATA_PTR) as *const *const u8) };
    if state_data.is_null() {
        return score;
    }
    let is_coastal = unsafe { *state_data.add(OFF_STATEDATA_IS_COASTAL) } != 0;
    let nb_len = unsafe { *(state_data.add(OFF_STATEDATA_NEIGHBORS_LEN) as *const i32) };
    let nb_ptr = unsafe { *(state_data.add(OFF_STATEDATA_NEIGHBORS_DATA) as *const *const i32) };
    let is_claimed: PeaceIsStateClaimedFn =
        unsafe { std::mem::transmute(pie + OFF_PEACE_IS_STATE_CLAIMED) };

    let neg_tag = if is_puppet && !negotiator.is_null() {
        let t = unsafe { *negotiator };
        if t > 0 && t != taker_tag { t } else { 0 }
    } else {
        0
    };

    let mut adj_taker: i32 = 0;
    let mut adj_overlord: i32 = 0;
    if nb_len > 0 && !nb_ptr.is_null() {
        let cap = (nb_len as usize).min(64);
        for i in 0..cap {
            let nb_id = unsafe { *nb_ptr.add(i) };
            if nb_id < 1 || nb_id >= n_states {
                continue;
            }
            let nb_state = unsafe { *states_arr.add(nb_id as usize) };
            if nb_state.is_null() {
                continue;
            }
            let nb_owner = unsafe { *(nb_state.add(OFF_STATE_OWNER_TAG) as *const i32) };
            if nb_owner == taker_tag || unsafe { is_claimed(conf, nb_id, taker) } != 0 {
                adj_taker += 1;
            } else if neg_tag > 0
                && (nb_owner == neg_tag || unsafe { is_claimed(conf, nb_id, negotiator) } != 0)
            {
                adj_overlord += 1;
            }
        }
    }

    const MAX_SAFE_SCORE: i64 = 1_000_000_000_000_000;
    let adjusted = if adj_taker >= 1 {
        // Contiguous to owned or already-claimed state: 3x (1 nb), 4x (2 nbs), 5x (3+ nbs)
        let mult = (2 + adj_taker.min(3)) as i64;
        if score < (MAX_SAFE_SCORE / mult) {
            score * mult
        } else {
            MAX_SAFE_SCORE
        }
    } else if is_puppet && adj_overlord >= 1 {
        // First puppet state next to its overlord: 2.5x
        if score < (MAX_SAFE_SCORE / 5) {
            (score * 5) / 2
        } else {
            MAX_SAFE_SCORE
        }
    } else if is_coastal {
        // Non-adjacent coastal foothold: 0.25x (strongly preferred over inland enclave on Turn 1,
        // then Turn 2+ contiguous beachhead expansion wins by 12x..20x!)
        (score / 4).max(1)
    } else {
        // Disconnected inland enclave: -95% penalty (still >= 1 as last-resort fallback)
        (score / 20).max(1)
    };

    PEACE_BORDERGORE_ADJUSTS.fetch_add(1, Ordering::Relaxed);
    adjusted
}

unsafe extern "C" fn hook_peace_calc_state_score(
    conf: *const u8,
    taker: *const i32,
    giver: *const i32,
    negotiator: *const i32,
    state_id: i32,
    action_type: i32,
    extra1: usize,
    extra2: usize,
    debug_str: usize,
) -> i64 {
    let orig: PeaceCalcStateScoreFn =
        unsafe { std::mem::transmute(ORIG_PEACE_CALC_STATE_SCORE.load(Ordering::Relaxed)) };
    let score = unsafe {
        orig(
            conf,
            taker,
            giver,
            negotiator,
            state_id,
            action_type,
            extra1,
            extra2,
            debug_str,
        )
    };
    let pie = PIE_BASE.load(Ordering::Relaxed);
    unsafe {
        adjust_peace_bordergore_score(pie, conf, taker, negotiator, state_id, action_type, score)
    }
}

unsafe fn apply_ai_smart_peace_once(pie: usize) {
    if AI_SMART_PEACE_DEFINES_APPLIED
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Relaxed)
        .is_err()
    {
        return;
    }
    let mut changed = 0usize;
    unsafe {
        let fold_ptr = (pie + OFF_NDEF_PEACE_FOLD_TURNS) as *mut i32;
        if *fold_ptr > 1 {
            *fold_ptr = 1;
            changed += 1;
        }
        let dist_ptr = (pie + OFF_NDEF_PEACE_TIE_DIST) as *mut i64;
        if *dist_ptr > 0 && *dist_ptr < 400_000 {
            *dist_ptr = 400_000; // 1.0 -> 4.0 distance tie-breaker
            changed += 1;
        }
    }
    eprintln!(
        "[hoi4-rust-accel] AI Smart Peace active: contiguous border scoring (3x..5x / -95% inland enclaves) + {}/2 peace defines tuned",
        changed
    );
}

// ============================================================================
// v1.0 Module 14: Dormant Peaceful Minor AI Hourly Throttling
// Staggers CCountryAI hourly passes (1 in 4 non-midnight hours) strictly for
// peaceful, <12-state, non-human, non-major AI minors while preserving exact
// CAICore countdown timers (+0x10, +0x14).
// ============================================================================
#[inline(always)]
unsafe fn get_country_from_ai(self_ptr: *mut u8) -> *mut u8 {
    if self_ptr.is_null() {
        return std::ptr::null_mut();
    }
    let owner = unsafe { *(self_ptr.add(0x08) as *const *mut u8) };
    if owner.is_null() {
        return std::ptr::null_mut();
    }
    let owner_vt = unsafe { *(owner as *const *const usize) };
    if owner_vt.is_null() {
        return std::ptr::null_mut();
    }
    let access_fn: OwnerAccessCountryFn = unsafe { std::mem::transmute(*owner_vt.add(0x58 / 8)) };
    unsafe { access_fn(owner) }
}

unsafe extern "C" fn hook_country_ai_pass1(self_ptr: *mut u8, arg1: usize, arg2: usize) {
    let orig: CountryAiPassFn =
        unsafe { std::mem::transmute(ORIG_COUNTRY_AI_PASS1.load(Ordering::Relaxed)) };
    let pie = PIE_BASE.load(Ordering::Relaxed);

    if pie != 0 && !self_ptr.is_null() && unsafe { *self_ptr.add(0x60) } != 0 {
        let country = unsafe { get_country_from_ai(self_ptr) };
        if !country.is_null() {
            let tag_id = unsafe { *(country.add(OFF_COUNTRY_TAG_ID) as *const u32) };
            let bit_idx = (tag_id as usize) & 511;
            let word_idx = bit_idx >> 6;
            let bit_mask = 1u64 << (bit_idx & 63);

            let num_states = unsafe { *(country.add(OFF_COUNTRY_NUM_STATES) as *const i32) };
            let is_major = unsafe { *country.add(OFF_COUNTRY_IS_MAJOR) };
            let gs = unsafe { *((pie + OFF_GAMESTATE_PTR) as *const *const u8) };

            let mut throttle = false;
            if (1..12).contains(&num_states) && is_major == 0 && !gs.is_null() {
                let raw_date = unsafe { *(gs.add(OFF_GS_CDATE_HOURS) as *const i32) };
                let total_hours = raw_date.wrapping_add(0xfd63aa40u32 as i32) as u32;
                // Never skip midnight (hour 0) or the country's staggered 4-hour slot
                if !total_hours.is_multiple_of(24)
                    && ((total_hours.wrapping_add(tag_id)) & 3) != 0
                {
                    let c10_ptr = unsafe { self_ptr.add(0x10) as *mut i32 };
                    let c14_ptr = unsafe { self_ptr.add(0x14) as *mut i32 };
                    let c10 = unsafe { *c10_ptr };
                    let c14 = unsafe { *c14_ptr };
                    // Never skip if CAICore daily/periodic countdown is due this hour
                    if c10 > 1 && c14 > 1 {
                        let has_human: CountryHasHumanFn =
                            unsafe { std::mem::transmute(pie + OFF_COUNTRY_HAS_HUMAN) };
                        if unsafe { has_human(country) } == 0 {
                            let diplo =
                                unsafe { *(country.add(OFF_COUNTRY_DIPLO_PTR) as *const *const u8) };
                            if !diplo.is_null() {
                                let is_at_war: DiploIsAtWarFn =
                                    unsafe { std::mem::transmute(pie + OFF_DIPLO_IS_AT_WAR) };
                                if unsafe { is_at_war(diplo, 0) } == 0 {
                                    unsafe {
                                        *c10_ptr = c10 - 1;
                                        *c14_ptr = c14 - 1;
                                    }
                                    throttle = true;
                                }
                            }
                        }
                    }
                }
            }

            if throttle {
                MINOR_SKIP_BITMAP[word_idx].fetch_or(bit_mask, Ordering::Relaxed);
                MINOR_AI_SKIPPED.fetch_add(1, Ordering::Relaxed);
                return;
            } else {
                MINOR_SKIP_BITMAP[word_idx].fetch_and(!bit_mask, Ordering::Relaxed);
            }
        }
    }

    MINOR_AI_EXECUTED.fetch_add(1, Ordering::Relaxed);
    unsafe { orig(self_ptr, arg1, arg2) };
}

unsafe extern "C" fn hook_country_ai_pass2(self_ptr: *mut u8, arg1: usize, arg2: usize) {
    if !self_ptr.is_null() && unsafe { *self_ptr.add(0x60) } != 0 {
        let country = unsafe { get_country_from_ai(self_ptr) };
        if !country.is_null() {
            let tag_id = unsafe { *(country.add(OFF_COUNTRY_TAG_ID) as *const u32) };
            let bit_idx = (tag_id as usize) & 511;
            let word = MINOR_SKIP_BITMAP[bit_idx >> 6].load(Ordering::Relaxed);
            if (word & (1u64 << (bit_idx & 63))) != 0 {
                MINOR_AI_SKIPPED.fetch_add(1, Ordering::Relaxed);
                return;
            }
        }
    }
    let orig: CountryAiPassFn =
        unsafe { std::mem::transmute(ORIG_COUNTRY_AI_PASS2.load(Ordering::Relaxed)) };
    unsafe { orig(self_ptr, arg1, arg2) };
}

// ============================================================================
// SIGPROF Sampling Profiler
// ============================================================================
unsafe extern "C" fn sigprof_handler(_sig: c_int, _info: *mut c_void, uctx: *mut c_void) {
    if !BENCH_RECORDING.load(Ordering::Relaxed) || uctx.is_null() {
        return;
    }
    let rip = unsafe { *((uctx as usize + UCONTEXT_RIP_OFF) as *const usize) };
    let base = PIE_BASE.load(Ordering::Relaxed);
    let key = if rip >= base + HOI4_TEXT_START && rip < base + HOI4_TEXT_END {
        (rip - base) & !0xff
    } else {
        (rip & !0xff) | 0x8000_0000_0000_0000
    };

    let is_main = unsafe { pthread_self() } == MAIN_PTHREAD.load(Ordering::Relaxed);
    if is_main {
        TOTAL_SAMPLES_MAIN.fetch_add(1, Ordering::Relaxed);
    } else {
        TOTAL_SAMPLES_WORK.fetch_add(1, Ordering::Relaxed);
    }

    let mut idx = hash_usize(key) & (SAMPLE_BUCKETS - 1);
    for _ in 0..32 {
        let slot = &SAMPLES[idx];
        let existing = slot.key.load(Ordering::Relaxed);
        if existing == key
            || (existing == 0
                && slot
                    .key
                    .compare_exchange(0, key, Ordering::Relaxed, Ordering::Relaxed)
                    .is_ok())
        {
            if is_main {
                slot.main_hits.fetch_add(1, Ordering::Relaxed);
            } else {
                slot.worker_hits.fetch_add(1, Ordering::Relaxed);
            }
            return;
        }
        idx = (idx + 1) & (SAMPLE_BUCKETS - 1);
    }
}

fn install_sigprof_sampler() {
    let sa = Sigaction {
        sa_sigaction: sigprof_handler as *const () as usize,
        sa_flags: (SA_SIGINFO | SA_RESTART) as usize,
        sa_restorer: 0,
        sa_mask: [0u64; 16],
    };
    let it = Itimerval {
        it_interval: Timeval {
            tv_sec: 0,
            tv_usec: 4000,
        },
        it_value: Timeval {
            tv_sec: 0,
            tv_usec: 4000,
        },
    };
    unsafe {
        sigaction(SIGPROF, &sa, std::ptr::null_mut());
        setitimer(ITIMER_PROF, &it, std::ptr::null_mut());
    }
}

// ============================================================================
// Lock-Free Rust Hybrid Spin-Then-Futex Worker Synchronization Engine
// ============================================================================
const EV_UNSIGNALED: u32 = 0;
const EV_SLEEPING: u32 = 1;
const EV_SIGNALED: u32 = 2;
const EV_SPINNING: u32 = 3;
const CEVENT_SIZE: usize = 0x60;

#[inline(always)]
unsafe fn ev_state<'a>(ev: *mut u8) -> &'a AtomicU32 {
    unsafe { &*((ev.add(0x58)) as *const AtomicU32) }
}

#[inline(always)]
unsafe fn fast_signal_event(ev: *mut u8) {
    let st = unsafe { ev_state(ev) };
    let prev = st.swap(EV_SIGNALED, Ordering::AcqRel);
    if prev == EV_SLEEPING {
        FAST_WAKE_FUTEX.fetch_add(1, Ordering::Relaxed);
        futex_wake_all(st);
    } else if prev == EV_SPINNING {
        FAST_WAKE_SPUN.fetch_add(1, Ordering::Relaxed);
    }
}

unsafe extern "C" fn hook_event_wait_auto(ev: *mut u8) {
    let st = unsafe { ev_state(ev) };
    if st
        .compare_exchange(
            EV_SIGNALED,
            EV_UNSIGNALED,
            Ordering::AcqRel,
            Ordering::Relaxed,
        )
        .is_ok()
    {
        return;
    }

    let spin_limit = SPIN_ITERS.load(Ordering::Relaxed);
    if spin_limit > 0 {
        let _ = st.compare_exchange(
            EV_UNSIGNALED,
            EV_SPINNING,
            Ordering::AcqRel,
            Ordering::Relaxed,
        );
        for _ in 0..spin_limit {
            if st.load(Ordering::Acquire) == EV_SIGNALED {
                if st
                    .compare_exchange(
                        EV_SIGNALED,
                        EV_UNSIGNALED,
                        Ordering::AcqRel,
                        Ordering::Relaxed,
                    )
                    .is_ok()
                {
                    return;
                }
            }
            std::hint::spin_loop();
        }
    }

    loop {
        let cur = st.load(Ordering::Acquire);
        if cur == EV_SIGNALED {
            if st
                .compare_exchange(
                    EV_SIGNALED,
                    EV_UNSIGNALED,
                    Ordering::AcqRel,
                    Ordering::Relaxed,
                )
                .is_ok()
            {
                return;
            }
            continue;
        }
        if cur != EV_SLEEPING {
            if st
                .compare_exchange(cur, EV_SLEEPING, Ordering::AcqRel, Ordering::Relaxed)
                .is_err()
            {
                continue;
            }
        }
        futex_wait(st, EV_SLEEPING);
    }
}

unsafe extern "C" fn hook_event_wait_manual(ev: *mut u8) {
    let st = unsafe { ev_state(ev) };
    if st.load(Ordering::Acquire) == EV_SIGNALED {
        return;
    }
    let spin_limit = SPIN_ITERS.load(Ordering::Relaxed);
    if spin_limit > 0 {
        let _ = st.compare_exchange(
            EV_UNSIGNALED,
            EV_SPINNING,
            Ordering::AcqRel,
            Ordering::Relaxed,
        );
        for _ in 0..spin_limit {
            if st.load(Ordering::Acquire) == EV_SIGNALED {
                return;
            }
            std::hint::spin_loop();
        }
    }
    loop {
        let cur = st.load(Ordering::Acquire);
        if cur == EV_SIGNALED {
            return;
        }
        if cur != EV_SLEEPING {
            if st
                .compare_exchange(cur, EV_SLEEPING, Ordering::AcqRel, Ordering::Relaxed)
                .is_err()
            {
                continue;
            }
        }
        futex_wait(st, EV_SLEEPING);
    }
}

unsafe extern "C" fn hook_event_signal(ev: *mut u8) {
    unsafe { fast_signal_event(ev) };
}

unsafe extern "C" fn hook_event_is_signaled(ev: *mut u8) -> u8 {
    let st = unsafe { ev_state(ev) };
    u8::from(st.load(Ordering::Acquire) == EV_SIGNALED)
}

unsafe extern "C" fn hook_event_reset(ev: *mut u8) {
    let st = unsafe { ev_state(ev) };
    let _ = st.compare_exchange(
        EV_SIGNALED,
        EV_UNSIGNALED,
        Ordering::AcqRel,
        Ordering::Relaxed,
    );
}

unsafe extern "C" fn hook_pool_wake_one_or_all(pool: *const u8, wake_all: c_int) {
    let count = unsafe { *((pool.add(0x18)) as *const i32) };
    if count <= 0 {
        return;
    }
    let slots = unsafe { *(pool as *const *mut u8) };
    if slots.is_null() {
        return;
    }

    if wake_all != 0 {
        for i in 0..(count as usize) {
            unsafe { fast_signal_event(slots.add(i * CEVENT_SIZE)) };
        }
        return;
    }

    for i in 0..(count as usize) {
        let ev = unsafe { slots.add(i * CEVENT_SIZE) };
        let st = unsafe { ev_state(ev) };
        if st.load(Ordering::Relaxed) == EV_SPINNING {
            if st
                .compare_exchange(
                    EV_SPINNING,
                    EV_SIGNALED,
                    Ordering::AcqRel,
                    Ordering::Relaxed,
                )
                .is_ok()
            {
                FAST_WAKE_SPUN.fetch_add(1, Ordering::Relaxed);
                return;
            }
        }
    }

    for i in 0..(count as usize) {
        let ev = unsafe { slots.add(i * CEVENT_SIZE) };
        let st = unsafe { ev_state(ev) };
        if st.load(Ordering::Relaxed) == EV_SLEEPING {
            unsafe { fast_signal_event(ev) };
            return;
        }
    }

    for i in 0..(count as usize) {
        unsafe { fast_signal_event(slots.add(i * CEVENT_SIZE)) };
    }
}

unsafe extern "C" fn hook_pool_wake_n(pool: *const u8, n: c_int) {
    if n <= 0 {
        return;
    }
    let count = unsafe { *((pool.add(0x18)) as *const i32) };
    if count <= 0 {
        return;
    }
    if n >= count {
        unsafe { hook_pool_wake_one_or_all(pool, 1) };
    } else {
        for _ in 0..n {
            unsafe { hook_pool_wake_one_or_all(pool, 0) };
        }
    }
}

// ============================================================================
// Breakthrough 8: Lock-Free Zero-Bounce Rust Worker Scheduler (0x3666d70)
// ============================================================================
// Paradox's stock PopTaskOrWait (0x3666d70) keeps 1 designated watcher awake while
// running_tasks > 0, but does so by decrementing searching_workers (lock sub),
// returning true with task=NULL, looping through ExecuteTask(NULL), and incrementing
// searching_workers (lock add) millions of times/sec without a single PAUSE!
// In late-game profiles that cache-line bounce storm burns ~31.7% of ALL worker CPU.
// Our Rust replacement preserves Paradox's exact SeqCst invariant and ring-buffer
// bit-swizzle while keeping the watcher in a zero-lock read-only PAUSE poll, skipping
// ExecuteTask(NULL) round-trips and WaitForTask's double system_clock::now() calls.
#[inline(always)]
fn swizzle_ring_idx(c: u32, mask: u32) -> usize {
    let mut d = c & mask;
    let a = ((c >> 3) ^ c) & 7;
    d ^= a;
    (d ^ (a << 3)) as usize
}

#[inline(always)]
unsafe fn try_pop_ring(
    head: &AtomicI32,
    tail: &AtomicI32,
    slots: *const AtomicUsize,
    mask: u32,
) -> Option<usize> {
    let mut t = tail.load(Ordering::Acquire);
    loop {
        let h = head.load(Ordering::Acquire);
        if h.wrapping_sub(t) <= 0 {
            return None;
        }
        match tail.compare_exchange_weak(
            t,
            t.wrapping_add(1),
            Ordering::AcqRel,
            Ordering::Acquire,
        ) {
            Ok(_) => {
                let idx = swizzle_ring_idx(t as u32, mask);
                let slot = unsafe { &*slots.add(idx) };
                loop {
                    let ptr = slot.swap(0, Ordering::AcqRel);
                    if ptr != 0 {
                        return Some(ptr);
                    }
                    while slot.load(Ordering::Acquire) == 0 {
                        core::arch::x86_64::_mm_pause();
                    }
                }
            }
            Err(actual) => {
                t = actual;
            }
        }
    }
}

unsafe extern "C" fn hook_pop_task_or_wait(worker: *mut u8, out_task: *mut usize) -> u8 {
    let manager = unsafe { *(worker as *const *mut u8) };
    let prio = unsafe { *(worker.add(0x3c) as *const u32) };
    let bucket: usize = if prio < 3 { (2 - prio) as usize } else { 1 };
    let bucket_off = bucket * 0x2c0;
    let pool = unsafe { manager.add(bucket_off + 0x100) };
    let running_tasks = unsafe { &*(manager.add(bucket_off + 0x140) as *const AtomicI32) };
    let searching_workers = unsafe { &*(pool.add(0x80) as *const AtomicI32) };
    let global_q = unsafe { *(manager.add(bucket_off + 0x398) as *const *mut u8) };

    let local_head = unsafe { &*(worker.add(0x100) as *const AtomicI32) };
    let local_tail = unsafe { &*(worker.add(0x140) as *const AtomicI32) };
    let local_slots = unsafe { worker.add(0x180) as *const AtomicUsize };

    let global_head = unsafe { &*(global_q.add(0x80) as *const AtomicI32) };
    let global_tail = unsafe { &*(global_q.add(0xc0) as *const AtomicI32) };
    let global_slots = unsafe { global_q.add(0x100) as *const AtomicUsize };

    let state_ptr = unsafe { &*(worker.add(0x13) as *const AtomicU8) };
    let pie = PIE_BASE.load(Ordering::Relaxed);
    let wake_fn: PoolWakeFn = unsafe { std::mem::transmute(pie + OFF_WAKE_ONE) };
    let wait_fn: EventWaitFn = unsafe { std::mem::transmute(pie + OFF_WAIT_AUTO1) };

    unsafe { *out_task = 0 };
    searching_workers.fetch_add(1, Ordering::SeqCst);

    let mut watcher_spins: u64 = 0;

    loop {
        if let Some(task) = unsafe { try_pop_ring(local_head, local_tail, local_slots, 0xfff) } {
            if watcher_spins > 0 {
                SCHED_WATCHER_POLLS.fetch_add(watcher_spins, Ordering::Relaxed);
            }
            unsafe { *out_task = task };
            if searching_workers.fetch_sub(1, Ordering::SeqCst) == 1 {
                unsafe { wake_fn(pool, 0) };
            }
            return 1;
        }
        if let Some(task) = unsafe { try_pop_ring(global_head, global_tail, global_slots, 0x7ffff) }
        {
            if watcher_spins > 0 {
                SCHED_WATCHER_POLLS.fetch_add(watcher_spins, Ordering::Relaxed);
            }
            unsafe { *out_task = task };
            if searching_workers.fetch_sub(1, Ordering::SeqCst) == 1 {
                unsafe { wake_fn(pool, 0) };
            }
            return 1;
        }
        if state_ptr.load(Ordering::Acquire) == 2 {
            if watcher_spins > 0 {
                SCHED_WATCHER_POLLS.fetch_add(watcher_spins, Ordering::Relaxed);
            }
            unsafe { wake_fn(pool, 1) };
            searching_workers.fetch_sub(1, Ordering::SeqCst);
            return 0;
        }

        // Designated watcher fast-poll: zero atomic RMWs while running_tasks > 0!
        if searching_workers.load(Ordering::Acquire) == 1 && running_tasks.load(Ordering::Acquire) > 0
        {
            watcher_spins = watcher_spins.saturating_add(1);
            let pauses = if watcher_spins < 64 {
                4
            } else if watcher_spins < 512 {
                16
            } else if watcher_spins < 4096 {
                64
            } else {
                128
            };
            for _ in 0..pauses {
                core::arch::x86_64::_mm_pause();
            }
            continue;
        }

        if watcher_spins > 0 {
            SCHED_WATCHER_POLLS.fetch_add(watcher_spins, Ordering::Relaxed);
            watcher_spins = 0;
        }

        let prev = searching_workers.fetch_sub(1, Ordering::SeqCst);
        if prev == 1 {
            let rt = running_tasks.load(Ordering::SeqCst);
            let g_avail = global_head
                .load(Ordering::SeqCst)
                .wrapping_sub(global_tail.load(Ordering::Acquire));
            let l_avail = local_head
                .load(Ordering::SeqCst)
                .wrapping_sub(local_tail.load(Ordering::Acquire));
            if rt > 0 || g_avail > 0 || l_avail > 0 {
                searching_workers.fetch_add(1, Ordering::SeqCst);
                core::arch::x86_64::_mm_pause();
                continue;
            }
        }

        // Sleep via CEvent::Wait (hits our lock-free futex hook when FAST_EVENT=1, zero clocks!)
        let sleep_flag = unsafe { worker.add(0x15) };
        let old_sleep = unsafe { *sleep_flag };
        unsafe { *sleep_flag = 1 };
        let worker_idx = unsafe { *(worker.add(0x38) as *const i32) } as isize;
        let events_base = unsafe { *(pool as *const *mut u8) };
        let ev = unsafe { events_base.offset(worker_idx * CEVENT_SIZE as isize) };
        unsafe { wait_fn(ev) };
        unsafe { *sleep_flag = old_sleep };

        searching_workers.fetch_add(1, Ordering::SeqCst);
    }
}

// ============================================================================
// Breakthrough 9: Lock-Free FastTaskWait (CTask::Wait @ 0x3662b10)
// ============================================================================
// Paradox's stock CTask::Wait loops sched_yield() into the Linux kernel on every
// iteration after 30 pause rounds (~10.8% kernel + 9.3% user = 20.1% of Main Thread CPU!).
// Replacing tight sched_yield() traps with low-power _mm_pause() (and yielding only
// once every 4096 cycles if contention drags on) eliminates 200k+ syscalls/sec while
// reacting to task completion in < 20 ns.
unsafe extern "C" fn hook_task_wait(task: *const u8) {
    if task.is_null() {
        return;
    }
    let job = unsafe { *(task.add(0x18) as *const *mut u8) };
    if job.is_null() {
        return;
    }
    let pie = PIE_BASE.load(Ordering::Relaxed);
    let try_run: TryRunJobFn = unsafe { std::mem::transmute(pie + OFF_TRY_RUN_JOB) };
    unsafe { try_run(job) };
    let job = unsafe { *(task.add(0x18) as *const *mut u8) };
    if job.is_null() {
        return;
    }
    let st = unsafe { &*(job.add(0x40) as *const AtomicU8) };
    let mut spins: u32 = 0;
    while (st.load(Ordering::Acquire) & 0xfe) != 2 {
        for _ in 0..8 {
            core::arch::x86_64::_mm_pause();
        }
        spins = spins.wrapping_add(1);
        if (spins & 0xfff) == 0 {
            unsafe { sched_yield() };
        }
    }
}

// Intercepts hoi4's internal 8 sched_yield@plt call sites (most notably 0x20ff554 inside
// gamestate.hourly parallel wait and 5 CTask::Wait clones), replacing repetitive kernel
// syscall traps with low-power _mm_pause() and yielding only once every 256 iterations.
unsafe extern "C" fn hook_hoi4_sched_yield() -> c_int {
    thread_local! {
        static CNT: std::cell::Cell<u32> = const { std::cell::Cell::new(0) };
    }
    let n = CNT.with(|c| {
        let v = c.get().wrapping_add(1);
        c.set(v);
        v
    });
    if (n & 0xff) != 0 {
        for _ in 0..24 {
            core::arch::x86_64::_mm_pause();
        }
        YIELD_ELIDED.fetch_add(1, Ordering::Relaxed);
        return 0;
    }
    unsafe { sched_yield() }
}

// ============================================================================
// Zone Profiler & Tick Detector Hooks
// ============================================================================
#[inline(always)]
fn record_zone(name_ptr: usize, elapsed_ns: u64) {
    if name_ptr == 0 {
        return;
    }

    let mut hourly_ptr = HOURLY_PTR.load(Ordering::Relaxed);
    if hourly_ptr == 0 {
        let s = unsafe { CStr::from_ptr(name_ptr as *const c_char) }.to_bytes();
        if s == b"gamestate.hourly" {
            HOURLY_PTR.store(name_ptr, Ordering::Relaxed);
            hourly_ptr = name_ptr;
        }
    }

    if name_ptr == hourly_ptr {
        let h = TOTAL_HOURS.fetch_add(1, Ordering::Relaxed) + 1;
        let max_hours = BENCH_MAX_HOURS.load(Ordering::Relaxed);
        if max_hours > 0 {
            let warmup = BENCH_WARMUP_HOURS.load(Ordering::Relaxed);
            if h == warmup + 1 {
                reset_stats();
                BENCH_START_CPU_NS.store(now_cpu_ns(), Ordering::Relaxed);
                BENCH_START_NS.store(now_ns(), Ordering::Relaxed);
                BENCH_RECORDING.store(true, Ordering::Release);
            } else if h >= warmup + max_hours {
                if !BENCH_FINISHED.swap(true, Ordering::AcqRel) {
                    BENCH_RECORDING.store(false, Ordering::Release);
                    emit_report(max_hours);
                    unsafe { _exit(0) };
                }
            }
        } else if COLLECT_ZONES.load(Ordering::Relaxed)
            && !PROFILE_STARTED_SIM.swap(true, Ordering::AcqRel)
        {
            // Interactive HOI4_ACCEL_PROFILE=1: start clean measurement when actual gameplay begins!
            reset_stats();
            BENCH_START_CPU_NS.store(now_cpu_ns(), Ordering::Relaxed);
            BENCH_START_NS.store(now_ns(), Ordering::Relaxed);
            BENCH_RECORDING.store(true, Ordering::Release);
        }
    }

    if !COLLECT_ZONES.load(Ordering::Relaxed) {
        return;
    }
    if BENCH_MAX_HOURS.load(Ordering::Relaxed) > 0 && !BENCH_RECORDING.load(Ordering::Relaxed) {
        return;
    }

    let is_main = unsafe { pthread_self() } == MAIN_PTHREAD.load(Ordering::Relaxed);
    let mut idx = hash_usize(name_ptr) & (TABLE_CAP - 1);
    for _ in 0..64 {
        let slot = &ZONES[idx];
        let existing = slot.name_ptr.load(Ordering::Acquire);
        if existing == name_ptr
            || (existing == 0
                && slot
                    .name_ptr
                    .compare_exchange(0, name_ptr, Ordering::AcqRel, Ordering::Acquire)
                    .is_ok())
        {
            slot.calls.fetch_add(1, Ordering::Relaxed);
            slot.total_ns.fetch_add(elapsed_ns, Ordering::Relaxed);
            if is_main {
                slot.main_ns.fetch_add(elapsed_ns, Ordering::Relaxed);
            } else {
                slot.worker_ns.fetch_add(elapsed_ns, Ordering::Relaxed);
            }
            let mut cur_max = slot.max_ns.load(Ordering::Relaxed);
            while elapsed_ns > cur_max {
                match slot.max_ns.compare_exchange_weak(
                    cur_max,
                    elapsed_ns,
                    Ordering::Relaxed,
                    Ordering::Relaxed,
                ) {
                    Ok(_) => break,
                    Err(v) => cur_max = v,
                }
            }
            return;
        }
        idx = (idx + 1) & (TABLE_CAP - 1);
    }
}

fn reset_stats() {
    for slot in &ZONES {
        if slot.name_ptr.load(Ordering::Relaxed) != 0 {
            slot.calls.store(0, Ordering::Relaxed);
            slot.total_ns.store(0, Ordering::Relaxed);
            slot.max_ns.store(0, Ordering::Relaxed);
            slot.main_ns.store(0, Ordering::Relaxed);
            slot.worker_ns.store(0, Ordering::Relaxed);
        }
    }
    for s in &SAMPLES {
        if s.key.load(Ordering::Relaxed) != 0 {
            s.main_hits.store(0, Ordering::Relaxed);
            s.worker_hits.store(0, Ordering::Relaxed);
        }
    }
    TOTAL_SAMPLES_MAIN.store(0, Ordering::Relaxed);
    TOTAL_SAMPLES_WORK.store(0, Ordering::Relaxed);
    FAST_WAKE_SPUN.store(0, Ordering::Relaxed);
    FAST_WAKE_FUTEX.store(0, Ordering::Relaxed);
    SCHED_WATCHER_POLLS.store(0, Ordering::Relaxed);
    YIELD_ELIDED.store(0, Ordering::Relaxed);
    FRAMES_RENDERED.store(0, Ordering::Relaxed);
    ACTIVE_SIM_FRAMES.store(0, Ordering::Relaxed);
    ACTIVE_SIM_NS.store(0, Ordering::Relaxed);
    BURST_EXTRA_TICKS.store(0, Ordering::Relaxed);
    WORST_FRAME_NS.store(0, Ordering::Relaxed);
    MID_TICK_CHECKS.store(0, Ordering::Relaxed);
    MID_TICK_FAST_SKIP.store(0, Ordering::Relaxed);
    MID_TICK_RENDERS.store(0, Ordering::Relaxed);
    PARFOR_REBALANCED.store(0, Ordering::Relaxed);
    PARFOR_TOTAL.store(0, Ordering::Relaxed);
    SLAB_ALLOC_HITS.store(0, Ordering::Relaxed);
    SLAB_FREE_HITS.store(0, Ordering::Relaxed);
    SLAB_FALLBACKS.store(0, Ordering::Relaxed);
    CW_REBUILDS_COALESCED.store(0, Ordering::Relaxed);
    AIRBASE_PRIO_BOOSTS.store(0, Ordering::Relaxed);
    PEACE_BORDERGORE_ADJUSTS.store(0, Ordering::Relaxed);
    MINOR_AI_SKIPPED.store(0, Ordering::Relaxed);
    MINOR_AI_EXECUTED.store(0, Ordering::Relaxed);
}

unsafe extern "C" fn hook_scoped_timer_ctor(this: *mut u64, name: *const c_char) {
    unsafe {
        *this.add(0) = name as usize as u64;
        *this.add(1) = 0;
        *this.add(2) = if COLLECT_ZONES.load(Ordering::Relaxed) {
            now_ns()
        } else {
            0
        };
        *this.add(3) = 0;
    }
}

unsafe extern "C" fn hook_scoped_timer_dtor(this: *mut u64) {
    let (name_ptr, start_ns) = unsafe { (*this.add(0) as usize, *this.add(2)) };
    if start_ns != u64::MAX && name_ptr != 0 {
        let elapsed = if COLLECT_ZONES.load(Ordering::Relaxed) {
            now_ns().saturating_sub(start_ns)
        } else {
            0
        };
        record_zone(name_ptr, elapsed);
    }
}

unsafe extern "C" fn hook_thread_prof_ctor(
    this: *mut u64,
    _parent: usize,
    name: *const c_char,
    _level: c_int,
) {
    unsafe {
        *this.add(0) = name as usize as u64;
        *this.add(1) = now_ns();
    }
}

unsafe extern "C" fn hook_thread_prof_dtor(this: *mut u64) {
    let (name_ptr, start_ns) = unsafe { (*this.add(0) as usize, *this.add(1)) };
    if start_ns != u64::MAX && name_ptr != 0 {
        let elapsed = now_ns().saturating_sub(start_ns);
        record_zone(name_ptr, elapsed);
    }
}

unsafe extern "C" fn hook_hw_conc_min4() -> u32 {
    let n = OVERRIDE_THREADS.load(Ordering::Relaxed);
    if n >= 2 { n as u32 } else { 8 }
}

// ============================================================================
// LD_PRELOAD interposition for pthread_setname_np -> Smart Affinity
// ============================================================================
fn init_cpu_topology() {
    let mut primaries: Vec<u32> = Vec::new();
    let mut siblings: Vec<u32> = Vec::new();
    let mut main_sibling: Option<u32> = None;
    let mut seen = [false; 128];

    for cpu in 0..128usize {
        let p = format!("/sys/devices/system/cpu/cpu{}/topology/thread_siblings_list", cpu);
        let Ok(txt) = fs::read_to_string(&p) else {
            break;
        };
        if seen[cpu] {
            continue;
        }
        let mut group: Vec<u32> = Vec::new();
        for part in txt.trim().split(',') {
            if let Some((a, b)) = part.split_once('-') {
                if let (Ok(s), Ok(e)) = (a.parse::<u32>(), b.parse::<u32>()) {
                    for x in s..=e {
                        group.push(x);
                    }
                }
            } else if let Ok(x) = part.parse::<u32>() {
                group.push(x);
            }
        }
        if group.is_empty() {
            group.push(cpu as u32);
        }
        group.sort_unstable();
        group.dedup();
        for &c in &group {
            if (c as usize) < seen.len() {
                seen[c as usize] = true;
            }
        }
        let primary = group[0];
        if primary != 0 {
            primaries.push(primary);
        }
        for &sib in group.iter().skip(1) {
            if primary == 0 {
                main_sibling = Some(sib);
            } else {
                siblings.push(sib);
            }
        }
    }

    // Order for Task_N workers: physical cores 1..N first, then non-main SMT siblings, then CPU 0's sibling last!
    let mut worker_order = Vec::new();
    worker_order.extend_from_slice(&primaries);
    worker_order.extend_from_slice(&siblings);
    if let Some(ms) = main_sibling {
        worker_order.push(ms);
    }
    if worker_order.is_empty() {
        worker_order.extend([1, 2, 3, 4, 5, 6, 7, 9, 10, 11, 12, 13, 14, 15, 8]);
    }
    let wlen = worker_order.len().min(64);
    for (i, &c) in worker_order.iter().take(wlen).enumerate() {
        TOPO_WORKER_CPUS[i].store(c, Ordering::Relaxed);
    }
    TOPO_WORKER_LEN.store(wlen, Ordering::Release);

    // Order for Task_BN background workers: SMT siblings only (fallback to last primaries if no SMT)
    let mut bg_order = Vec::new();
    bg_order.extend_from_slice(&siblings);
    if let Some(ms) = main_sibling {
        bg_order.push(ms);
    }
    if bg_order.is_empty() {
        bg_order.extend(primaries.iter().rev().copied());
    }
    let blen = bg_order.len().min(64);
    for (i, &c) in bg_order.iter().take(blen).enumerate() {
        TOPO_BG_CPUS[i].store(c, Ordering::Relaxed);
    }
    TOPO_BG_LEN.store(blen, Ordering::Release);
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn pthread_setname_np(thread: usize, name: *const c_char) -> c_int {
    let real_fn: Option<PthreadSetnameFn> = unsafe {
        let ptr = dlsym(RTLD_NEXT, c"pthread_setname_np".as_ptr());
        if ptr.is_null() {
            None
        } else {
            Some(std::mem::transmute(ptr))
        }
    };

    if ACTIVE_FOR_HOI4.load(Ordering::Relaxed)
        && AFFINITY_ENABLED.load(Ordering::Relaxed)
        && !name.is_null()
        && thread == unsafe { pthread_self() }
    {
        if let Ok(s) = unsafe { CStr::from_ptr(name) }.to_str() {
            if s.starts_with("Task_N") {
                let len = TOPO_WORKER_LEN.load(Ordering::Acquire);
                if len > 0 {
                    let slot = WORKER_PIN_COUNTER.fetch_add(1, Ordering::Relaxed);
                    let cpu = TOPO_WORKER_CPUS[slot % len].load(Ordering::Relaxed) as usize;
                    pin_current_thread_to_cpu(cpu);
                }
            } else if s.starts_with("Task_BN") {
                let len = TOPO_BG_LEN.load(Ordering::Acquire);
                if len > 0 {
                    let slot = BG_PIN_COUNTER.fetch_add(1, Ordering::Relaxed);
                    let cpu = TOPO_BG_CPUS[slot % len].load(Ordering::Relaxed) as usize;
                    pin_current_thread_to_cpu(cpu);
                }
            }
        }
    }

    match real_fn {
        Some(f) => unsafe { f(thread, name) },
        None => 0,
    }
}

// ============================================================================
// Telemetry & Report Emitter
// ============================================================================
fn read_per_thread_cpu() -> Vec<(String, i32, u64)> {
    let mut rows = Vec::new();
    if let Ok(entries) = fs::read_dir("/proc/self/task") {
        for entry in entries.flatten() {
            let tid: i32 = entry
                .file_name()
                .to_str()
                .and_then(|s| s.parse().ok())
                .unwrap_or(0);
            let comm = fs::read_to_string(entry.path().join("comm"))
                .unwrap_or_default()
                .trim()
                .to_string();
            if let Ok(stat) = fs::read_to_string(entry.path().join("stat")) {
                if let Some(rparen) = stat.rfind(')') {
                    let fields: Vec<&str> = stat[rparen + 2..].split_whitespace().collect();
                    if fields.len() >= 13 {
                        let utime: u64 = fields[11].parse().unwrap_or(0);
                        let stime: u64 = fields[12].parse().unwrap_or(0);
                        rows.push((comm, tid, utime + stime));
                    }
                }
            }
        }
    }
    rows.sort_by(|a, b| b.2.cmp(&a.2));
    rows
}

fn emit_report(measured_hours: u64) {
    let end_ns = now_ns();
    let start_ns = BENCH_START_NS.load(Ordering::Relaxed);
    let wall_ns = end_ns.saturating_sub(start_ns).max(1);
    let wall_s = wall_ns as f64 / 1e9;
    let hours_per_sec = measured_hours as f64 / wall_s;
    let days_per_sec = hours_per_sec / 24.0;
    let ms_per_day = (wall_s * 1000.0) / (measured_hours as f64 / 24.0);

    let cpu_ns = now_cpu_ns().saturating_sub(BENCH_START_CPU_NS.load(Ordering::Relaxed));
    let cpu_s = cpu_ns as f64 / 1e9;
    let avg_active_cores = cpu_s / wall_s;

    let mut zones: Vec<(String, u64, u64, u64, u64, u64)> = Vec::new();
    for slot in &ZONES {
        let p = slot.name_ptr.load(Ordering::Acquire);
        if p == 0 {
            continue;
        }
        let calls = slot.calls.load(Ordering::Relaxed);
        if calls == 0 {
            continue;
        }
        let total = slot.total_ns.load(Ordering::Relaxed);
        let max = slot.max_ns.load(Ordering::Relaxed);
        let main = slot.main_ns.load(Ordering::Relaxed);
        let worker = slot.worker_ns.load(Ordering::Relaxed);
        let name = unsafe { CStr::from_ptr(p as *const c_char) }
            .to_string_lossy()
            .into_owned();
        zones.push((name, calls, total, max, main, worker));
    }
    zones.sort_by(|a, b| b.2.cmp(&a.2));

    let mut out = String::new();
    out.push_str("\n================ HOI4 RUST ACCEL v1.4.1 REPORT ================\n");
    out.push_str(&format!(
        "Measured Hours : {} ({:.2} game days)\n",
        measured_hours,
        measured_hours as f64 / 24.0
    ));
    out.push_str(&format!("Wall Time      : {:.3} s\n", wall_s));
    out.push_str(&format!(
        "Throughput     : {:.2} hours/s  |  {:.2} days/s  |  {:.2} ms/day\n",
        hours_per_sec, days_per_sec, ms_per_day
    ));
    out.push_str(&format!(
        "CPU Utilization: {:.2}s CPU in {:.2}s wall  =>  {:.2} active cores (of 16 logical)\n",
        cpu_s, wall_s, avg_active_cores
    ));
    let tf_ns = TARGET_FRAME_NS.load(Ordering::Relaxed);
    let tf_fps = if tf_ns > 0 { 1_000_000_000 / tf_ns } else { 0 };
    let sf_fps = SMOOTH_FPS.load(Ordering::Relaxed);
    out.push_str(&format!(
        "Config         : target_fps={} smooth_fps={} fast_alloc={} fast_mod={} country_grain={} civil_war_fix={} ai_smart_supply={} ai_capture_empty={} ai_smart_airbase={} ai_tactical_prudence={} ai_smart_naval={} ai_smart_armor={} ai_smart_peace={} ai_throttle_minors={} fast_sched={} fast_wait={} fast_yield={} fast_event={} no_job_clock={} affinity={} quiet_log={} fast_save={} no_telemetry={}\n",
        tf_fps,
        sf_fps,
        FAST_ALLOC_ENABLED.load(Ordering::Relaxed),
        FAST_MOD_ENABLED.load(Ordering::Relaxed),
        COUNTRY_GRAIN_ENABLED.load(Ordering::Relaxed),
        CIVIL_WAR_FIX_ENABLED.load(Ordering::Relaxed),
        AI_SMART_SUPPLY_ENABLED.load(Ordering::Relaxed),
        AI_CAPTURE_EMPTY_ENABLED.load(Ordering::Relaxed),
        AI_SMART_AIRBASE_ENABLED.load(Ordering::Relaxed),
        AI_TACTICAL_PRUDENCE_ENABLED.load(Ordering::Relaxed),
        AI_SMART_NAVAL_ENABLED.load(Ordering::Relaxed),
        AI_SMART_ARMOR_ENABLED.load(Ordering::Relaxed),
        AI_SMART_PEACE_ENABLED.load(Ordering::Relaxed),
        AI_THROTTLE_MINORS_ENABLED.load(Ordering::Relaxed),
        FAST_SCHED_ENABLED.load(Ordering::Relaxed),
        FAST_TASK_WAIT_ENABLED.load(Ordering::Relaxed),
        FAST_YIELD_ENABLED.load(Ordering::Relaxed),
        FAST_EVENT_ENABLED.load(Ordering::Relaxed),
        NO_JOB_CLOCK_ENABLED.load(Ordering::Relaxed),
        AFFINITY_ENABLED.load(Ordering::Relaxed),
        QUIET_LOG_ENABLED.load(Ordering::Relaxed),
        FAST_SAVE_ENABLED.load(Ordering::Relaxed),
        NO_TELEMETRY_ENABLED.load(Ordering::Relaxed),
    ));
    if ORIG_SESSION_UPDATE.load(Ordering::Relaxed) != 0 {
        let r = FRAMES_RENDERED.load(Ordering::Relaxed);
        let extra = BURST_EXTRA_TICKS.load(Ordering::Relaxed);
        let worst_ms = WORST_FRAME_NS.load(Ordering::Relaxed) as f64 / 1e6;
        let ema_r_ms = EMA_RENDER_NS.load(Ordering::Relaxed) as f64 / 1e6;
        out.push_str(&format!(
            "Frame Pacing   : {} frames ({:.1} FPS) | worst_frame={:.1} ms | avg_render={:.1} ms | extra_ticks={}\n",
            r,
            r as f64 / wall_s,
            worst_ms,
            ema_r_ms,
            extra
        ));
        let act_f = ACTIVE_SIM_FRAMES.load(Ordering::Relaxed);
        let act_ns = ACTIVE_SIM_NS.load(Ordering::Relaxed);
        if act_f > 0 && act_ns > 0 {
            let act_s = act_ns as f64 / 1e9;
            out.push_str(&format!(
                "Unpaused Sim   : {} frames in {:.2}s => {:.1} FPS | {:.2} days/s active\n",
                act_f,
                act_s,
                act_f as f64 / act_s,
                (measured_hours as f64 / 24.0) / act_s,
            ));
        }
    }
    if sf_fps > 0 {
        out.push_str(&format!(
            "Anti-Freeze    : smooth_fps={} | mid_tick_checks={} (fast_skipped={}) | mid_tick_renders={}\n",
            sf_fps,
            MID_TICK_CHECKS.load(Ordering::Relaxed),
            MID_TICK_FAST_SKIP.load(Ordering::Relaxed),
            MID_TICK_RENDERS.load(Ordering::Relaxed),
        ));
    }
    if FAST_SCHED_ENABLED.load(Ordering::Relaxed) {
        out.push_str(&format!(
            "FastSched      : {} zero-lock watcher polls | {} sched_yield syscalls elided\n",
            SCHED_WATCHER_POLLS.load(Ordering::Relaxed),
            YIELD_ELIDED.load(Ordering::Relaxed),
        ));
    }
    if QUIET_LOG_ENABLED.load(Ordering::Relaxed) {
        out.push_str(&format!(
            "QuietLog       : {} allowed | {} spam lines suppressed\n",
            LOG_ALLOWED.load(Ordering::Relaxed),
            LOG_SUPPRESSED.load(Ordering::Relaxed),
        ));
    }
    if FAST_SAVE_ENABLED.load(Ordering::Relaxed) {
        out.push_str(&format!(
            "FastSave       : {} blocking fsync() stalls eliminated\n",
            FSYNC_SKIPPED.load(Ordering::Relaxed),
        ));
    }
    if FAST_EVENT_ENABLED.load(Ordering::Relaxed) {
        out.push_str(&format!(
            "FastEvent      : {} spin-wakes | {} futex-wakes\n",
            FAST_WAKE_SPUN.load(Ordering::Relaxed),
            FAST_WAKE_FUTEX.load(Ordering::Relaxed)
        ));
    }
    if FAST_MOD_ENABLED.load(Ordering::Relaxed) {
        out.push_str(&format!(
            "FastModifier   : Zen-3 branchless active (verify_mismatches={})\n",
            VERIFY_MOD_MISMATCHES.load(Ordering::Relaxed)
        ));
    }
    if COUNTRY_GRAIN_ENABLED.load(Ordering::Relaxed) {
        out.push_str(&format!(
            "Country Grain  : {}/{} dispatches rebalanced (17..=220 -> chunk=1)\n",
            PARFOR_REBALANCED.load(Ordering::Relaxed),
            PARFOR_TOTAL.load(Ordering::Relaxed)
        ));
    }
    if FAST_ALLOC_ENABLED.load(Ordering::Relaxed) {
        let used_mb = SLAB_BUMP_OFF.load(Ordering::Relaxed) as f64 / (1024.0 * 1024.0);
        out.push_str(&format!(
            "FastAlloc      : {} slab allocs | {} slab frees | {} >512B fallbacks | {:.1}/128.0 MB arena carved\n",
            SLAB_ALLOC_HITS.load(Ordering::Relaxed),
            SLAB_FREE_HITS.load(Ordering::Relaxed),
            SLAB_FALLBACKS.load(Ordering::Relaxed),
            used_mb
        ));
    }
    if CIVIL_WAR_FIX_ENABLED.load(Ordering::Relaxed) {
        out.push_str(&format!(
            "CivilWarFix    : {} redundant mid-split map/supply rebuilds coalesced\n",
            CW_REBUILDS_COALESCED.load(Ordering::Relaxed)
        ));
    }
    if AI_SMART_SUPPLY_ENABLED.load(Ordering::Relaxed) {
        out.push_str(&format!(
            "SmartSupply    : {} wartime land supply_node hubs auto-queued (max 1 active/country, 2-hop BFS, >=12 fac)\n",
            AI_SUPPLY_HUBS_QUEUED.load(Ordering::Relaxed)
        ));
    }
    if AI_CAPTURE_EMPTY_ENABLED.load(Ordering::Relaxed) {
        out.push_str(&format!(
            "CaptureEmpty   : {} empty enemy provinces captured via native CMoveCommand (pocket-safe)\n",
            AI_EMPTY_TILES_CAPTURED.load(Ordering::Relaxed)
        ));
    }
    if AI_SMART_AIRBASE_ENABLED.load(Ordering::Relaxed) {
        out.push_str(&format!(
            "SmartAirbase   : {} queued air_base priority evaluations boosted (3.2x)\n",
            AIRBASE_PRIO_BOOSTS.load(Ordering::Relaxed)
        ));
    }
    if AI_TACTICAL_PRUDENCE_ENABLED.load(Ordering::Relaxed) {
        out.push_str(&format!(
            "TacticalAI     : Sheep-AI v4 prudence, anti-shuffle & empty-tile patch active (applied={}, empty_tile_patch={})\n",
            AI_TACTICAL_PRUDENCE_APPLIED.load(Ordering::Relaxed),
            EMPTY_TILE_FIX_APPLIED.load(Ordering::Relaxed)
        ));
    }
    if AI_SMART_NAVAL_ENABLED.load(Ordering::Relaxed) {
        out.push_str(&format!(
            "SmartNaval     : Anti-suicide invasions & Strike Force concentration active (applied={})\n",
            AI_SMART_NAVAL_APPLIED.load(Ordering::Relaxed)
        ));
    }
    if AI_SMART_ARMOR_ENABLED.load(Ordering::Relaxed) {
        out.push_str(&format!(
            "SmartArmor     : {} industrial-scaled armor/mech strategy queries adjusted (defines_applied={})\n",
            AI_ARMOR_STRATEGY_BOOSTS.load(Ordering::Relaxed),
            AI_SMART_ARMOR_DEFINES_APPLIED.load(Ordering::Relaxed)
        ));
    }
    if AI_SMART_PEACE_ENABLED.load(Ordering::Relaxed) {
        out.push_str(&format!(
            "SmartPeace     : {} peace conference state scores adjusted for border contiguity\n",
            PEACE_BORDERGORE_ADJUSTS.load(Ordering::Relaxed)
        ));
    }
    if AI_THROTTLE_MINORS_ENABLED.load(Ordering::Relaxed) {
        out.push_str(&format!(
            "MinorAIThrot   : {} peaceful minor sub-passes skipped | {} active AI passes executed\n",
            MINOR_AI_SKIPPED.load(Ordering::Relaxed),
            MINOR_AI_EXECUTED.load(Ordering::Relaxed)
        ));
    }

    out.push_str("\n--- Top Profiler Zones ---\n");
    out.push_str(&format!(
        "{:<44} {:>8} {:>9} {:>8} {:>9} {:>9} {:>7}\n",
        "Zone", "Calls", "Total ms", "Avg us", "Main ms", "Work ms", "%Wall"
    ));
    for (name, calls, total, _max, main, worker) in zones.iter().take(20) {
        let total_ms = *total as f64 / 1e6;
        let avg_us = (*total as f64 / *calls as f64) / 1e3;
        let main_ms = *main as f64 / 1e6;
        let work_ms = *worker as f64 / 1e6;
        let pct_wall = (main_ms / (wall_s * 1000.0)) * 100.0;
        let short_name: String = name.chars().take(44).collect();
        out.push_str(&format!(
            "{:<44} {:>8} {:>9.2} {:>8.1} {:>9.2} {:>9.2} {:>6.1}%\n",
            short_name, calls, total_ms, avg_us, main_ms, work_ms, pct_wall
        ));
    }

    if SAMPLE_ENABLED.load(Ordering::Relaxed) {
        let tm = TOTAL_SAMPLES_MAIN.load(Ordering::Relaxed);
        let tw = TOTAL_SAMPLES_WORK.load(Ordering::Relaxed);
        let mut main_hot: Vec<(usize, u64)> = Vec::new();
        let mut work_hot: Vec<(usize, u64)> = Vec::new();
        for s in &SAMPLES {
            let k = s.key.load(Ordering::Relaxed);
            if k == 0 {
                continue;
            }
            let mh = s.main_hits.load(Ordering::Relaxed);
            let wh = s.worker_hits.load(Ordering::Relaxed);
            if mh > 0 {
                main_hot.push((k, mh));
            }
            if wh > 0 {
                work_hot.push((k, wh));
            }
        }
        main_hot.sort_by(|a, b| b.1.cmp(&a.1));
        work_hot.sort_by(|a, b| b.1.cmp(&a.1));
        let format_sample_key = |k: usize| -> String {
            if (k & 0x8000_0000_0000_0000) == 0 {
                return format!("hoi4+{:#010x}", k);
            }
            let raw_rip = k & 0x7fff_ffff_ffff_ffff;
            let mut info = DlInfo {
                dli_fname: std::ptr::null(),
                dli_fbase: std::ptr::null_mut(),
                dli_sname: std::ptr::null(),
                dli_saddr: std::ptr::null_mut(),
            };
            if unsafe { dladdr(raw_rip as *const c_void, &mut info) } != 0
                && !info.dli_fname.is_null()
            {
                let full = unsafe { CStr::from_ptr(info.dli_fname) }.to_string_lossy();
                let lib = full.rsplit('/').next().unwrap_or(&full);
                if !info.dli_sname.is_null() {
                    let sym = unsafe { CStr::from_ptr(info.dli_sname) }.to_string_lossy();
                    let short_sym: String = sym.chars().take(28).collect();
                    return format!("[{}:{}]", lib, short_sym);
                }
                let foff = raw_rip.saturating_sub(info.dli_fbase as usize);
                return format!("[{}+{:#x}]", lib, foff);
            }
            format!("[EXT+{:#x}]", raw_rip)
        };
        out.push_str(&format!(
            "\n--- SIGPROF 250Hz Main-Thread Hotspots (total={}) ---\n",
            tm
        ));
        for (k, h) in main_hot.iter().take(15) {
            let pct = (*h as f64 / tm.max(1) as f64) * 100.0;
            let label = format_sample_key(*k);
            out.push_str(&format!("  {:<36} {:>6} hits ({:>5.1}%)\n", label, h, pct));
        }
        out.push_str(&format!(
            "\n--- SIGPROF 250Hz Worker-Thread Hotspots (total={}) ---\n",
            tw
        ));
        for (k, h) in work_hot.iter().take(15) {
            let pct = (*h as f64 / tw.max(1) as f64) * 100.0;
            let label = format_sample_key(*k);
            out.push_str(&format!("  {:<36} {:>6} hits ({:>5.1}%)\n", label, h, pct));
        }
    }

    let threads = read_per_thread_cpu();
    out.push_str(&format!(
        "\n--- Top 10 Threads (cumulative CPU, total threads={}) ---\n",
        threads.len()
    ));
    for (comm, tid, ticks) in threads.iter().take(10) {
        out.push_str(&format!(
            "  tid={:<7} {:<20} {:.2} s CPU\n",
            tid,
            comm,
            *ticks as f64 / 100.0
        ));
    }
    out.push_str("=============================================================\n");

    if let Ok(log_path) = std::env::var("HOI4_ACCEL_LOG") {
        if let Ok(mut f) = fs::OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&log_path)
        {
            let _ = f.write_all(out.as_bytes());
        }
    }
    eprint!("{}", out);
}

// ============================================================================
// Safe In-Memory Patching & Executable Trampoline Builder
// ============================================================================
unsafe fn patch_bytes(addr: usize, bytes: &[u8]) -> bool {
    let page_size = 4096usize;
    let start_page = addr & !(page_size - 1);
    let end_page = (addr + bytes.len() + page_size - 1) & !(page_size - 1);
    let len = end_page - start_page;

    unsafe {
        if mprotect(
            start_page as *mut c_void,
            len,
            PROT_READ | PROT_WRITE | PROT_EXEC,
        ) != 0
        {
            return false;
        }
        std::ptr::copy_nonoverlapping(bytes.as_ptr(), addr as *mut u8, bytes.len());
        let _ = mprotect(start_page as *mut c_void, len, PROT_READ | PROT_EXEC);
    }
    true
}

unsafe fn verify_sig(addr: usize, expected: &[u8]) -> bool {
    let slice = unsafe { std::slice::from_raw_parts(addr as *const u8, expected.len()) };
    slice == expected
}

unsafe fn install_abs_jmp(addr: usize, target: usize) -> bool {
    let mut patch = [0u8; 14];
    patch[0] = 0xff;
    patch[1] = 0x25;
    patch[2..6].copy_from_slice(&0u32.to_le_bytes());
    patch[6..14].copy_from_slice(&(target as u64).to_le_bytes());
    unsafe { patch_bytes(addr, &patch) }
}

unsafe fn install_trampoline_hook(
    func_addr: usize,
    expected_prologue: &[u8],
    hook_target: usize,
) -> Option<usize> {
    let plen = expected_prologue.len();
    assert!(plen >= 14);
    if !unsafe { verify_sig(func_addr, expected_prologue) } {
        return None;
    }

    let tramp = unsafe {
        mmap(
            std::ptr::null_mut(),
            4096,
            PROT_READ | PROT_WRITE | PROT_EXEC,
            MAP_PRIVATE | MAP_ANONYMOUS,
            -1,
            0,
        )
    };
    if tramp.is_null() || tramp as isize == -1 {
        return None;
    }

    let tramp_ptr = tramp as *mut u8;
    unsafe {
        std::ptr::copy_nonoverlapping(expected_prologue.as_ptr(), tramp_ptr, plen);
        let ret_addr = (func_addr + plen) as u64;
        let jmp = tramp_ptr.add(plen);
        *jmp.add(0) = 0xff;
        *jmp.add(1) = 0x25;
        std::ptr::copy_nonoverlapping(0u32.to_le_bytes().as_ptr(), jmp.add(2), 4);
        std::ptr::copy_nonoverlapping(ret_addr.to_le_bytes().as_ptr(), jmp.add(6), 8);
    }

    let mut patch = vec![0x90u8; plen];
    patch[0] = 0xff;
    patch[1] = 0x25;
    patch[2..6].copy_from_slice(&0u32.to_le_bytes());
    patch[6..14].copy_from_slice(&(hook_target as u64).to_le_bytes());

    if !unsafe { patch_bytes(func_addr, &patch) } {
        return None;
    }
    Some(tramp as usize)
}

unsafe extern "C" fn find_main_base_cb(
    info: *mut DlPhdrInfo,
    _size: usize,
    data: *mut c_void,
) -> c_int {
    let info = unsafe { &*info };
    if !info.dlpi_name.is_null() {
        let first = unsafe { *info.dlpi_name };
        if first == 0 {
            let out = unsafe { &mut *(data as *mut usize) };
            *out = info.dlpi_addr;
            return 1;
        }
    }
    0
}

fn find_pie_base() -> usize {
    let mut base: usize = 0;
    unsafe {
        dl_iterate_phdr(find_main_base_cb, (&mut base as *mut usize) as *mut c_void);
    }
    base
}

// ============================================================================
// Configuration (.ini + Env Vars) & Library Constructor (.init_array)
// ============================================================================
fn load_ini_map() -> Vec<(String, String)> {
    let mut candidates: Vec<std::path::PathBuf> = Vec::new();
    if let Ok(p) = std::env::var("HOI4_ACCEL_INI") {
        candidates.push(std::path::PathBuf::from(p));
    }
    candidates.push(std::path::PathBuf::from("hoi4_accel.ini"));

    let mut info = DlInfo {
        dli_fname: std::ptr::null(),
        dli_fbase: std::ptr::null_mut(),
        dli_sname: std::ptr::null(),
        dli_saddr: std::ptr::null_mut(),
    };
    if unsafe { dladdr(hoi4_accel_init as *const c_void, &mut info) } != 0
        && !info.dli_fname.is_null()
    {
        let so_str = unsafe { CStr::from_ptr(info.dli_fname) }.to_string_lossy();
        let so_path = std::path::Path::new(so_str.as_ref());
        if let Some(dir) = so_path.parent() {
            candidates.push(dir.join("hoi4_accel.ini"));
            if let Some(p1) = dir.parent() {
                if let Some(p2) = p1.parent() {
                    candidates.push(p2.join("hoi4_accel.ini"));
                }
            }
        }
    }

    for path in candidates {
        if let Ok(txt) = fs::read_to_string(&path) {
            let mut pairs = Vec::new();
            for raw_line in txt.lines() {
                let no_comment = raw_line
                    .split([';', '#'])
                    .next()
                    .unwrap_or("")
                    .trim();
                if no_comment.is_empty() || no_comment.starts_with('[') {
                    continue;
                }
                if let Some((k, v)) = no_comment.split_once('=') {
                    let key = k.trim().to_ascii_lowercase();
                    let val = v.trim().to_string();
                    if !key.is_empty() && !val.is_empty() {
                        pairs.push((key, val));
                    }
                }
            }
            return pairs;
        }
    }
    Vec::new()
}

unsafe extern "C" fn hoi4_accel_init() {
    let exe = match fs::read_link("/proc/self/exe") {
        Ok(p) => p,
        Err(_) => return,
    };
    if exe.file_name().and_then(|s| s.to_str()) != Some("hoi4") {
        return;
    }

    ACTIVE_FOR_HOI4.store(true, Ordering::Release);
    MAIN_TID.store(current_tid(), Ordering::Relaxed);
    MAIN_PTHREAD.store(unsafe { pthread_self() }, Ordering::Relaxed);

    let base = find_pie_base();
    PIE_BASE.store(base, Ordering::Release);

    let ini_pairs = load_ini_map();
    let cfg_str = |env_k: &str, ini_k: &str| -> Option<String> {
        if let Ok(v) = std::env::var(env_k) {
            if !v.trim().is_empty() {
                return Some(v.trim().to_string());
            }
        }
        for (k, v) in &ini_pairs {
            if k == ini_k {
                return Some(v.clone());
            }
        }
        None
    };
    let cfg_bool = |env_k: &str, ini_k: &str, default: bool| -> bool {
        match cfg_str(env_k, ini_k)
            .map(|s| s.to_ascii_lowercase())
            .as_deref()
        {
            Some("1") | Some("true") | Some("on") => true,
            Some("0") | Some("false") | Some("off") => false,
            _ => default,
        }
    };
    let cfg_u64 = |env_k: &str, ini_k: &str, default: u64| -> u64 {
        cfg_str(env_k, ini_k)
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(default)
    };

    let mut need_scoped_timer = false;
    let light_mode = std::env::var("HOI4_BENCH_LIGHT").as_deref() == Ok("1");
    let noop_mode = cfg_bool("HOI4_ACCEL_NOOP", "noop", false);
    let raw_mode = std::env::var("HOI4_ACCEL_RAW").as_deref() == Ok("1");

    // Resolve preset defaults unless HOI4_ACCEL_NOOP=1 or HOI4_ACCEL_RAW=1
    let preset = cfg_str("HOI4_ACCEL_PRESET", "preset")
        .map(|s| s.to_ascii_lowercase())
        .unwrap_or_else(|| {
            if noop_mode || raw_mode {
                "none".into()
            } else {
                "smooth".into()
            }
        });

    eprintln!(
        "[hoi4-rust-accel v1.4.1] Attached to hoi4 (pid={}, pie_base={:#x}, preset={})",
        std::process::id(),
        base,
        preset
    );

    // (target_fps_burst, smooth_midtick_fps, core_opts_enabled)
    let (def_fps, def_smooth, def_core): (u64, u64, bool) = match preset.as_str() {
        "smooth" => (26, 30, true),
        "balanced" => (14, 14, true),
        "turbo" => (6, 6, true),
        "pure" => (0, 30, true),
        _ => (0, 0, false),
    };
    let (def_fast_ev, def_fast_sched, def_no_clk, def_grain, def_aff, def_qlog, def_fsave, def_notel) = (
        def_core, def_core, def_core, def_core, def_core, def_core, def_core, def_core,
    );
    let want_ai_smart_supply = !noop_mode && cfg_bool("HOI4_ACCEL_AI_SMART_SUPPLY", "ai_smart_supply", def_core);
    let want_ai_capture_empty =
        !noop_mode && cfg_bool("HOI4_ACCEL_AI_CAPTURE_EMPTY", "ai_capture_empty", def_core);
    if want_ai_smart_supply {
        AI_SMART_SUPPLY_ENABLED.store(true, Ordering::Release);
    }
    if want_ai_capture_empty {
        AI_CAPTURE_EMPTY_ENABLED.store(true, Ordering::Release);
    }
    if want_ai_smart_supply || want_ai_capture_empty {
        if let Some(tramp_sb) = unsafe {
            install_trampoline_hook(
                base + OFF_FIX_SUPPLY_BOTTLENECKS,
                &SIG_FIX_SUPPLY_BOTTLENECKS,
                hook_fix_supply_bottlenecks as *const () as usize,
            )
        } {
            ORIG_FIX_SUPPLY_BOTTLENECKS.store(tramp_sb, Ordering::Release);
            eprintln!(
                "[hoi4-rust-accel] Enabled Daily AI Logistics & Empty-Tile Capture Hook (FixSupplyBottlenecks @ 0x742d40, supply_hub={}, capture_empty={})",
                want_ai_smart_supply, want_ai_capture_empty
            );
        }
    }
    let want_ai_smart_naval =
        !noop_mode && cfg_bool("HOI4_ACCEL_AI_SMART_NAVAL", "ai_smart_naval", def_core);
    if want_ai_smart_naval {
        AI_SMART_NAVAL_ENABLED.store(true, Ordering::Release);
    }
    let mut want_ai_smart_airbase =
        !noop_mode && cfg_bool("HOI4_ACCEL_AI_SMART_AIRBASE", "ai_smart_airbase", def_core);
    if want_ai_smart_airbase {
        if let Some(tramp_ab) = unsafe {
            install_trampoline_hook(
                base + OFF_GET_BLDG_QUEUE_PRIO,
                &SIG_GET_BLDG_QUEUE_PRIO,
                hook_get_bldg_queue_prio as *const () as usize,
            )
        } {
            ORIG_GET_BLDG_QUEUE_PRIO.store(tramp_ab, Ordering::Release);
            AI_SMART_AIRBASE_ENABLED.store(true, Ordering::Release);
            eprintln!(
                "[hoi4-rust-accel] Enabled AiSmartAirbase (CAIConstruction::GetBuildingQueuePrio @ 0x74f4d0)"
            );
        } else {
            want_ai_smart_airbase = false;
        }
    }
    let want_ai_tactical_prudence = !noop_mode
        && cfg_bool(
            "HOI4_ACCEL_AI_TACTICAL_PRUDENCE",
            "ai_tactical_prudence",
            def_core,
        );
    if want_ai_tactical_prudence {
        AI_TACTICAL_PRUDENCE_ENABLED.store(true, Ordering::Release);
        unsafe {
            if verify_sig(base + OFF_EMPTY_TILE_SIG, &SIG_EMPTY_TILE_BRANCH)
                && patch_bytes(base + OFF_EMPTY_TILE_PATCH, &[0x90, 0x90])
            {
                EMPTY_TILE_FIX_APPLIED.store(true, Ordering::Release);
                eprintln!(
                    "[hoi4-rust-accel] Enabled Empty-Tile Stall Fix (CUnitOrder @ 0x2569860 -> NOP; CanMoveIntoProvince intact)"
                );
            }
        }
    }
    let mut want_ai_smart_armor =
        !noop_mode && cfg_bool("HOI4_ACCEL_AI_SMART_ARMOR", "ai_smart_armor", def_core);
    if want_ai_smart_armor {
        unsafe {
            let strat_slot = base + OFF_STRATEGY_STORE_VT + OFF_STRATEGY_GET_VAL_SLOT;
            let cur_strat = *(strat_slot as *const usize);
            if cur_strat == base + OFF_STRATEGY_GET_VAL_FN {
                ORIG_GET_STRATEGY_VALUE.store(cur_strat, Ordering::Release);
                if patch_rodata_ptr(strat_slot, hook_get_strategy_value as *const () as usize) {
                    AI_SMART_ARMOR_ENABLED.store(true, Ordering::Release);
                    eprintln!(
                        "[hoi4-rust-accel] Enabled AiSmartArmor (CAIStrategyStore::GetStrategyValue vtable @ 0x434ea90+0x28)"
                    );
                } else {
                    want_ai_smart_armor = false;
                }
            } else {
                want_ai_smart_armor = false;
            }
        }
    }
    let mut want_ai_smart_peace =
        !noop_mode && cfg_bool("HOI4_ACCEL_AI_SMART_PEACE", "ai_smart_peace", def_core);
    if want_ai_smart_peace {
        if unsafe { verify_sig(base + OFF_PEACE_IS_STATE_CLAIMED, &SIG_PEACE_IS_STATE_CLAIMED) } {
            if let Some(tramp_pc) = unsafe {
                install_trampoline_hook(
                    base + OFF_PEACE_CALC_STATE_SCORE,
                    &SIG_PEACE_CALC_STATE_SCORE,
                    hook_peace_calc_state_score as *const () as usize,
                )
            } {
                ORIG_PEACE_CALC_STATE_SCORE.store(tramp_pc, Ordering::Release);
                AI_SMART_PEACE_ENABLED.store(true, Ordering::Release);
                eprintln!(
                    "[hoi4-rust-accel] Enabled AiSmartPeace (anti-bordergore CPeaceWinnerAi::CalcStateActionScore @ 0xeafef0)"
                );
            } else {
                want_ai_smart_peace = false;
            }
        } else {
            want_ai_smart_peace = false;
        }
    }

    let bench_hrs = cfg_u64("HOI4_BENCH_HOURS", "bench_hours", 0);
    if bench_hrs > 0 {
        BENCH_MAX_HOURS.store(bench_hrs, Ordering::Relaxed);
        BENCH_WARMUP_HOURS.store(cfg_u64("HOI4_BENCH_WARMUP", "bench_warmup", 24), Ordering::Relaxed);
        COLLECT_ZONES.store(true, Ordering::Relaxed);
        need_scoped_timer = true;
    }
    if cfg_bool("HOI4_ACCEL_PROFILE", "profile", false) {
        COLLECT_ZONES.store(true, Ordering::Relaxed);
        BENCH_START_NS.store(now_ns(), Ordering::Relaxed);
        BENCH_START_CPU_NS.store(now_cpu_ns(), Ordering::Relaxed);
        BENCH_RECORDING.store(true, Ordering::Release);
        need_scoped_timer = true;
    }

    // Breakthrough 1: Render-Aware Multi-Tick Speed-5 Bursting (also drives lazy AI NDefines init)
    let target_fps = if noop_mode {
        0
    } else {
        cfg_u64("HOI4_ACCEL_TARGET_FPS", "target_fps", def_fps)
    };
    let need_session_hook = (target_fps >= 1 && target_fps <= 240)
        || BENCH_MAX_HOURS.load(Ordering::Relaxed) > 0
        || COLLECT_ZONES.load(Ordering::Relaxed)
        || want_ai_smart_supply
        || want_ai_capture_empty
        || want_ai_smart_airbase
        || want_ai_tactical_prudence
        || want_ai_smart_naval
        || want_ai_smart_armor
        || want_ai_smart_peace;
    if need_session_hook {
        let frame_ns = if target_fps >= 1 && target_fps <= 240 {
            1_000_000_000 / target_fps
        } else {
            0
        };
        let max_burst = cfg_u64("HOI4_ACCEL_MAX_BURST", "max_burst", 16).clamp(1, 64);
        MAX_BURST_TICKS.store(max_burst, Ordering::Relaxed);
        if unsafe { verify_sig(base + OFF_GAMESTATE_TICK_CLOCK, &SIG_TICK_CLOCK) } {
            if let Some(tramp) = unsafe {
                install_trampoline_hook(
                    base + OFF_SESSION_UPDATE,
                    &SIG_SESSION_UPDATE,
                    hook_session_update as *const () as usize,
                )
            } {
                ORIG_SESSION_UPDATE.store(tramp, Ordering::Release);
                TARGET_FRAME_NS.store(frame_ns, Ordering::Release);
                if frame_ns > 0 {
                    need_scoped_timer = true;
                    eprintln!(
                        "[hoi4-rust-accel] Enabled Render-Aware Speed-5 Bursting (preset={}, target={} FPS, max_burst={})",
                        preset, target_fps, max_burst
                    );
                }
            }
        }
    }

    // Breakthrough 4: Anti-Freeze Mid-Tick Frame Pacer (ShouldRenderMidTick @ 0x20b07a0)
    let smooth_fps = if noop_mode {
        0
    } else {
        cfg_u64("HOI4_ACCEL_SMOOTH_FPS", "smooth_fps", def_smooth)
    };
    if smooth_fps >= 1 && smooth_fps <= 240 {
        unsafe {
            if verify_sig(base + OFF_SHOULD_RENDER_MIDTICK, &SIG_SHOULD_RENDER_MIDTICK)
                && install_abs_jmp(
                    base + OFF_SHOULD_RENDER_MIDTICK,
                    hook_should_render_mid_tick as *const () as usize,
                )
            {
                SMOOTH_FPS.store(smooth_fps, Ordering::Release);
                eprintln!(
                    "[hoi4-rust-accel] Enabled Mid-Tick Anti-Freeze Frame Pacer (smooth={} FPS, camera=60 FPS)",
                    smooth_fps
                );
            }
        }
    }

    // Breakthrough 2: Pure Rust Zen-3 Branchless CModifier::GetValue (enabled in core presets in v1.0)
    if !noop_mode && cfg_bool("HOI4_ACCEL_FAST_MOD", "fast_mod", def_core) {
        if let Ok(v) = std::env::var("HOI4_ACCEL_VERIFY_MOD") {
            if let Ok(n) = v.parse::<u64>() {
                VERIFY_MOD_REMAINING.store(n, Ordering::Relaxed);
            }
        }
        let mod_hook = if VERIFY_MOD_REMAINING.load(Ordering::Relaxed) > 0 {
            hook_modifier_get_value_verify as *const () as usize
        } else {
            hook_modifier_get_value_pure as *const () as usize
        };
        if let Some(tramp) = unsafe {
            install_trampoline_hook(
                base + OFF_MODIFIER_GET_VAL,
                &SIG_MODIFIER_GET_VAL,
                mod_hook,
            )
        } {
            ORIG_MODIFIER_GET_VAL.store(tramp, Ordering::Release);
            FAST_MOD_ENABLED.store(true, Ordering::Release);
            eprintln!(
                "[hoi4-rust-accel] Replaced CModifier::GetValue (0x2241b00) with Rust branchless CMOV binary search"
            );
        }
    }

    // Breakthrough 3: Adaptive Country-Loop Grain
    if !noop_mode && cfg_bool("HOI4_ACCEL_COUNTRY_GRAIN", "country_grain", def_grain) {
        if let Some(tramp) = unsafe {
            install_trampoline_hook(
                base + OFF_PARFOR_RANGE_INIT,
                &SIG_PARFOR_RANGE,
                hook_parfor_range_init as *const () as usize,
            )
        } {
            ORIG_PARFOR_RANGE_INIT.store(tramp, Ordering::Release);
            COUNTRY_GRAIN_ENABLED.store(true, Ordering::Release);
            eprintln!(
                "[hoi4-rust-accel] Enabled Adaptive Country-Loop Grain (17..=220 -> chunk=1)"
            );
        }
    }

    // v1.0 Module 11: Lock-Free Slab + TLS Magazine Allocator (CPdxNewDeleteAllocator @ 0x4349510)
    if !noop_mode && cfg_bool("HOI4_ACCEL_FAST_ALLOC", "fast_alloc", def_core) {
        unsafe {
            let alloc_slot = base + OFF_PDX_ALLOC_VT + OFF_PDX_ALLOC_SLOT;
            let free_slot = base + OFF_PDX_ALLOC_VT + OFF_PDX_FREE_SLOT;
            let cur_alloc = *(alloc_slot as *const usize);
            let cur_free = *(free_slot as *const usize);
            if cur_alloc == base + OFF_PDX_ALLOC_FN
                && cur_free == base + OFF_PDX_FREE_FN
                && init_slab_arena()
            {
                ORIG_PDX_ALLOC.store(cur_alloc, Ordering::Release);
                ORIG_PDX_FREE.store(cur_free, Ordering::Release);
                if patch_rodata_ptr(alloc_slot, hook_pdx_alloc as *const () as usize)
                    && patch_rodata_ptr(free_slot, hook_pdx_free as *const () as usize)
                {
                    FAST_ALLOC_ENABLED.store(true, Ordering::Release);
                    eprintln!(
                        "[hoi4-rust-accel] Enabled FastAlloc (128 MB lock-free slab + TLS magazine on CPdxNewDeleteAllocator)"
                    );
                }
            }
        }
    }

    // v1.0 Module 12: Civil War Freeze Fix (CCivilWarSetup::Execute + CInGameIdler map rebuild coalescer)
    if !noop_mode && cfg_bool("HOI4_ACCEL_CIVIL_WAR_FIX", "civil_war_fix", def_core) {
        unsafe {
            let idler_slot = base + OFF_INGAME_IDLER_VT + OFF_IDLER_MAP_REBUILD_SLOT;
            let cur_rebuild = *(idler_slot as *const usize);
            if cur_rebuild == base + OFF_IDLER_MAP_REBUILD_FN {
                if let Some(tramp_cw) = install_trampoline_hook(
                    base + OFF_CW_SETUP_EXEC,
                    &SIG_CW_SETUP_EXEC,
                    hook_cw_setup_exec as *const () as usize,
                ) {
                    ORIG_CW_SETUP_EXEC.store(tramp_cw, Ordering::Release);
                    ORIG_IDLER_MAP_REBUILD.store(cur_rebuild, Ordering::Release);
                    if patch_rodata_ptr(idler_slot, hook_idler_map_rebuild as *const () as usize) {
                        CIVIL_WAR_FIX_ENABLED.store(true, Ordering::Release);
                        eprintln!(
                            "[hoi4-rust-accel] Enabled CivilWarFix (coalesced per-state CInGameIdler map/supply rebuilds)"
                        );
                    }
                }
            }
        }
    }

    // v1.0 Module 14: Dormant Peaceful Minor AI Hourly Throttling (CCountryAI @ 0x434b3b8)
    if !noop_mode && cfg_bool("HOI4_ACCEL_AI_THROTTLE_MINORS", "ai_throttle_minors", def_core) {
        unsafe {
            let p1_slot = base + OFF_COUNTRY_AI_VT + OFF_COUNTRY_AI_PASS1_SLOT;
            let p2_slot = base + OFF_COUNTRY_AI_VT + OFF_COUNTRY_AI_PASS2_SLOT;
            let cur_p1 = *(p1_slot as *const usize);
            let cur_p2 = *(p2_slot as *const usize);
            if cur_p1 == base + OFF_COUNTRY_AI_PASS1_FN && cur_p2 == base + OFF_COUNTRY_AI_PASS2_FN {
                ORIG_COUNTRY_AI_PASS1.store(cur_p1, Ordering::Release);
                ORIG_COUNTRY_AI_PASS2.store(cur_p2, Ordering::Release);
                if patch_rodata_ptr(p1_slot, hook_country_ai_pass1 as *const () as usize)
                    && patch_rodata_ptr(p2_slot, hook_country_ai_pass2 as *const () as usize)
                {
                    AI_THROTTLE_MINORS_ENABLED.store(true, Ordering::Release);
                    eprintln!(
                        "[hoi4-rust-accel] Enabled AiThrottleMinors (staggered 4h cadence strictly for peaceful <12-state non-major AI minors)"
                    );
                }
            }
        }
    }

    let env_bool = |env_k: &str, ini_k: &str, default: bool| -> bool {
        cfg_bool(env_k, ini_k, default)
    };

    if !noop_mode {
        if let Some(v) = cfg_str("HOI4_ACCEL_THREADS", "threads") {
            if let Ok(n) = v.parse::<i32>() {
                if n >= 2 {
                    OVERRIDE_THREADS.store(n, Ordering::Relaxed);
                }
            }
        }
        if let Some(v) = cfg_str("HOI4_ACCEL_SPIN", "spin") {
            if let Ok(n) = v.parse::<u32>() {
                SPIN_ITERS.store(n, Ordering::Relaxed);
            }
        }
        if env_bool("HOI4_ACCEL_AFFINITY", "affinity", def_aff) {
            init_cpu_topology();
            AFFINITY_ENABLED.store(true, Ordering::Release);
            let _ = pin_current_thread_to_cpu(0);
            eprintln!(
                "[hoi4-rust-accel] Enabled topology-aware Zen-3 core pinning (main=CPU0, {} worker slots, {} bg slots)",
                TOPO_WORKER_LEN.load(Ordering::Relaxed),
                TOPO_BG_LEN.load(Ordering::Relaxed)
            );
        }
    }

    if need_scoped_timer {
        let mut ok_count = 0;
        unsafe {
            if verify_sig(base + OFF_SCOPED_TIMER_CTOR, &SIG_PUSH_RBP_4157_4156)
                && verify_sig(base + OFF_SCOPED_TIMER_DTOR, &SIG_DTOR_5350_4883)
            {
                if install_abs_jmp(
                    base + OFF_SCOPED_TIMER_CTOR,
                    hook_scoped_timer_ctor as *const () as usize,
                ) && install_abs_jmp(
                    base + OFF_SCOPED_TIMER_DTOR,
                    hook_scoped_timer_dtor as *const () as usize,
                ) {
                    ok_count += 2;
                }
            }
            if COLLECT_ZONES.load(Ordering::Relaxed)
                && !light_mode
                && verify_sig(base + OFF_THREAD_PROF_CTOR, &SIG_PUSH_RBP_4157_4156)
                && verify_sig(base + OFF_THREAD_PROF_DTOR, &SIG_DTOR_5350_4883)
            {
                if install_abs_jmp(
                    base + OFF_THREAD_PROF_CTOR,
                    hook_thread_prof_ctor as *const () as usize,
                ) && install_abs_jmp(
                    base + OFF_THREAD_PROF_DTOR,
                    hook_thread_prof_dtor as *const () as usize,
                ) {
                    ok_count += 2;
                }
            }
        }
        eprintln!(
            "[hoi4-rust-accel] Timer hooks: {} installed (light={})",
            ok_count, light_mode
        );
    }

    if std::env::var("HOI4_ACCEL_SAMPLE").as_deref() == Ok("1") {
        SAMPLE_ENABLED.store(true, Ordering::Relaxed);
        install_sigprof_sampler();
        eprintln!("[hoi4-rust-accel] Enabled 250Hz SIGPROF RIP sampler");
    }

    let req_threads = OVERRIDE_THREADS.load(Ordering::Relaxed);
    if req_threads >= 2 {
        unsafe {
            if verify_sig(base + OFF_HW_CONC_MIN4, &SIG_HW_CONC_MIN4)
                && install_abs_jmp(
                    base + OFF_HW_CONC_MIN4,
                    hook_hw_conc_min4 as *const () as usize,
                )
            {
                eprintln!(
                    "[hoi4-rust-accel] Overrode hw_conc_min4() -> {} (~{} workers)",
                    req_threads,
                    req_threads - 1
                );
            }
        }
    }

    if !noop_mode && env_bool("HOI4_ACCEL_FAST_EVENT", "fast_event", def_fast_ev) {
        let mut ok = 0;
        unsafe {
            let targets: [(usize, &[u8], usize); 8] = [
                (
                    OFF_WAIT_AUTO1,
                    &SIG_PUSH_RBP_4157_4156,
                    hook_event_wait_auto as *const () as usize,
                ),
                (
                    OFF_WAIT_AUTO2,
                    &SIG_PUSH_RBP_4157_4156,
                    hook_event_wait_auto as *const () as usize,
                ),
                (
                    OFF_WAKE_N,
                    &SIG_PUSH_RBP_4157_4156,
                    hook_pool_wake_n as *const () as usize,
                ),
                (
                    OFF_WAKE_ONE,
                    &SIG_PUSH_RBP_4157_4156,
                    hook_pool_wake_one_or_all as *const () as usize,
                ),
                (
                    OFF_SIGNAL,
                    &SIG_SIGNAL_ISSIG,
                    hook_event_signal as *const () as usize,
                ),
                (
                    OFF_WAIT_MAN,
                    &SIG_PUSH_RBP_4157_4156,
                    hook_event_wait_manual as *const () as usize,
                ),
                (
                    OFF_IS_SIG,
                    &SIG_SIGNAL_ISSIG,
                    hook_event_is_signaled as *const () as usize,
                ),
                (
                    OFF_RESET,
                    &SIG_RESET,
                    hook_event_reset as *const () as usize,
                ),
            ];
            for (off, sig, dst) in targets {
                if verify_sig(base + off, sig) && install_abs_jmp(base + off, dst) {
                    ok += 1;
                }
            }
        }
        if ok == 8 {
            FAST_EVENT_ENABLED.store(true, Ordering::Release);
            eprintln!(
                "[hoi4-rust-accel] Installed 8/8 lock-free Rust futex scheduler hooks (spin={})",
                SPIN_ITERS.load(Ordering::Relaxed)
            );
        }
    }

    // Breakthrough 8: Lock-Free Zero-Bounce Rust Worker Scheduler (PopTaskOrWait @ 0x3666d70)
    if !noop_mode && env_bool("HOI4_ACCEL_FAST_SCHED", "fast_sched", def_fast_sched) {
        unsafe {
            if verify_sig(base + OFF_POP_TASK_OR_WAIT, &SIG_POP_TASK_OR_WAIT)
                && install_abs_jmp(
                    base + OFF_POP_TASK_OR_WAIT,
                    hook_pop_task_or_wait as *const () as usize,
                )
            {
                FAST_SCHED_ENABLED.store(true, Ordering::Release);
                eprintln!(
                    "[hoi4-rust-accel] Replaced PopTaskOrWait (0x3666d70) with lock-free zero-bounce Rust scheduler"
                );
            }
        }
    }

    // Breakthrough 9: Lock-Free FastTaskWait (CTask::Wait @ 0x3662b10)
    if !noop_mode && env_bool("HOI4_ACCEL_FAST_WAIT", "fast_wait", def_fast_sched) {
        unsafe {
            if verify_sig(base + OFF_TASK_WAIT, &SIG_TASK_WAIT)
                && install_abs_jmp(
                    base + OFF_TASK_WAIT,
                    hook_task_wait as *const () as usize,
                )
            {
                FAST_TASK_WAIT_ENABLED.store(true, Ordering::Release);
                eprintln!(
                    "[hoi4-rust-accel] Replaced CTask::Wait (0x3662b10) — eliminated sched_yield() trap loop"
                );
            }
        }
    }

    // Breakthrough 10: Elide hoi4 internal sched_yield@plt spin-wait kernel traps (0x20ff554 + 5 clones)
    if !noop_mode && env_bool("HOI4_ACCEL_FAST_YIELD", "fast_yield", def_fast_sched) {
        unsafe {
            if verify_sig(base + OFF_SCHED_YIELD_PLT, &SIG_SCHED_YIELD_PLT)
                && install_abs_jmp(
                    base + OFF_SCHED_YIELD_PLT,
                    hook_hoi4_sched_yield as *const () as usize,
                )
            {
                FAST_YIELD_ENABLED.store(true, Ordering::Release);
                eprintln!(
                    "[hoi4-rust-accel] Patched hoi4 sched_yield@plt (0x5140d0) — eliminated 8 spin-wait syscall loops"
                );
            }
        }
    }

    if !noop_mode && env_bool("HOI4_ACCEL_NO_JOB_CLOCK", "no_job_clock", def_no_clk) {
        unsafe {
            if verify_sig(base + OFF_EXEC_TASK_NOW1, &SIG_EXEC_TASK_NOW1)
                && verify_sig(base + OFF_EXEC_TASK_NOW2, &SIG_EXEC_TASK_NOW2)
            {
                let nop5 = [0x90u8; 5];
                let nop41 = [0x90u8; 41];
                if patch_bytes(base + OFF_EXEC_TASK_NOW1, &nop5)
                    && patch_bytes(base + OFF_EXEC_TASK_NOW2, &nop41)
                {
                    NO_JOB_CLOCK_ENABLED.store(true, Ordering::Release);
                    eprintln!(
                        "[hoi4-rust-accel] Patched ExecuteTask per-job clock_gettime + lock add"
                    );
                }
            }
        }
    }

    // Breakthrough 5: Lock-Free error.log Spam Deduplicator
    if !noop_mode && env_bool("HOI4_ACCEL_QUIET_LOG", "quiet_log", def_qlog) {
        unsafe {
            if verify_sig(base + OFF_SET_LOGGER, &SIG_SET_LOGGER)
                && install_abs_jmp(base + OFF_SET_LOGGER, hook_set_logger as *const () as usize)
            {
                // Wrap any logger already installed before our constructor
                let cur_logger = *((base + OFF_G_LOGGER_PTR) as *const *mut u8);
                if !cur_logger.is_null() {
                    wrap_logger_vtable(cur_logger);
                }
                QUIET_LOG_ENABLED.store(true, Ordering::Release);
                eprintln!(
                    "[hoi4-rust-accel] Enabled lock-free error.log spam deduplicator (max 8 per error pattern)"
                );
            }
        }
    }

    // Breakthrough 6: Fast Save (skip blocking synchronous fsync @ 0x3672b60)
    if !noop_mode && env_bool("HOI4_ACCEL_FAST_SAVE", "fast_save", def_fsave) {
        unsafe {
            if verify_sig(base + OFF_FILE_FSYNC, &SIG_FILE_FSYNC)
                && install_abs_jmp(base + OFF_FILE_FSYNC, hook_fast_fsync as *const () as usize)
            {
                FAST_SAVE_ENABLED.store(true, Ordering::Release);
                eprintln!(
                    "[hoi4-rust-accel] Enabled FastSave (eliminated blocking synchronous fsync stalls)"
                );
            }
        }
    }

    // Breakthrough 7: Disable Paradox SDK Telemetry cleanly
    if !noop_mode && env_bool("HOI4_ACCEL_NO_TELEMETRY", "no_telemetry", def_notel) {
        unsafe {
            if verify_sig(base + OFF_TELEMETRY_CALL, &SIG_TELEMETRY_CALL)
                && patch_bytes(base + OFF_TELEMETRY_CALL, &[0x90u8; 5])
            {
                NO_TELEMETRY_ENABLED.store(true, Ordering::Release);
                eprintln!("[hoi4-rust-accel] Disabled Paradox SDK telemetry dispatch");
            }
        }
    }
}

unsafe extern "C" fn hoi4_accel_fini() {
    if !ACTIVE_FOR_HOI4.load(Ordering::Relaxed) {
        return;
    }
    if BENCH_MAX_HOURS.load(Ordering::Relaxed) == 0 && COLLECT_ZONES.load(Ordering::Relaxed) {
        let h = TOTAL_HOURS.load(Ordering::Relaxed);
        if h > 0 && !BENCH_FINISHED.swap(true, Ordering::AcqRel) {
            BENCH_RECORDING.store(false, Ordering::Release);
            emit_report(h);
        }
    }
}

#[used]
#[unsafe(link_section = ".init_array")]
static INIT_ARRAY: unsafe extern "C" fn() = hoi4_accel_init;

#[used]
#[unsafe(link_section = ".fini_array")]
static FINI_ARRAY: unsafe extern "C" fn() = hoi4_accel_fini;
