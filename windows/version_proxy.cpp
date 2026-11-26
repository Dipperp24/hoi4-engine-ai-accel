// ============================================================================
// hoi4-accel v1.3-win64: Non-Destructive Windows x86_64 Engine & AI Accelerator
// Target: Hearts of Iron IV v1.20.0.0.8873 (Operation Postern, PE32+ x86-64)
//
// Drop `version.dll` (and optional `hoi4_accel.ini`) next to `hoi4.exe`.
// Zero game files on disk are modified; non-HoI4 processes (such as dowser.exe
// or crashreporter.exe) strictly pass through untouched to System32\version.dll.
// ============================================================================

#ifndef WIN32_LEAN_AND_MEAN
#define WIN32_LEAN_AND_MEAN
#endif
#ifndef NOMINMAX
#define NOMINMAX
#endif

#include <windows.h>
#include <intrin.h>

#include <atomic>
#include <cctype>
#include <cstdint>
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <cwchar>

// ============================================================================
// 1. Transparent System32\version.dll Export Forwarding (All 19 Win32 Exports)
// ============================================================================
extern "C" {
FARPROC g_ver_procs[19] = {nullptr};
}

static HMODULE g_real_version_dll = nullptr;

static const char* const kVersionExportNames[19] = {
    "GetFileVersionInfoA",
    "GetFileVersionInfoByHandle",
    "GetFileVersionInfoExA",
    "GetFileVersionInfoExW",
    "GetFileVersionInfoSizeA",
    "GetFileVersionInfoSizeExA",
    "GetFileVersionInfoSizeExW",
    "GetFileVersionInfoSizeW",
    "GetFileVersionInfoW",
    "VerFindFileA",
    "VerFindFileW",
    "VerInstallFileA",
    "VerInstallFileW",
    "VerLanguageNameA",
    "VerLanguageNameW",
    "VerQueryValueA",
    "VerQueryValueIndexA",
    "VerQueryValueIndexW",
    "VerQueryValueW",
};

#define DEFINE_VERSION_THUNK(idx, name)                               \
    __asm__(                                                          \
        ".text\n\t"                                                   \
        ".p2align 4\n\t"                                              \
        ".globl proxy_" #name "\n"                                    \
        "proxy_" #name ":\n\t"                                        \
        "movq g_ver_procs+" #idx "*8(%rip), %rax\n\t"                 \
        "testq %rax, %rax\n\t"                                        \
        "jz 1f\n\t"                                                   \
        "jmp *%rax\n"                                                 \
        "1:\n\t"                                                      \
        "xorl %eax, %eax\n\t"                                         \
        "ret\n");

DEFINE_VERSION_THUNK(0, GetFileVersionInfoA)
DEFINE_VERSION_THUNK(1, GetFileVersionInfoByHandle)
DEFINE_VERSION_THUNK(2, GetFileVersionInfoExA)
DEFINE_VERSION_THUNK(3, GetFileVersionInfoExW)
DEFINE_VERSION_THUNK(4, GetFileVersionInfoSizeA)
DEFINE_VERSION_THUNK(5, GetFileVersionInfoSizeExA)
DEFINE_VERSION_THUNK(6, GetFileVersionInfoSizeExW)
DEFINE_VERSION_THUNK(7, GetFileVersionInfoSizeW)
DEFINE_VERSION_THUNK(8, GetFileVersionInfoW)
DEFINE_VERSION_THUNK(9, VerFindFileA)
DEFINE_VERSION_THUNK(10, VerFindFileW)
DEFINE_VERSION_THUNK(11, VerInstallFileA)
DEFINE_VERSION_THUNK(12, VerInstallFileW)
DEFINE_VERSION_THUNK(13, VerLanguageNameA)
DEFINE_VERSION_THUNK(14, VerLanguageNameW)
DEFINE_VERSION_THUNK(15, VerQueryValueA)
DEFINE_VERSION_THUNK(16, VerQueryValueIndexA)
DEFINE_VERSION_THUNK(17, VerQueryValueIndexW)
DEFINE_VERSION_THUNK(18, VerQueryValueW)

static bool load_system_version_dll() {
    if (g_real_version_dll != nullptr) {
        return true;
    }
    wchar_t sys_dir[MAX_PATH + 32] = {0};
    UINT len = GetSystemDirectoryW(sys_dir, MAX_PATH);
    if (len == 0 || len >= MAX_PATH) {
        return false;
    }
    wcscat_s(sys_dir, L"\\version.dll");
    g_real_version_dll = LoadLibraryW(sys_dir);
    if (!g_real_version_dll) {
        return false;
    }
    for (int i = 0; i < 19; ++i) {
        g_ver_procs[i] = GetProcAddress(g_real_version_dll, kVersionExportNames[i]);
    }
    return true;
}

// ============================================================================
// 2. Machine-Verified RVAs & Signatures for hoi4.exe v1.20.0.0.8873
// ============================================================================
static constexpr uintptr_t RVA_GAMESTATE_TICK_CLOCK   = 0x01f3670;
static constexpr uintptr_t RVA_SHOULD_RENDER_MIDTICK  = 0x02bacd0;
static constexpr uintptr_t RVA_SESSION_UPDATE         = 0x0f88c10;
static constexpr uintptr_t RVA_UPDATE_ONE_FRAME       = 0x2297b50;
static constexpr uintptr_t RVA_TIMER_GET_SEC          = 0x22c47a0;

static constexpr uintptr_t RVA_SET_LOGGER             = 0x2550050;
static constexpr uintptr_t RVA_SCOPED_TIMER_CTOR      = 0x25554d0;
static constexpr uintptr_t RVA_SCOPED_TIMER_DTOR      = 0x2555c20;
static constexpr uintptr_t RVA_WORKER_THREAD_INIT     = 0x255a110;
static constexpr uintptr_t RVA_TASK_WAIT              = 0x255afb0;
static constexpr uintptr_t RVA_EVENT_WAIT_AUTO        = 0x255b230;
static constexpr uintptr_t RVA_POOL_WAKE_ONE_OR_ALL   = 0x255b3e0;
static constexpr uintptr_t RVA_EXEC_TASK_NOW1         = 0x255d0f3;
static constexpr uintptr_t RVA_EXEC_TASK_NOW2         = 0x255d186;
static constexpr uintptr_t RVA_EXEC_TASK_LOCKADD_END  = 0x255d1b3;
static constexpr uintptr_t RVA_POP_TASK_OR_WAIT       = 0x255d2a0;
static constexpr uintptr_t RVA_FILE_FLUSH             = 0x259d9e0;
static constexpr uintptr_t RVA_SWITCH_TO_THREAD_THUNK = 0x25a6e48;
static constexpr uintptr_t RVA_TELEMETRY_CALL         = 0x20e9aa6;

static constexpr uintptr_t RVA_IAT_SWITCH_TO_THREAD   = 0x27892c0;
static constexpr uintptr_t RVA_IAT_TELEM_PAYLOAD_DTOR = 0x2789a20;
static constexpr uintptr_t RVA_IAT_QUEUE_TELEMETRY    = 0x2789a90;

static constexpr uintptr_t RVA_STR_GAMESTATE_HOURLY   = 0x27a4860;
static constexpr uintptr_t RVA_GAMESTATE_PTR          = 0x33e3b30;
static constexpr uintptr_t RVA_G_LOGGER_PTR           = 0x3695af8;

// v1.0: CPdxParallelForRange::Init & CModifier::GetValue (MSVC x64)
static constexpr uintptr_t RVA_PARFOR_RANGE_INIT      = 0x2576ac0;
static constexpr uintptr_t RVA_MODIFIER_GET_VAL       = 0x05a63d0;
static constexpr uintptr_t RVA_G_MODIFIER_DEFS        = 0x33e3650;
static constexpr uintptr_t RVA_G_FEATURE_FN           = 0x33e4a00;

// v1.0: CPdxNewDeleteAllocator vtable & virtual methods
static constexpr uintptr_t RVA_PDX_ALLOC_VT           = 0x2798570;
static constexpr uintptr_t OFF_PDX_ALLOC_SLOT         = 0x08;
static constexpr uintptr_t OFF_PDX_FREE_SLOT          = 0x10;
static constexpr uintptr_t RVA_PDX_ALLOC_FN           = 0x011d5d0;
static constexpr uintptr_t RVA_PDX_FREE_FN            = 0x011d5f0;

// v1.0: Civil War Freeze Fix (CCivilWarSetup::Execute + CInGameIdler map/supply rebuild)
static constexpr uintptr_t RVA_CW_SETUP_EXEC          = 0x1169250;
static constexpr uintptr_t RVA_INGAME_IDLER_VT        = 0x29ea628;
static constexpr uintptr_t OFF_IDLER_MAP_REBUILD_SLOT = 0x238;
static constexpr uintptr_t RVA_IDLER_MAP_REBUILD_FN   = 0x0b8fa80;

// v1.0 -> v1.3: NDefines::NAI & NDefines::NAITheatre supply/railway/motorization globals (.data)
// + Native Wartime Land `supply_node` Auto-Builder (FixSupplyBottlenecks @ 0x13a9fc0)
static constexpr uintptr_t RVA_NDEF_SUPPLY_FREQ_HRS   = 0x33e6124; // int32_t hours (vanilla 168)
static constexpr uintptr_t RVA_NDEF_SUPPLY_SAT_THR    = 0x33e61c8; // int64_t CFixedPoint64 x100,000 (vanilla 85,000)
static constexpr uintptr_t RVA_NDEF_SUPPLY_MOT_FREQ   = 0x33e6250; // int32_t hours (vanilla 52)
static constexpr uintptr_t RVA_NDEF_DIV_SUPPLY_MOT    = 0x33e9c48; // int64_t CFixedPoint64 x100,000 (vanilla 80,000)
static constexpr uintptr_t RVA_NDEF_PRIO_SUPPLY_BLDG  = 0x33e6510; // int64_t CFixedPoint64 x100,000 (vanilla 110,000)
static constexpr uintptr_t RVA_NDEF_PRIO_RAILWAY      = 0x33e65d8; // int64_t CFixedPoint64 x100,000 (vanilla 400,000)
static constexpr uintptr_t RVA_NDEF_SUPPLY_MAX_DEPTH  = 0x33e79a8; // int64_t CFixedPoint64 x100,000 (vanilla 500,000)
static constexpr uintptr_t RVA_NDEF_SUPPLY_CRISIS_LIM = 0x33e7a40; // int64_t CFixedPoint64 x100,000 (vanilla 10,000)
static constexpr uintptr_t RVA_FIX_SUPPLY_BOTTLENECKS = 0x13a9fc0;
static constexpr uintptr_t RVA_BASE_AI_GET_TAG        = 0x02c09f0;
static constexpr uintptr_t RVA_BASE_AI_GET_COUNTRY    = 0x02c0960;
static constexpr uintptr_t RVA_BLDG_DB_PTR            = 0x33e36e8;
static constexpr uintptr_t RVA_BLDG_PROD_LINE_VT      = 0x29fcfb8;
static constexpr size_t    OFF_COUNTRY_UNITS_DATA     = 0x290;
static constexpr size_t    OFF_COUNTRY_UNITS_LEN      = 0x29c;
static constexpr size_t    OFF_COUNTRY_CONSTR_PTR     = 0xf68;
static constexpr size_t    OFF_PROV_ID                = 0xa4;
static constexpr size_t    OFF_PROV_STATIC_DATA       = 0xb8;
static constexpr size_t    OFF_PROV_BLDG_CONTAINER    = 0x190;
static constexpr uintptr_t RVA_BLDG_CONT_CTRL_TAG     = 0x12071a0;
static constexpr uintptr_t RVA_BLDG_CONT_CAN_CONSTR   = 0x1208d40;
static constexpr uintptr_t RVA_BLDG_LEVEL_INIT        = 0x145f530;
static constexpr uintptr_t RVA_BLDG_LEVEL_GET         = 0x145f790;
static constexpr uintptr_t RVA_SBLDG_LOC_CTOR         = 0x1208690;
static constexpr uintptr_t RVA_OP_NEW                 = 0x25a39ac;
static constexpr uintptr_t RVA_ADD_CONSTR_CMD_CTOR    = 0x11d0110;
static constexpr uintptr_t RVA_SESSION_POST           = 0x02b6700;
static constexpr size_t    ADD_CONSTR_CMD_SIZE        = 0x50;

// v1.3: Active-Battleplan Empty-Tile `too_weak` Fix (CUnitOrder @ 0x1573921)
static constexpr uintptr_t RVA_EMPTY_TILE_SIG         = 0x1573921;
static constexpr uintptr_t RVA_EMPTY_TILE_PATCH1      = 0x1573929;
static constexpr uintptr_t RVA_EMPTY_TILE_PATCH2      = 0x1573933;

// v1.3: Smart Armored & Mechanized AI (CAIStrategyStore::GetStrategyValue + Tank/Upgrade NDefines::NAI)
static constexpr uintptr_t RVA_STRATEGY_STORE_VT      = 0x27b8be0;
static constexpr uintptr_t OFF_STRATEGY_GET_VAL_SLOT  = 0x20;
static constexpr uintptr_t RVA_STRATEGY_GET_VAL_FN    = 0x13744b0;
static constexpr uintptr_t RVA_CTOKEN_FROM_STRING     = 0x25416a0;
static constexpr uintptr_t RVA_NDEF_UPG_DEFICIT_DAYS  = 0x33e9e3c; // i32  (60 -> 210)
static constexpr uintptr_t RVA_NDEF_UPG_PCT_FORCES    = 0x33e7ce0; // fp64 (20,000 -> 35,000)
static constexpr uintptr_t RVA_NDEF_UPG_DIV_RELUCT    = 0x33e7c68; // i32  (7 -> 4)
static constexpr uintptr_t RVA_NDEF_VAR_XP_MOD_LAND   = 0x33e5f5c; // i32  (35 -> 20)
static constexpr uintptr_t RVA_NDEF_VAR_XP_LEG_LAND   = 0x33e6194; // i32  (35 -> 20)
static constexpr uintptr_t RVA_NDEF_VAR_XP_RES_LAND   = 0x33e63b4; // i32  (50 -> 30)
static constexpr uintptr_t RVA_NDEF_XP_UPG_LAND_EQ    = 0x33e7758; // fp64 (100,000 -> 200,000)
static constexpr uintptr_t RVA_NDEF_XP_UPD_LAND_TMPL  = 0x33e76c8; // fp64 (200,000 -> 250,000)
static constexpr uintptr_t RVA_NDEF_ASSIGN_TANKS_WAR  = 0x33e8cb0; // fp64 (600,000 -> 1,000,000)

// v1.1: Smart Airbase Expansion (CAIConstruction::GetBuildingQueuePrio + Air Coverage NDefines::NAI)
static constexpr uintptr_t RVA_GET_BLDG_QUEUE_PRIO    = 0x1395220;
static constexpr size_t    OFF_CONSTR_ITEM_INNER      = 0x78;
static constexpr size_t    OFF_CONSTR_BUILDING_PTR    = 0x1e0;
static constexpr size_t    OFF_BLDG_IS_AIR_BASE       = 0x363;
static constexpr uintptr_t RVA_NDEF_WANTED_PLANES_FAC = 0x33e6968; // fp64 (vanilla 50,000 -> 75,000)
static constexpr uintptr_t RVA_NDEF_AIR_COV_STAY_PUT  = 0x33eb380; // fp64 (vanilla 50,000 -> 75,000)
static constexpr uintptr_t RVA_NDEF_OUR_COMBATS_AIR   = 0x33e6ca8; // fp64 (vanilla 15,500,000 -> 45,000,000)
static constexpr uintptr_t RVA_NDEF_FIGHTERS_PER_PLN  = 0x33e7088; // fp64 (vanilla 100,000 -> 200,000)
static constexpr uintptr_t RVA_NDEF_DEF_SUPPLY_AIR    = 0x33e6488; // fp64 (vanilla 400,000 -> 900,000)

// v1.1 -> v1.2: Tactical Prudence, Equipment Conservation & AIFC Spearheads (Sheep AI non-cheating NDefines::NAI)
// NOTE: PLAN_ATTACK_MIN_STRENGTH_FACTOR_* are kept at vanilla so undefended empty-tile advances are never blocked.
static constexpr uintptr_t RVA_NDEF_ATK_HEAVY_DEF_LIM = 0x33ea930; // fp64 (50,000 -> 95,000)
static constexpr uintptr_t RVA_NDEF_PLAN_VAL_EXECUTE  = 0x33e7dc0; // fp64 signed (-50,000 -> -10,000)
static constexpr uintptr_t RVA_NDEF_LOC_BAL_ADVANCE   = 0x33ebad0; // fp64 (> 0 -> 0, Sheep AI)
static constexpr uintptr_t RVA_NDEF_STR_UNIT_WEAK     = 0x33eb258; // fp64 (30,000 -> 50,000)
static constexpr uintptr_t RVA_NDEF_ORG_UNIT_WEAK     = 0x33eb300; // fp64 (25,000 -> 35,000)
static constexpr uintptr_t RVA_NDEF_PLAN_WEAK_ABORT   = 0x33eb618; // fp64 (65,000 -> 45,000)
static constexpr uintptr_t RVA_NDEF_CANCEL_DISADV_RAT = 0x33ea9b0; // fp64 (150,000 -> 115,000)
static constexpr uintptr_t RVA_NDEF_CANCEL_MIN_DUR_HR = 0x33eaa7c; // i32  (48 -> 12)
static constexpr uintptr_t RVA_NDEF_DESP_WEAK_STR_LIM = 0x33ea738; // fp64 (35,000 -> 65,000)
static constexpr uintptr_t RVA_NDEF_MIN_FIELD_STR_BLD = 0x33eb488; // fp64 (70,000 -> 88,000)
static constexpr uintptr_t RVA_NDEF_START_TRAIN_EQ    = 0x33e76b8; // fp64 (40,000 -> 85,000)
static constexpr uintptr_t RVA_NDEF_STOP_TRAIN_EQ     = 0x33e7740; // fp64 (25,000 -> 75,000)
static constexpr uintptr_t RVA_NDEF_DEPLOY_MIN_EQ_WAR = 0x33eae30; // fp64 (85,000 -> 90,000)
static constexpr uintptr_t RVA_NDEF_PROD_EQ_SURPLUS   = 0x33ea400; // fp64 (80,000 -> 100,000)
static constexpr uintptr_t RVA_NDEF_FRONT_EVAL_ACC    = 0x33e9040; // fp64 (100,000 -> 150,000)
static constexpr uintptr_t RVA_NDEF_FRONT_EVAL_AIR    = 0x33e90f0; // fp64 (100,000 -> 200,000)
static constexpr uintptr_t RVA_NDEF_FRONT_EVAL_SUPLCK = 0x33e9188; // fp64 (100,000 -> 150,000)
static constexpr uintptr_t RVA_NDEF_TERRAIN_IMPORT    = 0x33e8c10; // fp64 (1,000,000)
static constexpr uintptr_t RVA_NDEF_TANKS_MOUNTAINS   = 0x33e8ad8; // fp64 signed (-> -1,500,000)
static constexpr uintptr_t RVA_NDEF_TANKS_JUNGLE      = 0x33e8b70; // fp64 signed (-> -2,000,000)
static constexpr uintptr_t RVA_NDEF_ENTRENCH_WEIGHT   = 0x33eadb8; // fp64 (200,000 -> 220,000)
static constexpr uintptr_t RVA_NDEF_ENCIRCLE_DISC_HRS = 0x33e7fe4; // i32  (72 -> 24)
static constexpr uintptr_t RVA_NDEF_BULGE_LOW_CUTOFF  = 0x33ebaa8; // fp64 (95,000 -> 120,000)
static constexpr uintptr_t RVA_NDEF_BULGE_HIGH_CUTOFF = 0x33eba08; // fp64 (150,000 -> 180,000)
static constexpr uintptr_t RVA_NDEF_CUTOFF_EDGE_PROX  = 0x33ebb60; // i32  (2 -> 1)
static constexpr uintptr_t RVA_NDEF_MAX_MICRO_ATKS    = 0x33e8e50; // i32  (3 -> 5)
static constexpr uintptr_t RVA_NDEF_MICRO_INACT_VAL   = 0x33eacb8; // fp64 (25,000 -> 15,000)
static constexpr uintptr_t RVA_NDEF_AIFC_UPD_DAYS     = 0x33ea2d4; // i32  (5 -> 4)
static constexpr uintptr_t RVA_NDEF_AIFC_UNIT_RATIO   = 0x33ea718; // fp64 (15,000 -> 22,000)
static constexpr uintptr_t RVA_NDEF_AIFC_ACTIVATE_ORG = 0x33eaab0; // fp64 (20,000 -> 30,000)
static constexpr uintptr_t RVA_NDEF_AIFC_NUDGE_DAYS   = 0x33eadf0; // i32  (15 -> 10)
static constexpr uintptr_t RVA_NDEF_AIFC_HARDNESS     = 0x33eb228; // fp64 (30,000,000 -> 80,000,000)
static constexpr uintptr_t RVA_NDEF_AIFC_ARMOR        = 0x33eb0b8; // fp64 (3,000,000 -> 8,000,000)
static constexpr uintptr_t RVA_NDEF_AIFC_SOFT_ATK     = 0x33eaf78; // fp64 (600,000 -> 1,200,000)
static constexpr uintptr_t RVA_NDEF_AIFC_HARD_ATK     = 0x33eb010; // fp64 (800,000 -> 1,600,000)
static constexpr uintptr_t RVA_NDEF_AIFC_EXPERIENCE   = 0x33eb688; // fp64 (30,000,000 -> 50,000,000)
static constexpr uintptr_t RVA_NDEF_AIFC_TGT_SUPPLY   = 0x33eb7d0; // fp64 (2,000,000 -> 3,000,000)
static constexpr uintptr_t RVA_NDEF_AIFC_TGT_PORT     = 0x33eb878; // fp64 (1,000,000 -> 1,500,000)
static constexpr uintptr_t RVA_NDEF_AIFC_TGT_PORT_LVL = 0x33eb910; // fp64 (100,000 -> 200,000)
static constexpr uintptr_t RVA_NDEF_AIFC_COST_MTN     = 0x33ec038; // fp64 (300,000 -> 400,000)

// v1.2: Smart Peace Conference Anti-Bordergore (CPeaceWinnerAi::CalcStateActionScore + CPeaceConference::IsStateClaimedBy)
static constexpr uintptr_t RVA_PEACE_CALC_STATE_SCORE = 0x1a8ca20;
static constexpr uintptr_t RVA_PEACE_IS_STATE_CLAIMED = 0x0ebc270;
static constexpr size_t    OFF_GS_STATES_DATA         = 0x2c8;
static constexpr size_t    OFF_GS_STATES_LEN          = 0x2d4;
static constexpr size_t    OFF_STATE_DATA_PTR         = 0x30;
static constexpr size_t    OFF_STATE_CORES_DATA       = 0xb0;
static constexpr size_t    OFF_STATE_CORES_LEN        = 0xbc;
static constexpr size_t    OFF_STATE_OWNER_TAG        = 0xc8;
static constexpr size_t    OFF_STATEDATA_NB_DATA      = 0x110;
static constexpr size_t    OFF_STATEDATA_NB_LEN       = 0x11c;
static constexpr size_t    OFF_STATEDATA_IS_COASTAL   = 0x15d;
static constexpr int32_t   PEACE_ACT_TAKE_STATES      = 0x30cf;
static constexpr int32_t   PEACE_ACT_GIVE_STATES      = 0x29e6;
static constexpr int32_t   PEACE_ACT_PUPPET           = 0x30d1;
static constexpr uintptr_t RVA_NDEF_PEACE_FOLD_TURNS  = 0x33e652c; // i32  (2 -> 1)
static constexpr uintptr_t RVA_NDEF_PEACE_TIE_DIST    = 0x33e6698; // fp64 (100,000 -> 400,000)

// v1.0: Dormant Peaceful Minor AI Hourly Throttling (CCountryAI vtable & CCountry offsets)
static constexpr uintptr_t RVA_COUNTRY_AI_VT          = 0x27b8c20;
static constexpr uintptr_t OFF_COUNTRY_AI_PASS1_SLOT  = 0x10;
static constexpr uintptr_t RVA_COUNTRY_AI_PASS1_FN    = 0x02c4ae0;
static constexpr uintptr_t OFF_COUNTRY_AI_PASS2_SLOT  = 0x18;
static constexpr uintptr_t RVA_COUNTRY_AI_PASS2_FN    = 0x02c4b50;
static constexpr uintptr_t RVA_COUNTRY_HAS_HUMAN      = 0x0748ca0;
static constexpr uintptr_t RVA_DIPLO_IS_AT_WAR        = 0x0da7bb0;
static constexpr size_t    OFF_GS_CDATE_HOURS         = 0x468; // gs + 0x460 + 0x08
static constexpr size_t    OFF_COUNTRY_TAG_ID         = 0x08;
static constexpr size_t    OFF_COUNTRY_NUM_STATES     = 0x484;
static constexpr size_t    OFF_COUNTRY_DIPLO_PTR      = 0xf88;
static constexpr size_t    OFF_COUNTRY_IS_MAJOR       = 0x145a;

// CGameState field offsets on Windows MSVC x64 (verified @ 0xf88d69 & 0x1f4031..0x1f404b)
static constexpr size_t OFF_GS_NUM_PLAYERS = 0x104;
static constexpr size_t OFF_GS_GAME_SPEED  = 0x4bc;
static constexpr size_t OFF_GS_PAUSED      = 0x4c0;

// CEvent stride on Windows MSVC x64 (verified @ 0x255b0d8 & 0x255d3c1: 5 * 32 = 0xa0)
static constexpr intptr_t WIN_CEVENT_SIZE  = 0xa0;

static constexpr uint8_t SIG_UPDATE_ONE_FRAME[16] = {
    0x40, 0x53, 0x55, 0x56, 0x57, 0x41, 0x56, 0x48,
    0x83, 0xec, 0x70, 0x0f, 0x29, 0x74, 0x24, 0x60
};
static constexpr uint8_t SIG_SESSION_UPDATE[15] = {
    0x4c, 0x8b, 0xdc, 0x49, 0x89, 0x5b, 0x08, 0x49,
    0x89, 0x6b, 0x18, 0x49, 0x89, 0x73, 0x20
};
static constexpr uint8_t SIG_TICK_CLOCK[16] = {
    0x40, 0x55, 0x53, 0x56, 0x57, 0x41, 0x54, 0x41,
    0x55, 0x41, 0x56, 0x41, 0x57, 0x48, 0x8d, 0xac
};
static constexpr uint8_t SIG_SHOULD_RENDER_MIDTICK[16] = {
    0x48, 0x89, 0x5c, 0x24, 0x10, 0x57, 0x48, 0x83,
    0xec, 0x30, 0x48, 0x8b, 0x01, 0x8b, 0xfa, 0x48
};
static constexpr uint8_t SIG_POP_TASK_OR_WAIT[17] = {
    0x48, 0x89, 0x5c, 0x24, 0x10, 0x48, 0x89, 0x74,
    0x24, 0x18, 0x48, 0x89, 0x7c, 0x24, 0x20, 0x41, 0x56
};
static constexpr uint8_t SIG_TASK_WAIT[16] = {
    0x40, 0x53, 0x48, 0x83, 0xec, 0x20, 0x48, 0x8b,
    0x59, 0x18, 0x48, 0x85, 0xdb, 0x74, 0x72, 0x0f
};
static constexpr uint8_t SIG_WORKER_THREAD_INIT[16] = {
    0x48, 0x83, 0xec, 0x28, 0x85, 0xc9, 0x74, 0x39,
    0x83, 0xe9, 0x01, 0x74, 0x1e, 0x83, 0xf9, 0x01
};
static constexpr uint8_t SIG_SWITCH_TO_THREAD_THUNK[7] = {
    0x48, 0xff, 0x25, 0x71, 0x24, 0x1e, 0x00
};
static constexpr uint8_t SIG_EXEC_TASK_NOW1[5] = {
    0xe8, 0x08, 0x1c, 0xcb, 0xfd
};
static constexpr uint8_t SIG_EXEC_TASK_NOW2[12] = {
    0xe8, 0x75, 0x1b, 0xcb, 0xfd, 0x48, 0x8b, 0x08,
    0x48, 0x2b, 0x4c, 0x24
};
static constexpr uint8_t SIG_EXEC_TASK_LOCKADD_END[7] = {
    0xf0, 0x48, 0x01, 0x10, 0x41, 0x88, 0x2f
};
static constexpr uint8_t SIG_SET_LOGGER[17] = {
    0x4c, 0x8b, 0x05, 0xa1, 0x5a, 0x14, 0x01, 0x48,
    0x89, 0x0d, 0x9a, 0x5a, 0x14, 0x01, 0x4d, 0x85, 0xc0
};
static constexpr uint8_t SIG_FILE_FLUSH[16] = {
    0x48, 0x83, 0xec, 0x28, 0xff, 0x15, 0xa6, 0xbd,
    0x1e, 0x00, 0x85, 0xc0, 0x75, 0x2c, 0xff, 0x15
};
static constexpr uint8_t SIG_TELEMETRY_CALL[6] = {
    0xff, 0x15, 0xe4, 0xff, 0x69, 0x00
};
static constexpr uint8_t SIG_SCOPED_TIMER_CTOR[16] = {
    0x48, 0x89, 0x74, 0x24, 0x18, 0x57, 0x48, 0x83,
    0xec, 0x30, 0x33, 0xc0, 0x48, 0xc7, 0x41, 0x10
};
static constexpr uint8_t SIG_SCOPED_TIMER_DTOR[16] = {
    0x40, 0x53, 0x48, 0x83, 0xec, 0x20, 0x48, 0x83,
    0x79, 0x10, 0xff, 0x48, 0x8b, 0xd9, 0x0f, 0x84
};
static constexpr uint8_t SIG_PARFOR_RANGE_INIT[15] = {
    0x48, 0x89, 0x5c, 0x24, 0x08, 0x48, 0x89, 0x6c,
    0x24, 0x10, 0x48, 0x89, 0x74, 0x24, 0x18
};
static constexpr uint8_t SIG_MODIFIER_GET_VAL[15] = {
    0x48, 0x89, 0x5c, 0x24, 0x18, 0x48, 0x89, 0x74,
    0x24, 0x20, 0x57, 0x48, 0x83, 0xec, 0x20
};
static constexpr uint8_t SIG_CW_SETUP_EXEC[15] = {
    0x48, 0x8b, 0xc4, 0x4c, 0x89, 0x40, 0x18, 0x48,
    0x89, 0x50, 0x10, 0x48, 0x89, 0x48, 0x08
};
static constexpr uint8_t SIG_GET_BLDG_QUEUE_PRIO[16] = {
    0x4c, 0x89, 0x44, 0x24, 0x18, 0x48, 0x89, 0x54,
    0x24, 0x10, 0x53, 0x55, 0x56, 0x57, 0x41, 0x54
};
static constexpr uint8_t SIG_PEACE_CALC_STATE_SCORE[15] = {
    0x48, 0x89, 0x5c, 0x24, 0x10, 0x4c, 0x89, 0x4c,
    0x24, 0x20, 0x4c, 0x89, 0x44, 0x24, 0x18
};
static constexpr uint8_t SIG_PEACE_IS_STATE_CLAIMED[8] = {
    0x48, 0x83, 0xec, 0x28, 0x4c, 0x8b, 0x89, 0x28
};
static constexpr uint8_t SIG_FIX_SUPPLY_BOTTLENECKS[16] = {
    0x48, 0x89, 0x5c, 0x24, 0x18, 0x55, 0x56, 0x57,
    0x41, 0x54, 0x41, 0x55, 0x41, 0x56, 0x41, 0x57
};
static constexpr uint8_t SIG_EMPTY_TILE_BRANCH[20] = {
    0x48, 0x8b, 0x45, 0xa0, 0x48, 0x39, 0x45, 0xa8, 0x7c, 0x78,
    0x48, 0x8b, 0x45, 0xb0, 0x48, 0x3b, 0x45, 0xb8, 0x7c, 0x6e
};

// ============================================================================
// 3. Timing, State & Profiler Tables
// ============================================================================
static uintptr_t g_exe_base = 0;
static DWORD g_main_tid = 0;
static DWORD g_tls_yield_idx = TLS_OUT_OF_INDEXES;
static DWORD g_tls_slab_idx = TLS_OUT_OF_INDEXES;
static uint64_t g_qpc_freq = 10000000ULL;
static wchar_t g_log_path[MAX_PATH] = {0};
static wchar_t g_ini_path[MAX_PATH] = {0};

static std::atomic<bool> g_active_for_hoi4{false};
static std::atomic<bool> g_collect_zones{false};
static std::atomic<bool> g_profile_started_sim{false};
static std::atomic<bool> g_bench_recording{false};
static std::atomic<bool> g_report_emitted{false};

static std::atomic<uintptr_t> g_hourly_ptr{0};
static std::atomic<uint64_t> g_total_hours{0};
static std::atomic<uint64_t> g_bench_warmup_hours{24};
static std::atomic<uint64_t> g_bench_max_hours{0};
static std::atomic<uint64_t> g_bench_start_ns{0};
static std::atomic<uint64_t> g_bench_start_cpu_ns{0};

// Feature flags & live telemetry counters
static std::atomic<bool> g_fast_sched_enabled{false};
static std::atomic<bool> g_fast_wait_enabled{false};
static std::atomic<bool> g_fast_yield_enabled{false};
static std::atomic<bool> g_no_job_clock_enabled{false};
static std::atomic<bool> g_affinity_enabled{false};
static std::atomic<bool> g_quiet_log_enabled{false};
static std::atomic<bool> g_fast_save_enabled{false};
static std::atomic<bool> g_no_telemetry_enabled{false};
static std::atomic<bool> g_country_grain_enabled{false};
static std::atomic<bool> g_fast_mod_enabled{false};
static std::atomic<bool> g_fast_alloc_enabled{false};
static std::atomic<bool> g_civil_war_fix_enabled{false};
static std::atomic<bool> g_ai_smart_supply_enabled{false};
static std::atomic<bool> g_ai_smart_supply_applied{false};
static std::atomic<bool> g_ai_smart_airbase_enabled{false};
static std::atomic<bool> g_ai_smart_airbase_defines_applied{false};
static std::atomic<bool> g_ai_tactical_prudence_enabled{false};
static std::atomic<bool> g_ai_tactical_prudence_applied{false};
static std::atomic<bool> g_empty_tile_fix_applied{false};
static std::atomic<bool> g_ai_smart_armor_enabled{false};
static std::atomic<bool> g_ai_smart_armor_defines_applied{false};
static std::atomic<bool> g_ai_smart_peace_enabled{false};
static std::atomic<bool> g_ai_smart_peace_defines_applied{false};
static std::atomic<bool> g_ai_throttle_minors_enabled{false};

static std::atomic<uint64_t> g_sched_watcher_polls{0};
static std::atomic<uint64_t> g_yield_elided{0};
static std::atomic<uint64_t> g_fsync_skipped{0};
static std::atomic<uint64_t> g_telemetry_blocked{0};
static std::atomic<uint64_t> g_log_allowed{0};
static std::atomic<uint64_t> g_log_suppressed{0};
static std::atomic<uint64_t> g_parfor_rebalanced{0};
static std::atomic<uint64_t> g_parfor_total{0};
static std::atomic<uint64_t> g_slab_alloc_hits{0};
static std::atomic<uint64_t> g_slab_free_hits{0};
static std::atomic<uint64_t> g_slab_fallbacks{0};
static std::atomic<uint64_t> g_cw_rebuilds_coalesced{0};
static std::atomic<uint64_t> g_ai_supply_hubs_queued{0};
static std::atomic<uint64_t> g_airbase_prio_boosts{0};
static std::atomic<uint64_t> g_ai_armor_strategy_boosts{0};
static std::atomic<uint64_t> g_peace_bordergore_adjusts{0};
static std::atomic<uint64_t> g_minor_ai_skipped{0};
static std::atomic<uint64_t> g_minor_ai_executed{0};

static std::atomic<uintptr_t> g_orig_parfor_range_init{0};
static std::atomic<uintptr_t> g_orig_modifier_get_val{0};
static std::atomic<uintptr_t> g_orig_pdx_alloc{0};
static std::atomic<uintptr_t> g_orig_pdx_free{0};
static std::atomic<uintptr_t> g_orig_cw_setup_exec{0};
static std::atomic<uintptr_t> g_orig_idler_map_rebuild{0};
static std::atomic<uintptr_t> g_orig_fix_supply_bottlenecks{0};
static std::atomic<uintptr_t> g_orig_get_bldg_queue_prio{0};
static std::atomic<uintptr_t> g_orig_get_strategy_value{0};
static std::atomic<uintptr_t> g_orig_peace_calc_state_score{0};
static std::atomic<uintptr_t> g_orig_country_ai_pass1{0};
static std::atomic<uintptr_t> g_orig_country_ai_pass2{0};
static std::atomic<uintptr_t> g_supply_node_bldg_ptr{0};
static std::atomic<uintptr_t> g_naval_base_bldg_ptr{0};
static std::atomic<bool>      g_bldg_db_resolved{false};
static std::atomic<uint32_t>  g_supply_hub_last_hr[512];
static std::atomic<int32_t>   g_tok_armor{-1};
static std::atomic<int32_t>   g_tok_mechanized{-1};
static std::atomic<int32_t>   g_tok_infantry{-1};
static std::atomic<uint32_t>  g_in_civil_war_depth{0};
static std::atomic<uintptr_t> g_deferred_cw_idler{0};
static std::atomic<uint64_t>  g_minor_skip_bitmap[8];

// Frame pacing & Speed-5 bursting state
static std::atomic<uintptr_t> g_orig_update_one_frame{0};
static std::atomic<uintptr_t> g_orig_session_update{0};
static std::atomic<uintptr_t> g_orig_switch_to_thread{0};
static std::atomic<uint64_t> g_target_frame_ns{0};
static std::atomic<uint64_t> g_max_burst_ticks{16};
static std::atomic<uint64_t> g_frames_rendered{0};
static std::atomic<uint64_t> g_active_sim_frames{0};
static std::atomic<uint64_t> g_active_sim_ns{0};
static std::atomic<uint64_t> g_burst_extra_ticks{0};
static std::atomic<uint64_t> g_prev_session_end_ns{0};
static std::atomic<uint64_t> g_last_frame_start_ns{0};
static std::atomic<uint64_t> g_ema_render_ns{8000000ULL};
static std::atomic<uint64_t> g_ema_tick_ns{14000000ULL};
static std::atomic<uint64_t> g_worst_frame_ns{0};
static std::atomic<bool> g_in_burst{false};

// Mid-tick RDTSC anti-freeze pacer
static std::atomic<uint64_t> g_smooth_fps{0};
static std::atomic<uint64_t> g_mid_tick_checks{0};
static std::atomic<uint64_t> g_mid_tick_fast_skip{0};
static std::atomic<uint64_t> g_mid_tick_renders{0};
static std::atomic<uint64_t> g_last_mid_check_tsc{0};
static std::atomic<uint64_t> g_tsc_per_ms{0};
static std::atomic<uint64_t> g_tsc_cal_ns{0};
static std::atomic<uint64_t> g_tsc_cal_tsc{0};

// CPU Topology Pinning
static uint32_t g_topo_worker_cpus[64] = {0};
static size_t g_topo_worker_len = 0;
static uint32_t g_topo_bg_cpus[64] = {0};
static size_t g_topo_bg_len = 0;
static std::atomic<size_t> g_worker_pin_counter{0};
static std::atomic<size_t> g_bg_pin_counter{0};

// Lock-free Zone Profiler table
static constexpr size_t ZONE_CAP = 2048;
struct ZoneSlot {
    std::atomic<uintptr_t> name_ptr{0};
    std::atomic<uint64_t> calls{0};
    std::atomic<uint64_t> total_ns{0};
    std::atomic<uint64_t> max_ns{0};
    std::atomic<uint64_t> main_ns{0};
    std::atomic<uint64_t> worker_ns{0};
};
static ZoneSlot g_zones[ZONE_CAP];

// Lock-free Logger Deduplicator v2
static constexpr size_t LOG_DEDUP_CAP = 8192;
static constexpr size_t LOG_VT_CAP = 16;
struct LogDedupSlot {
    std::atomic<uint64_t> hash{0};
    std::atomic<uint32_t> count{0};
};
static LogDedupSlot g_log_dedup[LOG_DEDUP_CAP];
static std::atomic<uintptr_t> g_log_vt_keys[LOG_VT_CAP];
static std::atomic<uintptr_t> g_log_vt_orig[LOG_VT_CAP];

__forceinline uint64_t now_ns() {
    LARGE_INTEGER li;
    QueryPerformanceCounter(&li);
    return static_cast<uint64_t>(
        (static_cast<unsigned __int128>(static_cast<uint64_t>(li.QuadPart)) * 1000000000ULL) /
        g_qpc_freq
    );
}

static uint64_t now_cpu_ns() {
    FILETIME ct{}, et{}, kt{}, ut{};
    if (!GetProcessTimes(GetCurrentProcess(), &ct, &et, &kt, &ut)) {
        return 0;
    }
    uint64_t k = (static_cast<uint64_t>(kt.dwHighDateTime) << 32) | kt.dwLowDateTime;
    uint64_t u = (static_cast<uint64_t>(ut.dwHighDateTime) << 32) | ut.dwLowDateTime;
    return (k + u) * 100ULL;
}

__forceinline size_t hash_uintptr(uintptr_t p) {
    uint64_t x = static_cast<uint64_t>(p) * 0x9e3779b97f4a7c15ULL;
    x ^= x >> 30;
    x *= 0xbf58476d1ce4e5b9ULL;
    return static_cast<size_t>(x ^ (x >> 27));
}

static void append_log_line(const char* text, bool truncate_first = false) {
    if (g_log_path[0] == L'\0') {
        return;
    }
    HANDLE h = CreateFileW(
        g_log_path,
        FILE_GENERIC_WRITE,
        FILE_SHARE_READ | FILE_SHARE_WRITE,
        nullptr,
        truncate_first ? CREATE_ALWAYS : OPEN_ALWAYS,
        FILE_ATTRIBUTE_NORMAL,
        nullptr
    );
    if (h == INVALID_HANDLE_VALUE) {
        return;
    }
    if (!truncate_first) {
        SetFilePointer(h, 0, nullptr, FILE_END);
    }
    DWORD written = 0;
    WriteFile(h, text, static_cast<DWORD>(std::strlen(text)), &written, nullptr);
    CloseHandle(h);
}

static void reset_stats() {
    for (size_t i = 0; i < ZONE_CAP; ++i) {
        if (g_zones[i].name_ptr.load(std::memory_order_relaxed) != 0) {
            g_zones[i].calls.store(0, std::memory_order_relaxed);
            g_zones[i].total_ns.store(0, std::memory_order_relaxed);
            g_zones[i].max_ns.store(0, std::memory_order_relaxed);
            g_zones[i].main_ns.store(0, std::memory_order_relaxed);
            g_zones[i].worker_ns.store(0, std::memory_order_relaxed);
        }
    }
    g_sched_watcher_polls.store(0, std::memory_order_relaxed);
    g_yield_elided.store(0, std::memory_order_relaxed);
    g_frames_rendered.store(0, std::memory_order_relaxed);
    g_active_sim_frames.store(0, std::memory_order_relaxed);
    g_active_sim_ns.store(0, std::memory_order_relaxed);
    g_burst_extra_ticks.store(0, std::memory_order_relaxed);
    g_worst_frame_ns.store(0, std::memory_order_relaxed);
    g_mid_tick_checks.store(0, std::memory_order_relaxed);
    g_mid_tick_fast_skip.store(0, std::memory_order_relaxed);
    g_mid_tick_renders.store(0, std::memory_order_relaxed);
    g_parfor_rebalanced.store(0, std::memory_order_relaxed);
    g_parfor_total.store(0, std::memory_order_relaxed);
    g_slab_alloc_hits.store(0, std::memory_order_relaxed);
    g_slab_free_hits.store(0, std::memory_order_relaxed);
    g_slab_fallbacks.store(0, std::memory_order_relaxed);
    g_cw_rebuilds_coalesced.store(0, std::memory_order_relaxed);
    g_ai_supply_hubs_queued.store(0, std::memory_order_relaxed);
    g_airbase_prio_boosts.store(0, std::memory_order_relaxed);
    g_ai_armor_strategy_boosts.store(0, std::memory_order_relaxed);
    g_peace_bordergore_adjusts.store(0, std::memory_order_relaxed);
    g_minor_ai_skipped.store(0, std::memory_order_relaxed);
    g_minor_ai_executed.store(0, std::memory_order_relaxed);
}

static void emit_report(uint64_t measured_hours);
static void apply_ai_smart_supply_once();
static void apply_ai_smart_airbase_once();
static void apply_ai_tactical_prudence_once();
static void apply_ai_smart_armor_once();
static void apply_ai_smart_peace_once();

// ============================================================================
// 4. Safe Patching & Executable Trampoline Helpers (W^X + FlushInstructionCache)
// ============================================================================
static bool verify_sig(uintptr_t addr, const uint8_t* expected, size_t len) {
    return std::memcmp(reinterpret_cast<const void*>(addr), expected, len) == 0;
}

static bool patch_bytes(uintptr_t addr, const uint8_t* bytes, size_t len) {
    DWORD old_prot = 0;
    if (!VirtualProtect(reinterpret_cast<void*>(addr), len, PAGE_EXECUTE_READWRITE, &old_prot)) {
        return false;
    }
    std::memcpy(reinterpret_cast<void*>(addr), bytes, len);
    DWORD dummy = 0;
    VirtualProtect(reinterpret_cast<void*>(addr), len, old_prot, &dummy);
    FlushInstructionCache(GetCurrentProcess(), reinterpret_cast<const void*>(addr), len);
    return true;
}

static bool patch_data_ptr(uintptr_t addr, uintptr_t new_val) {
    DWORD old_prot = 0;
    if (!VirtualProtect(reinterpret_cast<void*>(addr), sizeof(uintptr_t), PAGE_READWRITE, &old_prot)) {
        return false;
    }
    *reinterpret_cast<uintptr_t*>(addr) = new_val;
    DWORD dummy = 0;
    VirtualProtect(reinterpret_cast<void*>(addr), sizeof(uintptr_t), old_prot, &dummy);
    return true;
}

static bool install_abs_jmp(uintptr_t addr, uintptr_t target) {
    uint8_t patch[14];
    patch[0] = 0xff;
    patch[1] = 0x25;
    std::memset(&patch[2], 0, 4);
    std::memcpy(&patch[6], &target, 8);
    return patch_bytes(addr, patch, sizeof(patch));
}

static uintptr_t install_trampoline_hook(
    uintptr_t func_addr,
    const uint8_t* expected_prologue,
    size_t plen,
    uintptr_t hook_target
) {
    if (plen < 14 || plen > 32 || !verify_sig(func_addr, expected_prologue, plen)) {
        return 0;
    }
    auto* tramp = static_cast<uint8_t*>(
        VirtualAlloc(nullptr, 4096, MEM_COMMIT | MEM_RESERVE, PAGE_READWRITE)
    );
    if (!tramp) {
        return 0;
    }
    std::memcpy(tramp, expected_prologue, plen);
    uint64_t ret_addr = static_cast<uint64_t>(func_addr + plen);
    tramp[plen + 0] = 0xff;
    tramp[plen + 1] = 0x25;
    std::memset(&tramp[plen + 2], 0, 4);
    std::memcpy(&tramp[plen + 6], &ret_addr, 8);

    DWORD old_prot = 0;
    if (!VirtualProtect(tramp, 4096, PAGE_EXECUTE_READ, &old_prot)) {
        VirtualFree(tramp, 0, MEM_RELEASE);
        return 0;
    }
    FlushInstructionCache(GetCurrentProcess(), tramp, plen + 14);

    uint8_t patch[32];
    std::memset(patch, 0x90, plen);
    patch[0] = 0xff;
    patch[1] = 0x25;
    std::memset(&patch[2], 0, 4);
    std::memcpy(&patch[6], &hook_target, 8);
    if (!patch_bytes(func_addr, patch, plen)) {
        VirtualFree(tramp, 0, MEM_RELEASE);
        return 0;
    }
    return reinterpret_cast<uintptr_t>(tramp);
}

// ============================================================================
// 5. Frame Pacing & Render-Aware Multi-Tick Speed-5 Bursting
//    - UpdateOneFrame (0x2297b50): true rendered FPS & worst-frame tracker
//    - SessionUpdate  (0x0f88c10): Speed-5 multi-tick bursting inside budget
// ============================================================================
using UpdateOneFrameFn = void (*)(uint8_t* idler, uint8_t allow_sim);
using SessionUpdateFn  = void (*)(uint8_t* session);

extern "C" void hook_update_one_frame(uint8_t* idler, uint8_t allow_sim) {
    uintptr_t orig_ptr = g_orig_update_one_frame.load(std::memory_order_relaxed);
    auto orig = reinterpret_cast<UpdateOneFrameFn>(orig_ptr);

    uint64_t frame_start = now_ns();
    bool is_unpaused_sim = false;
    if (g_exe_base != 0) {
        auto* gs = *reinterpret_cast<uint8_t* const*>(g_exe_base + RVA_GAMESTATE_PTR);
        if (gs != nullptr) {
            is_unpaused_sim = (gs[OFF_GS_PAUSED] == 0);
            if (g_ai_smart_supply_enabled.load(std::memory_order_relaxed) &&
                !g_ai_smart_supply_applied.load(std::memory_order_relaxed)) {
                apply_ai_smart_supply_once();
            }
            if (g_ai_smart_airbase_enabled.load(std::memory_order_relaxed) &&
                !g_ai_smart_airbase_defines_applied.load(std::memory_order_relaxed)) {
                apply_ai_smart_airbase_once();
            }
            if (g_ai_tactical_prudence_enabled.load(std::memory_order_relaxed) &&
                !g_ai_tactical_prudence_applied.load(std::memory_order_relaxed)) {
                apply_ai_tactical_prudence_once();
            }
            if (g_ai_smart_armor_enabled.load(std::memory_order_relaxed) &&
                !g_ai_smart_armor_defines_applied.load(std::memory_order_relaxed)) {
                apply_ai_smart_armor_once();
            }
            if (g_ai_smart_peace_enabled.load(std::memory_order_relaxed) &&
                !g_ai_smart_peace_defines_applied.load(std::memory_order_relaxed)) {
                apply_ai_smart_peace_once();
            }
        }
    }

    uint64_t prev_start = g_last_frame_start_ns.exchange(frame_start, std::memory_order_relaxed);
    if (prev_start > 0 && frame_start > prev_start) {
        uint64_t fdt = frame_start - prev_start;
        if (is_unpaused_sim && fdt < 2000000000ULL) {
            g_active_sim_frames.fetch_add(1, std::memory_order_relaxed);
            g_active_sim_ns.fetch_add(fdt, std::memory_order_relaxed);
        }
        if (fdt < 2000000000ULL) {
            uint64_t cur_w = g_worst_frame_ns.load(std::memory_order_relaxed);
            while (fdt > cur_w &&
                   !g_worst_frame_ns.compare_exchange_weak(
                       cur_w, fdt, std::memory_order_relaxed, std::memory_order_relaxed)) {
            }
        }
    }

    orig(idler, allow_sim);
    g_frames_rendered.fetch_add(1, std::memory_order_relaxed);
}

extern "C" void hook_session_update(uint8_t* session) {
    uintptr_t orig_ptr = g_orig_session_update.load(std::memory_order_relaxed);
    auto orig = reinterpret_cast<SessionUpdateFn>(orig_ptr);

    uint64_t session_start = now_ns();
    uint64_t prev_end = g_prev_session_end_ns.load(std::memory_order_relaxed);
    if (prev_end > 0 && session_start > prev_end) {
        uint64_t rdt = session_start - prev_end;
        if (rdt > 100000000ULL) {
            rdt = 100000000ULL;
        }
        uint64_t old_r = g_ema_render_ns.load(std::memory_order_relaxed);
        g_ema_render_ns.store((old_r * 3 + rdt) / 4, std::memory_order_relaxed);
    }

    uint64_t budget_ns = g_target_frame_ns.load(std::memory_order_relaxed);
    if (budget_ns == 0 || g_in_burst.exchange(true, std::memory_order_acq_rel)) {
        orig(session);
        g_prev_session_end_ns.store(now_ns(), std::memory_order_relaxed);
        return;
    }

    uint64_t h0 = g_total_hours.load(std::memory_order_relaxed);
    uint64_t t0 = now_ns();
    orig(session);
    uint64_t t1 = now_ns();
    uint64_t h_prev = g_total_hours.load(std::memory_order_relaxed);

    if (h_prev > h0) {
        uint64_t tick_dt = (t1 > t0) ? (t1 - t0) : 0;
        if (tick_dt > 500000000ULL) {
            tick_dt = 500000000ULL;
        }
        uint64_t old_t = g_ema_tick_ns.load(std::memory_order_relaxed);
        uint64_t avg_tick = (old_t * 3 + tick_dt) / 4;
        g_ema_tick_ns.store(avg_tick, std::memory_order_relaxed);

        uint64_t render_est = g_ema_render_ns.load(std::memory_order_relaxed);
        uint64_t sim_budget_ns = (budget_ns > render_est) ? (budget_ns - render_est) : 0;
        uint64_t max_burst = g_max_burst_ticks.load(std::memory_order_relaxed);
        uint64_t extra = 0;

        while (extra < max_burst) {
            uint64_t now = now_ns();
            uint64_t elapsed = (now > session_start) ? (now - session_start) : 0;
            uint64_t predicted_end = (budget_ns <= 55000000ULL)
                                         ? (elapsed + (avg_tick * 3) / 4)
                                         : elapsed;
            if (predicted_end >= sim_budget_ns) {
                break;
            }
            auto* gs = *reinterpret_cast<uint8_t* const*>(g_exe_base + RVA_GAMESTATE_PTR);
            if (!gs) {
                break;
            }
            int32_t speed = *reinterpret_cast<const int32_t*>(gs + OFF_GS_GAME_SPEED);
            uint8_t paused = gs[OFF_GS_PAUSED];
            int32_t num_players = *reinterpret_cast<const int32_t*>(gs + OFF_GS_NUM_PLAYERS);
            // Strictly Speed 5 (index 4), unpaused, and singleplayer (num_players <= 1)
            if (speed != 4 || paused != 0 || num_players > 1) {
                break;
            }

            uint64_t bt0 = now_ns();
            orig(session);
            uint64_t bt1 = now_ns();
            uint64_t h_now = g_total_hours.load(std::memory_order_relaxed);
            if (h_now <= h_prev) {
                break;
            }
            uint64_t bdt = (bt1 > bt0) ? (bt1 - bt0) : 0;
            if (bdt > 500000000ULL) {
                bdt = 500000000ULL;
            }
            avg_tick = (avg_tick * 3 + bdt) / 4;
            g_ema_tick_ns.store(avg_tick, std::memory_order_relaxed);
            h_prev = h_now;
            ++extra;
        }
        if (extra > 0) {
            g_burst_extra_ticks.fetch_add(extra, std::memory_order_relaxed);
        }
    }

    g_prev_session_end_ns.store(now_ns(), std::memory_order_relaxed);
    g_in_burst.store(false, std::memory_order_release);
}

// ============================================================================
// 6. Mid-Tick Anti-Freeze Frame Pacer (ShouldRenderMidTick @ 0x2bacd0)
//    Verified MSVC vtable offsets: +0x3a0 (is_async_ready), +0x2f0 (is_cam_moving)
// ============================================================================
using IdlerBoolMethodFn = uint8_t (*)(const uint8_t*);
using TimerGetSecFn     = double (*)(const uint8_t*);

extern "C" uint8_t hook_should_render_mid_tick(const uint8_t* idler, int game_speed) {
    if (g_in_civil_war_depth.load(std::memory_order_relaxed) > 0) {
        return 0;
    }
    uint64_t tsc = __rdtsc();
    uint64_t tsc_per_ms = g_tsc_per_ms.load(std::memory_order_relaxed);
    if (tsc_per_ms > 0) {
        uint64_t next_tsc = g_last_mid_check_tsc.load(std::memory_order_relaxed);
        if (tsc < next_tsc) {
            g_mid_tick_fast_skip.fetch_add(1, std::memory_order_relaxed);
            return 0;
        }
        g_last_mid_check_tsc.store(tsc + tsc_per_ms * 2, std::memory_order_relaxed);
    } else {
        uint64_t cal_ns = g_tsc_cal_ns.load(std::memory_order_relaxed);
        uint64_t now = now_ns();
        if (cal_ns == 0) {
            g_tsc_cal_tsc.store(tsc, std::memory_order_relaxed);
            g_tsc_cal_ns.store(now, std::memory_order_relaxed);
        } else if (now > cal_ns && (now - cal_ns) >= 10000000ULL) {
            uint64_t dt_ns = now - cal_ns;
            uint64_t cal_tsc = g_tsc_cal_tsc.load(std::memory_order_relaxed);
            if (tsc > cal_tsc) {
                uint64_t dt_tsc = tsc - cal_tsc;
                auto per_ms = static_cast<uint64_t>(
                    (static_cast<unsigned __int128>(dt_tsc) * 1000000ULL) / dt_ns
                );
                if (per_ms >= 500000ULL && per_ms <= 20000000ULL) {
                    g_tsc_per_ms.store(per_ms, std::memory_order_relaxed);
                    g_last_mid_check_tsc.store(tsc + per_ms * 2, std::memory_order_relaxed);
                }
            }
        }
    }

    g_mid_tick_checks.fetch_add(1, std::memory_order_relaxed);
    if (!idler) {
        return 0;
    }
    const auto* vtable = *reinterpret_cast<const uintptr_t* const*>(idler);
    auto is_async_ready = reinterpret_cast<IdlerBoolMethodFn>(vtable[0x3a0 / 8]);
    if (is_async_ready(idler) == 0) {
        return 0;
    }

    auto is_cam_moving = reinterpret_cast<IdlerBoolMethodFn>(vtable[0x2f0 / 8]);
    bool cam_moving = (is_cam_moving(idler) != 0);

    uint64_t smooth_cfg = g_smooth_fps.load(std::memory_order_relaxed);
    double smooth = static_cast<double>(smooth_cfg > 0 ? smooth_cfg : 30);
    double target_dt = (cam_moving || game_speed <= 1) ? (1.0 / 60.0) : (1.0 / smooth);

    double last_render_dur = static_cast<double>(*reinterpret_cast<const float*>(idler + 0x1c));
    double min_safe = last_render_dur * 1.15;
    double threshold = ((target_dt > min_safe) ? target_dt : min_safe) - 0.001;

    const auto* timer_obj = *reinterpret_cast<const uint8_t* const*>(idler + 0x50);
    if (!timer_obj) {
        return 0;
    }
    auto get_sec = reinterpret_cast<TimerGetSecFn>(g_exe_base + RVA_TIMER_GET_SEC);
    double now_sec = get_sec(timer_obj);
    double last_render_sec = static_cast<double>(*reinterpret_cast<const float*>(idler + 0x2c));

    if ((now_sec - last_render_sec) > threshold) {
        g_mid_tick_renders.fetch_add(1, std::memory_order_relaxed);
        if (tsc_per_ms > 0) {
            g_last_mid_check_tsc.store(__rdtsc() + tsc_per_ms * 8, std::memory_order_relaxed);
        }
        return 1;
    }
    return 0;
}

// ============================================================================
// 7. Lock-Free Zero-Bounce Worker Scheduler (PopTaskOrWait @ 0x255d2a0)
//    + FastTaskWait (0x255afb0) + SwitchToThread IAT Yield Elision (0x27892c0)
// ============================================================================
using PoolWakeFn  = void (*)(const uint8_t* pool, int wake_all);
using EventWaitFn = void (*)(uint8_t* ev);
using SwitchToThreadFn = BOOL (WINAPI*)();

__forceinline size_t swizzle_ring_idx(uint32_t c, uint32_t mask) {
    uint32_t d = c & mask;
    uint32_t a = ((c >> 3) ^ c) & 7U;
    d ^= a;
    return static_cast<size_t>(d ^ (a << 3));
}

__forceinline uintptr_t try_pop_ring(
    std::atomic<int32_t>* head,
    std::atomic<int32_t>* tail,
    std::atomic<uintptr_t>* slots,
    uint32_t mask
) {
    int32_t t = tail->load(std::memory_order_acquire);
    for (;;) {
        int32_t h = head->load(std::memory_order_acquire);
        if (static_cast<int32_t>(static_cast<uint32_t>(h) - static_cast<uint32_t>(t)) <= 0) {
            return 0;
        }
        if (tail->compare_exchange_weak(
                t,
                static_cast<int32_t>(static_cast<uint32_t>(t) + 1U),
                std::memory_order_acq_rel,
                std::memory_order_acquire)) {
            size_t idx = swizzle_ring_idx(static_cast<uint32_t>(t), mask);
            std::atomic<uintptr_t>& slot = slots[idx];
            for (;;) {
                uintptr_t ptr = slot.exchange(0, std::memory_order_acq_rel);
                if (ptr != 0) {
                    return ptr;
                }
                while (slot.load(std::memory_order_acquire) == 0) {
                    _mm_pause();
                }
            }
        }
    }
}

extern "C" uint8_t hook_pop_task_or_wait(uint8_t* worker, uintptr_t* out_task) {
    auto* manager = *reinterpret_cast<uint8_t**>(worker);
    uint32_t prio = *reinterpret_cast<const uint32_t*>(worker + 0x3c);
    size_t bucket = (prio == 2) ? 0 : (prio == 1 ? 1 : (prio == 0 ? 2 : 1));
    size_t bucket_off = bucket * 0x2c0;

    uint8_t* pool = manager + 0x100 + bucket_off;
    auto* running_tasks = reinterpret_cast<std::atomic<int32_t>*>(pool + 0x40);
    auto* searching_workers = reinterpret_cast<std::atomic<int32_t>*>(pool + 0x80);
    auto* global_q = *reinterpret_cast<uint8_t* const*>(pool + 0x298);

    auto* local_head = reinterpret_cast<std::atomic<int32_t>*>(worker + 0x100);
    auto* local_tail = reinterpret_cast<std::atomic<int32_t>*>(worker + 0x140);
    auto* local_slots = reinterpret_cast<std::atomic<uintptr_t>*>(worker + 0x180);

    auto* global_head = reinterpret_cast<std::atomic<int32_t>*>(global_q + 0x80);
    auto* global_tail = reinterpret_cast<std::atomic<int32_t>*>(global_q + 0xc0);
    auto* global_slots = reinterpret_cast<std::atomic<uintptr_t>*>(global_q + 0x100);

    auto* state_ptr = reinterpret_cast<std::atomic<uint8_t>*>(worker + 0x13);
    auto wake_fn = reinterpret_cast<PoolWakeFn>(g_exe_base + RVA_POOL_WAKE_ONE_OR_ALL);
    auto wait_fn = reinterpret_cast<EventWaitFn>(g_exe_base + RVA_EVENT_WAIT_AUTO);

    *out_task = 0;
    searching_workers->fetch_add(1, std::memory_order_seq_cst);

    uint64_t watcher_spins = 0;
    for (;;) {
        if (uintptr_t task = try_pop_ring(local_head, local_tail, local_slots, 0xfffU)) {
            if (watcher_spins > 0) {
                g_sched_watcher_polls.fetch_add(watcher_spins, std::memory_order_relaxed);
            }
            *out_task = task;
            if (searching_workers->fetch_sub(1, std::memory_order_seq_cst) == 1) {
                wake_fn(pool, 0);
            }
            return 1;
        }
        if (uintptr_t task = try_pop_ring(global_head, global_tail, global_slots, 0x7ffffU)) {
            if (watcher_spins > 0) {
                g_sched_watcher_polls.fetch_add(watcher_spins, std::memory_order_relaxed);
            }
            *out_task = task;
            if (searching_workers->fetch_sub(1, std::memory_order_seq_cst) == 1) {
                wake_fn(pool, 0);
            }
            return 1;
        }
        if (state_ptr->load(std::memory_order_acquire) == 2) {
            if (watcher_spins > 0) {
                g_sched_watcher_polls.fetch_add(watcher_spins, std::memory_order_relaxed);
            }
            wake_fn(pool, 1);
            searching_workers->fetch_sub(1, std::memory_order_seq_cst);
            return 0;
        }

        // Designated watcher zero-lock poll while tasks are running
        if (searching_workers->load(std::memory_order_acquire) == 1 &&
            running_tasks->load(std::memory_order_acquire) > 0) {
            ++watcher_spins;
            int pauses = (watcher_spins < 64) ? 4
                         : (watcher_spins < 512) ? 16
                         : (watcher_spins < 4096) ? 64
                                                  : 128;
            for (int i = 0; i < pauses; ++i) {
                _mm_pause();
            }
            continue;
        }

        if (watcher_spins > 0) {
            g_sched_watcher_polls.fetch_add(watcher_spins, std::memory_order_relaxed);
            watcher_spins = 0;
        }

        int32_t prev = searching_workers->fetch_sub(1, std::memory_order_seq_cst);
        if (prev == 1) {
            int32_t rt = running_tasks->load(std::memory_order_seq_cst);
            int32_t g_avail = static_cast<int32_t>(
                static_cast<uint32_t>(global_head->load(std::memory_order_seq_cst)) -
                static_cast<uint32_t>(global_tail->load(std::memory_order_acquire))
            );
            int32_t l_avail = static_cast<int32_t>(
                static_cast<uint32_t>(local_head->load(std::memory_order_seq_cst)) -
                static_cast<uint32_t>(local_tail->load(std::memory_order_acquire))
            );
            if (rt > 0 || g_avail > 0 || l_avail > 0) {
                searching_workers->fetch_add(1, std::memory_order_seq_cst);
                _mm_pause();
                continue;
            }
        }

        uint8_t* sleep_flag = worker + 0x15;
        uint8_t old_sleep = *sleep_flag;
        *sleep_flag = 1;
        auto worker_idx = static_cast<intptr_t>(*reinterpret_cast<const int32_t*>(worker + 0x38));
        auto* events_base = *reinterpret_cast<uint8_t* const*>(pool);
        uint8_t* ev = events_base + worker_idx * WIN_CEVENT_SIZE;
        wait_fn(ev);
        *sleep_flag = old_sleep;

        searching_workers->fetch_add(1, std::memory_order_seq_cst);
    }
}

extern "C" void hook_task_wait(const uint8_t* task) {
    if (!task) {
        return;
    }
    const auto* job = *reinterpret_cast<const uint8_t* const*>(task + 0x18);
    if (!job) {
        return;
    }
    // Verified @ 0x255afbf: job state byte on MSVC x64 is at +0x60
    const auto* st = reinterpret_cast<const std::atomic<uint8_t>*>(job + 0x60);
    uint32_t spins = 0;
    while ((st->load(std::memory_order_acquire) & 0xfeU) != 2U) {
        for (int i = 0; i < 8; ++i) {
            _mm_pause();
        }
        ++spins;
        if ((spins & 0xfffU) == 0) {
            uintptr_t fn = g_orig_switch_to_thread.load(std::memory_order_relaxed);
            if (fn != 0) {
                reinterpret_cast<SwitchToThreadFn>(fn)();
            }
        }
    }
}

static BOOL WINAPI hook_switch_to_thread() {
    uint32_t n = 1;
    if (g_tls_yield_idx != TLS_OUT_OF_INDEXES) {
        n = static_cast<uint32_t>(reinterpret_cast<uintptr_t>(TlsGetValue(g_tls_yield_idx))) + 1U;
        TlsSetValue(g_tls_yield_idx, reinterpret_cast<LPVOID>(static_cast<uintptr_t>(n)));
    }
    if ((n & 0xffU) != 0) {
        for (int i = 0; i < 24; ++i) {
            _mm_pause();
        }
        g_yield_elided.fetch_add(1, std::memory_order_relaxed);
        return FALSE;
    }
    uintptr_t fn = g_orig_switch_to_thread.load(std::memory_order_relaxed);
    return fn ? reinterpret_cast<SwitchToThreadFn>(fn)() : FALSE;
}

// ============================================================================
// 8. Topology-Aware CPU Core Pinning (WorkerThreadInit @ 0x255a110)
//    Automatically detects physical cores vs SMT siblings via Win32 API and
//    gracefully skips asymmetric Intel Hybrid (P+E) layouts so Windows 11
//    Thread Director remains in charge on hybrid CPUs.
// ============================================================================
static bool init_cpu_topology() {
    DWORD byte_len = 0;
    GetLogicalProcessorInformation(nullptr, &byte_len);
    if (byte_len == 0 || byte_len > 65536) {
        return false;
    }
    auto* buf = static_cast<SYSTEM_LOGICAL_PROCESSOR_INFORMATION*>(
        HeapAlloc(GetProcessHeap(), 0, byte_len)
    );
    if (!buf) {
        return false;
    }
    if (!GetLogicalProcessorInformation(buf, &byte_len)) {
        HeapFree(GetProcessHeap(), 0, buf);
        return false;
    }

    uint32_t primaries[64] = {0};
    size_t n_prim = 0;
    uint32_t siblings[64] = {0};
    size_t n_sib = 0;
    int32_t main_sibling = -1;
    int expected_threads_per_core = 0;
    bool hybrid_detected = false;

    size_t count = byte_len / sizeof(SYSTEM_LOGICAL_PROCESSOR_INFORMATION);
    for (size_t i = 0; i < count; ++i) {
        if (buf[i].Relationship != RelationProcessorCore) {
            continue;
        }
        ULONG_PTR mask = buf[i].ProcessorMask;
        if (mask == 0) {
            continue;
        }
        int pop = static_cast<int>(__popcnt64(static_cast<uint64_t>(mask)));
        if (expected_threads_per_core == 0) {
            expected_threads_per_core = pop;
        } else if (pop != expected_threads_per_core) {
            hybrid_detected = true;
            break;
        }

        int first_cpu = -1;
        for (uint32_t c = 0; c < 64; ++c) {
            if ((mask & (static_cast<ULONG_PTR>(1) << c)) == 0) {
                continue;
            }
            if (first_cpu < 0) {
                first_cpu = static_cast<int>(c);
                if (c != 0 && n_prim < 64) {
                    primaries[n_prim++] = c;
                }
            } else {
                if (first_cpu == 0) {
                    main_sibling = static_cast<int32_t>(c);
                } else if (n_sib < 64) {
                    siblings[n_sib++] = c;
                }
            }
        }
    }
    HeapFree(GetProcessHeap(), 0, buf);

    if (hybrid_detected || n_prim < 3) {
        return false;
    }

    g_topo_worker_len = 0;
    for (size_t i = 0; i < n_prim && g_topo_worker_len < 64; ++i) {
        g_topo_worker_cpus[g_topo_worker_len++] = primaries[i];
    }
    for (size_t i = 0; i < n_sib && g_topo_worker_len < 64; ++i) {
        g_topo_worker_cpus[g_topo_worker_len++] = siblings[i];
    }
    if (main_sibling >= 0 && g_topo_worker_len < 64) {
        g_topo_worker_cpus[g_topo_worker_len++] = static_cast<uint32_t>(main_sibling);
    }

    g_topo_bg_len = 0;
    for (size_t i = 0; i < n_sib && g_topo_bg_len < 64; ++i) {
        g_topo_bg_cpus[g_topo_bg_len++] = siblings[i];
    }
    if (main_sibling >= 0 && g_topo_bg_len < 64) {
        g_topo_bg_cpus[g_topo_bg_len++] = static_cast<uint32_t>(main_sibling);
    }
    if (g_topo_bg_len == 0) {
        for (size_t i = n_prim; i > 0 && g_topo_bg_len < 64; --i) {
            g_topo_bg_cpus[g_topo_bg_len++] = primaries[i - 1];
        }
    }
    return g_topo_worker_len > 0;
}

extern "C" void hook_worker_thread_init(int prio) {
    HANDLE cur = GetCurrentThread();
    if (prio == 2) {
        SetThreadPriority(cur, THREAD_PRIORITY_ABOVE_NORMAL);
    } else if (prio == 1) {
        SetThreadPriority(cur, THREAD_PRIORITY_NORMAL);
    } else if (prio == 0) {
        SetThreadPriority(cur, THREAD_PRIORITY_BELOW_NORMAL);
    }

    if (g_affinity_enabled.load(std::memory_order_relaxed)) {
        if (prio == 2 && g_topo_worker_len > 0) {
            size_t slot = g_worker_pin_counter.fetch_add(1, std::memory_order_relaxed);
            uint32_t cpu = g_topo_worker_cpus[slot % g_topo_worker_len];
            SetThreadAffinityMask(cur, static_cast<DWORD_PTR>(1ULL << cpu));
        } else if (prio == 0 && g_topo_bg_len > 0) {
            size_t slot = g_bg_pin_counter.fetch_add(1, std::memory_order_relaxed);
            uint32_t cpu = g_topo_bg_cpus[slot % g_topo_bg_len];
            SetThreadAffinityMask(cur, static_cast<DWORD_PTR>(1ULL << cpu));
        }
    }
}

// ============================================================================
// 9. QuietLog (MSVC 32-byte SSO std::string), FastSave & NoTelemetry (MSVC ABI)
// ============================================================================
struct MsvcString {
    union {
        char buf[16];
        const char* ptr;
    } u;
    uint64_t size;
    uint64_t cap;

    const char* data() const { return (cap > 15ULL) ? u.ptr : u.buf; }
};

using LoggerDeletingDtorFn = void (*)(uint8_t* self, uint32_t flags);
using LoggerWriteFn = void (*)(
    uint8_t* self,
    int level,
    const MsvcString* file,
    int line,
    const MsvcString* msg
);

static uint64_t fnv1a_log_sig(
    int level,
    const MsvcString* file,
    const MsvcString* msg
) {
    uint64_t h = 0xcbf29ce484222325ULL;
    auto mix = [&](uint8_t b) {
        h ^= static_cast<uint64_t>(b);
        h *= 0x100000001b3ULL;
    };
    for (int i = 0; i < 4; ++i) {
        mix(static_cast<uint8_t>((static_cast<uint32_t>(level) >> (i * 8)) & 0xffU));
    }
    // NOTE: Do NOT hash `line` — HoI4 passes script line numbers (e.g. 1..87000+ in RU_loc.txt)
    // in `line`, which would explode unique patterns and overflow the dedup table!
    if (file && file->size > 0) {
        const char* p = file->data();
        if (p) {
            size_t n = (file->size < 128) ? static_cast<size_t>(file->size) : 128;
            size_t kept = 0;
            for (size_t i = 0; i < n; ++i) {
                uint8_t c = static_cast<uint8_t>(p[i]);
                if (c >= '0' && c <= '9') {
                    continue;
                }
                mix(c);
                if (++kept >= 64) {
                    break;
                }
            }
        }
    }
    if (msg && msg->size > 0) {
        const char* p = msg->data();
        if (p) {
            size_t n = (msg->size < 160) ? static_cast<size_t>(msg->size) : 160;
            size_t kept = 0;
            for (size_t i = 0; i < n; ++i) {
                uint8_t c = static_cast<uint8_t>(p[i]);
                if (c >= '0' && c <= '9') {
                    continue;
                }
                mix(c);
                if (++kept >= 96) {
                    break;
                }
            }
        }
    }
    return (h == 0) ? 1ULL : h;
}

static bool should_allow_log(uint64_t sig, int level) {
    static constexpr uint32_t MAX_PER_PATTERN = 8;
    size_t idx = static_cast<size_t>(sig) & (LOG_DEDUP_CAP - 1);
    for (size_t probe = 0; probe < 32; ++probe) {
        LogDedupSlot& slot = g_log_dedup[idx];
        uint64_t existing = slot.hash.load(std::memory_order_relaxed);
        uint64_t zero = 0;
        if (existing == sig ||
            (existing == 0 &&
             slot.hash.compare_exchange_strong(
                 zero, sig, std::memory_order_relaxed, std::memory_order_relaxed))) {
            uint32_t prev = slot.count.fetch_add(1, std::memory_order_relaxed);
            return prev < MAX_PER_PATTERN;
        }
        idx = (idx + 1) & (LOG_DEDUP_CAP - 1);
    }
    return level < 3;
}

extern "C" void hook_logger_write(
    uint8_t* self,
    int level,
    const MsvcString* file,
    int line,
    const MsvcString* msg
) {
    if (!self) {
        return;
    }
    uintptr_t vtable = *reinterpret_cast<const uintptr_t*>(self);
    uintptr_t orig_fn = 0;
    for (size_t i = 0; i < LOG_VT_CAP; ++i) {
        if (g_log_vt_keys[i].load(std::memory_order_acquire) == vtable) {
            orig_fn = g_log_vt_orig[i].load(std::memory_order_relaxed);
            break;
        }
    }
    if (orig_fn == 0) {
        return;
    }

    if (g_in_civil_war_depth.load(std::memory_order_relaxed) > 0 && level >= 2) {
        g_log_suppressed.fetch_add(1, std::memory_order_relaxed);
        return;
    }

    uint64_t sig = fnv1a_log_sig(level, file, msg);
    if (!should_allow_log(sig, level)) {
        g_log_suppressed.fetch_add(1, std::memory_order_relaxed);
        return;
    }

    g_log_allowed.fetch_add(1, std::memory_order_relaxed);
    reinterpret_cast<LoggerWriteFn>(orig_fn)(self, level, file, line, msg);
}

static void wrap_logger_vtable(uint8_t* logger) {
    if (!logger) {
        return;
    }
    uintptr_t vtable = *reinterpret_cast<const uintptr_t*>(logger);
    if (vtable == 0) {
        return;
    }
    // Verified @ 0x254f25f: CLogger::Write is slot 1 (+0x08) on MSVC x64
    uintptr_t write_slot_addr = vtable + 0x08;
    uintptr_t orig_write = *reinterpret_cast<const uintptr_t*>(write_slot_addr);
    auto hook_addr = reinterpret_cast<uintptr_t>(&hook_logger_write);
    if (orig_write == 0 || orig_write == hook_addr) {
        return;
    }
    for (size_t i = 0; i < LOG_VT_CAP; ++i) {
        uintptr_t cur = g_log_vt_keys[i].load(std::memory_order_acquire);
        if (cur == vtable) {
            return;
        }
        uintptr_t zero = 0;
        if (cur == 0 &&
            g_log_vt_keys[i].compare_exchange_strong(
                zero, vtable, std::memory_order_acq_rel, std::memory_order_acquire)) {
            g_log_vt_orig[i].store(orig_write, std::memory_order_release);
            patch_data_ptr(write_slot_addr, hook_addr);
            return;
        }
    }
}

extern "C" void hook_set_logger(uint8_t* new_logger) {
    if (new_logger) {
        wrap_logger_vtable(new_logger);
    }
    auto* slot = reinterpret_cast<uint8_t**>(g_exe_base + RVA_G_LOGGER_PTR);
    uint8_t* old = *slot;
    *slot = new_logger;
    if (old && old != new_logger) {
        const auto* vt = *reinterpret_cast<const uintptr_t* const*>(old);
        if (vt && vt[0] != 0) {
            // Verified @ 0x2550066: scalar deleting destructor at slot 0 with flags=1
            reinterpret_cast<LoggerDeletingDtorFn>(vt[0])(old, 1);
        }
    }
}

extern "C" int hook_fast_flush(void* /*file_handle*/) {
    g_fsync_skipped.fetch_add(1, std::memory_order_relaxed);
    return 1;
}

using TelemetryPayloadDtorFn = void (*)(void* payload);

extern "C" void hook_queue_and_send_telemetry(void* /*api*/, void* payload) {
    // MSVC x64 ABI: callee MUST destroy by-value C++ objects!
    if (payload && g_exe_base != 0) {
        uintptr_t dtor_addr = *reinterpret_cast<const uintptr_t*>(
            g_exe_base + RVA_IAT_TELEM_PAYLOAD_DTOR
        );
        if (dtor_addr != 0) {
            reinterpret_cast<TelemetryPayloadDtorFn>(dtor_addr)(payload);
        }
    }
    g_telemetry_blocked.fetch_add(1, std::memory_order_relaxed);
}

// ============================================================================
// 9B. v1.0 Deep Engine & AI Modules (MSVC x64 ABI Verified)
//     - CountryGrain     : CPdxParallelForRange::Init (0x2576ac0, returns `this`)
//     - FastModifier     : Branchless CMOV CModifier::GetValue (0x5a63d0, hidden out_fp in %rdx)
//     - FastAlloc        : 128 MB lock-free ABA-safe slab + Win32 TLS magazine (0x2798570)
//     - CivilWarFix      : Coalesces 100+ per-state CInGameIdler map/supply rebuilds (0x1169250)
//     - AiSmartSupply    : Mod-safe NDefines clamping for proactive supply hubs & railways
//     - AiThrottleMinors : Staggered 4h cadence strictly for peaceful <12-state non-major AI minors
// ============================================================================
using ParforRangeInitFn = uint8_t* (*)(
    uint8_t* self,
    int start,
    int end,
    int mode,
    int max_threads,
    const uint8_t* pool_pair
);

extern "C" uint8_t* hook_parfor_range_init(
    uint8_t* self,
    int start,
    int end,
    int mode,
    int max_threads,
    const uint8_t* pool_pair
) {
    auto orig = reinterpret_cast<ParforRangeInitFn>(
        g_orig_parfor_range_init.load(std::memory_order_relaxed)
    );
    uint8_t* res = orig(self, start, end, mode, max_threads, pool_pair);
    g_parfor_total.fetch_add(1, std::memory_order_relaxed);
    if (g_country_grain_enabled.load(std::memory_order_relaxed) && self != nullptr) {
        int n = end - start;
        if (n >= 17 && n <= 220) {
            auto* chunk_ptr = reinterpret_cast<int32_t*>(self + 0x18);
            if (*chunk_ptr > 1) {
                *chunk_ptr = 1;
                g_parfor_rebalanced.fetch_add(1, std::memory_order_relaxed);
            }
        }
    }
    return res;
}

struct ModifierEntry {
    uint32_t key;
    uint32_t pad;
    int64_t value;
};
using FeatureMaskFn = uint32_t (*)();

extern "C" int64_t* hook_modifier_get_value(
    const uint8_t* arr,
    int64_t* out_fp,
    uint32_t key
) {
    int32_t len = *reinterpret_cast<const int32_t*>(arr + 0x0c);
    if (len <= 0) {
        *out_fp = 0;
        return out_fp;
    }
    const auto* base = *reinterpret_cast<const ModifierEntry* const*>(arr);
    auto n = static_cast<size_t>(len);

    // Guaranteed branchless CMOV binary search (zero branch mispredictions on Zen/Core)
    while (n > 1) {
        size_t half = n >> 1;
        const ModifierEntry* mid = base + half;
        uint32_t mid_key = mid->key;
        __asm__(
            "cmpl %[key], %[mid_key]\n\t"
            "cmovbeq %[mid], %[base]"
            : [base] "+r"(base)
            : [mid_key] "r"(mid_key), [key] "r"(key), [mid] "r"(mid)
            : "cc"
        );
        n -= half;
    }

    if (base->key != key) {
        *out_fp = 0;
        return out_fp;
    }

    const auto* defs = *reinterpret_cast<const uint8_t* const*>(g_exe_base + RVA_G_MODIFIER_DEFS);
    const uint8_t* def = defs + static_cast<intptr_t>(static_cast<int32_t>(key)) * 0x78;
    uint32_t mask = *reinterpret_cast<const uint32_t*>(def + 0x68);
    if (mask != 0) {
        uintptr_t fn_ptr = *reinterpret_cast<const uintptr_t*>(g_exe_base + RVA_G_FEATURE_FN);
        if (fn_ptr == 0 || (reinterpret_cast<FeatureMaskFn>(fn_ptr)() & mask) == 0) {
            *out_fp = 0;
            return out_fp;
        }
    }

    int64_t val = base->value;
    if ((def[0x60] & 0x10U) != 0) {
        if (val < 0) val = 0;
        else if (val > 100000LL) val = 100000LL;
    }
    *out_fp = val;
    return out_fp;
}

// --- Lock-Free ABA-Safe Slab + Win32 TLS Magazine Allocator ---
static constexpr size_t   SLAB_ARENA_SIZE = 128ULL * 1024ULL * 1024ULL;
static constexpr size_t   SLAB_PAGE_SIZE  = 65536ULL;
static constexpr uint32_t SLAB_MAGIC      = 0x484f4934U;
static constexpr uint64_t SLAB_OFF_MASK   = 0x0fffffffULL;
static constexpr uint64_t SLAB_GEN_INC    = 1ULL << 28;
static constexpr size_t   SLAB_CLASS_BYTES[4] = {64, 128, 256, 512};
static constexpr size_t   TLS_MAG_CAP     = 64;
static constexpr size_t   MAX_TLS_MAGS    = 256;

struct alignas(64) SlabPageHdr {
    uint32_t magic;
    uint32_t class_idx;
    uint64_t pad[7];
};

struct TlsSlabMag {
    size_t counts[4];
    uint8_t* slots[4][TLS_MAG_CAP];
};

static std::atomic<uintptr_t> g_slab_arena_base{0};
static std::atomic<size_t>    g_slab_bump_off{0};
static std::atomic<uint64_t>  g_slab_global_free[4];
static std::atomic<size_t>    g_tls_mag_pool_next{0};
static TlsSlabMag             g_tls_mag_pool[MAX_TLS_MAGS];

static bool init_slab_arena() {
    void* raw = VirtualAlloc(
        nullptr,
        SLAB_ARENA_SIZE + SLAB_PAGE_SIZE,
        MEM_RESERVE,
        PAGE_NOACCESS
    );
    if (!raw) {
        return false;
    }
    uintptr_t aligned = (reinterpret_cast<uintptr_t>(raw) + (SLAB_PAGE_SIZE - 1)) &
                        ~(SLAB_PAGE_SIZE - 1);
    g_slab_bump_off.store(0, std::memory_order_release);
    g_slab_arena_base.store(aligned, std::memory_order_release);
    return true;
}

__forceinline void slab_push_global(uintptr_t arena, size_t cls, uint8_t* ptr) {
    auto off = static_cast<uint32_t>(reinterpret_cast<uintptr_t>(ptr) - arena);
    auto& list = g_slab_global_free[cls];
    uint64_t head = list.load(std::memory_order_acquire);
    for (;;) {
        *reinterpret_cast<uint32_t*>(ptr) = static_cast<uint32_t>(head & SLAB_OFF_MASK);
        uint64_t next_head = ((head & ~SLAB_OFF_MASK) + SLAB_GEN_INC) | static_cast<uint64_t>(off);
        if (list.compare_exchange_weak(
                head, next_head, std::memory_order_acq_rel, std::memory_order_acquire)) {
            break;
        }
    }
}

__forceinline uint8_t* slab_pop_global(uintptr_t arena, size_t cls) {
    auto& list = g_slab_global_free[cls];
    uint64_t head = list.load(std::memory_order_acquire);
    for (;;) {
        auto off = static_cast<size_t>(head & SLAB_OFF_MASK);
        if (off == 0) {
            return nullptr;
        }
        auto* ptr = reinterpret_cast<uint8_t*>(arena + off);
        uint64_t next_off = static_cast<uint64_t>(*reinterpret_cast<const uint32_t*>(ptr));
        uint64_t next_head = ((head & ~SLAB_OFF_MASK) + SLAB_GEN_INC) | next_off;
        if (list.compare_exchange_weak(
                head, next_head, std::memory_order_acq_rel, std::memory_order_acquire)) {
            return ptr;
        }
    }
}

static uint8_t* slab_carve_page(uintptr_t arena, size_t cls, TlsSlabMag* mag) {
    size_t cur = g_slab_bump_off.load(std::memory_order_relaxed);
    size_t page_off = 0;
    for (;;) {
        if (cur + SLAB_PAGE_SIZE > SLAB_ARENA_SIZE) {
            return nullptr;
        }
        if (g_slab_bump_off.compare_exchange_weak(
                cur,
                cur + SLAB_PAGE_SIZE,
                std::memory_order_acq_rel,
                std::memory_order_relaxed)) {
            page_off = cur;
            break;
        }
    }

    auto* page_ptr = reinterpret_cast<uint8_t*>(arena + page_off);
    if (!VirtualAlloc(page_ptr, SLAB_PAGE_SIZE, MEM_COMMIT, PAGE_READWRITE)) {
        return nullptr;
    }

    auto* hdr = reinterpret_cast<SlabPageHdr*>(page_ptr);
    hdr->magic = SLAB_MAGIC;
    hdr->class_idx = static_cast<uint32_t>(cls);

    size_t bsize = SLAB_CLASS_BYTES[cls];
    uint8_t* first_block = page_ptr + 64;
    size_t off = 64 + bsize;
    if (mag != nullptr) {
        while (off + bsize <= SLAB_PAGE_SIZE && mag->counts[cls] < (TLS_MAG_CAP / 2)) {
            mag->slots[cls][mag->counts[cls]++] = page_ptr + off;
            off += bsize;
        }
    }
    while (off + bsize <= SLAB_PAGE_SIZE) {
        slab_push_global(arena, cls, page_ptr + off);
        off += bsize;
    }
    return first_block;
}

__forceinline TlsSlabMag* get_tls_slab_mag() {
    if (g_tls_slab_idx == TLS_OUT_OF_INDEXES) {
        return nullptr;
    }
    auto* mag = static_cast<TlsSlabMag*>(TlsGetValue(g_tls_slab_idx));
    if (!mag) {
        size_t idx = g_tls_mag_pool_next.fetch_add(1, std::memory_order_relaxed);
        if (idx < MAX_TLS_MAGS) {
            mag = &g_tls_mag_pool[idx];
            TlsSetValue(g_tls_slab_idx, mag);
        }
    }
    return mag;
}

using PdxAllocFn = void* (*)(void* self, size_t size, size_t align);
using PdxFreeFn  = void (*)(void* self, void* ptr);

extern "C" void* hook_pdx_alloc(void* self, size_t size, size_t align) {
    if (align <= 16 && size <= 512) {
        uintptr_t arena = g_slab_arena_base.load(std::memory_order_relaxed);
        if (arena != 0) {
            size_t cls = (size <= 64) ? 0 : (size <= 128) ? 1 : (size <= 256) ? 2 : 3;
            TlsSlabMag* mag = get_tls_slab_mag();
            uint8_t* block = nullptr;
            if (mag) {
                size_t n = mag->counts[cls];
                if (n > 0) {
                    mag->counts[cls] = n - 1;
                    block = mag->slots[cls][n - 1];
                } else {
                    block = slab_pop_global(arena, cls);
                    if (block) {
                        for (int i = 0; i < 15; ++i) {
                            uint8_t* extra = slab_pop_global(arena, cls);
                            if (!extra) break;
                            mag->slots[cls][mag->counts[cls]++] = extra;
                        }
                    } else {
                        block = slab_carve_page(arena, cls, mag);
                    }
                }
            } else {
                block = slab_pop_global(arena, cls);
                if (!block) {
                    block = slab_carve_page(arena, cls, nullptr);
                }
            }
            if (block) {
                g_slab_alloc_hits.fetch_add(1, std::memory_order_relaxed);
                return block;
            }
        }
    }
    g_slab_fallbacks.fetch_add(1, std::memory_order_relaxed);
    return reinterpret_cast<PdxAllocFn>(
        g_orig_pdx_alloc.load(std::memory_order_relaxed)
    )(self, size, align);
}

extern "C" void hook_pdx_free(void* self, void* ptr) {
    if (!ptr) {
        return;
    }
    uintptr_t arena = g_slab_arena_base.load(std::memory_order_relaxed);
    uintptr_t uptr = reinterpret_cast<uintptr_t>(ptr);
    if (arena != 0 && (uptr - arena) < SLAB_ARENA_SIZE) {
        const auto* hdr = reinterpret_cast<const SlabPageHdr*>(uptr & ~(SLAB_PAGE_SIZE - 1));
        if (hdr->magic == SLAB_MAGIC && hdr->class_idx < 4) {
            size_t cls = hdr->class_idx;
            g_slab_free_hits.fetch_add(1, std::memory_order_relaxed);
            TlsSlabMag* mag = get_tls_slab_mag();
            auto* bptr = static_cast<uint8_t*>(ptr);
            if (mag) {
                size_t n = mag->counts[cls];
                if (n >= TLS_MAG_CAP) {
                    size_t keep = TLS_MAG_CAP / 2;
                    for (size_t i = keep; i < n; ++i) {
                        slab_push_global(arena, cls, mag->slots[cls][i]);
                    }
                    n = keep;
                }
                mag->slots[cls][n] = bptr;
                mag->counts[cls] = n + 1;
            } else {
                slab_push_global(arena, cls, bptr);
            }
            return;
        }
    }
    reinterpret_cast<PdxFreeFn>(g_orig_pdx_free.load(std::memory_order_relaxed))(self, ptr);
}

// --- Civil War Freeze Fix ---
using IdlerMapRebuildFn = void (*)(uint8_t* self);
using CwSetupExecFn     = void (*)(uint8_t* self, uint8_t* out_tag, uint8_t flag);

extern "C" void hook_idler_map_rebuild(uint8_t* self) {
    if (g_in_civil_war_depth.load(std::memory_order_acquire) > 0) {
        if (self != nullptr) {
            g_deferred_cw_idler.store(
                reinterpret_cast<uintptr_t>(self),
                std::memory_order_release
            );
        }
        g_cw_rebuilds_coalesced.fetch_add(1, std::memory_order_relaxed);
        return;
    }
    reinterpret_cast<IdlerMapRebuildFn>(
        g_orig_idler_map_rebuild.load(std::memory_order_relaxed)
    )(self);
}

extern "C" void hook_cw_setup_exec(uint8_t* self, uint8_t* out_tag, uint8_t flag) {
    g_in_civil_war_depth.fetch_add(1, std::memory_order_acq_rel);
    reinterpret_cast<CwSetupExecFn>(
        g_orig_cw_setup_exec.load(std::memory_order_relaxed)
    )(self, out_tag, flag);

    if (g_in_civil_war_depth.fetch_sub(1, std::memory_order_acq_rel) == 1) {
        uintptr_t deferred_idler = g_deferred_cw_idler.exchange(0, std::memory_order_acq_rel);
        uintptr_t orig_rebuild = g_orig_idler_map_rebuild.load(std::memory_order_relaxed);
        if (deferred_idler != 0 && orig_rebuild != 0) {
            reinterpret_cast<IdlerMapRebuildFn>(orig_rebuild)(
                reinterpret_cast<uint8_t*>(deferred_idler)
            );
        }
    }
}

// --- v1.0 -> v1.3 Smart AI Supply, Railways & Native Wartime Land `supply_node` Auto-Builder ---
using FixSupplyBottlenecksFn = void (*)(uint8_t* ai_constr);
using BaseAiResolveWinFn     = uint8_t* (*)(uint8_t* obj);
using BaseAiGetTagWinFn      = const uint32_t* (*)(uint8_t* base_ai);
using GsGetProvinceWinFn     = const uint8_t* (*)(const uint8_t* gs_sub, int32_t id);
using BldgContCtrlTagWinFn   = const uint32_t* (*)(const uint8_t* container);
using BldgContCanConstrWinFn = uint8_t (*)(const uint8_t* container, const uint8_t* bldg, uint8_t flag);
using BldgLevelInitWinFn     = void (*)(uint64_t* out_handle, const uint8_t* bldg, const uint8_t* container);
using BldgLevelGetWinFn      = int32_t (*)(uint64_t* handle);
using SBldgLocCtorWinFn      = uint64_t* (*)(const uint8_t* container, uint64_t* loc_buf, const uint8_t* bldg);
using OpNewWinFn             = uint8_t* (*)(size_t bytes);
using AddConstrCmdCtorWinFn  = uint8_t* (*)(
    uint8_t* cmd, uint8_t flag, const uint32_t* tag_ptr, const uint64_t* loc_buf, int32_t count
);
using SessionPostWinFn       = void (*)(uint8_t* cmd, const uint32_t* tag_ptr);

static void resolve_supply_buildings_once() {
    bool expected = false;
    if (!g_bldg_db_resolved.compare_exchange_strong(
            expected, true, std::memory_order_acq_rel, std::memory_order_relaxed)) {
        return;
    }
    if (g_exe_base == 0) {
        g_bldg_db_resolved.store(false, std::memory_order_release);
        return;
    }
    const auto* db = *reinterpret_cast<const uint8_t* const*>(g_exe_base + RVA_BLDG_DB_PTR);
    if (!db) {
        g_bldg_db_resolved.store(false, std::memory_order_release);
        return;
    }
    const auto* names = *reinterpret_cast<const uint8_t* const*>(db + 0x58);
    int32_t names_len = *reinterpret_cast<const int32_t*>(db + 0x64);
    const auto* bldgs = *reinterpret_cast<const uint8_t* const* const*>(db + 0x40);
    int32_t bldgs_len = *reinterpret_cast<const int32_t*>(db + 0x4c);
    if (!names || !bldgs || names_len <= 0 || bldgs_len <= 1) {
        g_bldg_db_resolved.store(false, std::memory_order_release);
        return;
    }
    size_t cap = (names_len < 128) ? static_cast<size_t>(names_len) : 128;
    auto bcap = static_cast<size_t>(bldgs_len);
    for (size_t i = 0; i < cap; ++i) {
        if (i + 1 >= bcap) break;
        const auto* ms = reinterpret_cast<const MsvcString*>(names + i * 0x28);
        if (ms->size == 0 || ms->size > 64) continue;
        const char* p = ms->data();
        if (!p) continue;
        uintptr_t bptr = reinterpret_cast<uintptr_t>(bldgs[i + 1]);
        if (bptr == 0) continue;
        if (ms->size == 11 && std::memcmp(p, "supply_node", 11) == 0) {
            g_supply_node_bldg_ptr.store(bptr, std::memory_order_release);
        } else if (ms->size == 10 && std::memcmp(p, "naval_base", 10) == 0) {
            g_naval_base_bldg_ptr.store(bptr, std::memory_order_release);
        }
    }
}

__forceinline int32_t get_bldg_level_in_prov(const uint8_t* bldg, const uint8_t* container) {
    if (!bldg || !container || g_exe_base == 0) return 0;
    auto init_fn = reinterpret_cast<BldgLevelInitWinFn>(g_exe_base + RVA_BLDG_LEVEL_INIT);
    auto get_fn  = reinterpret_cast<BldgLevelGetWinFn>(g_exe_base + RVA_BLDG_LEVEL_GET);
    uint64_t handle[4] = {0, 0, 0, 0};
    init_fn(handle, bldg, container);
    return get_fn(handle);
}

static bool prov_borders_non_own_land(
    const uint8_t* gs_sub,
    GsGetProvinceWinFn get_prov,
    const uint8_t* prov,
    uint32_t tag_id
) {
    const auto* sp = *reinterpret_cast<const uint8_t* const*>(prov + OFF_PROV_STATIC_DATA);
    if (!sp) return false;
    const auto* adj_ptr = *reinterpret_cast<const uint8_t* const*>(sp + 0x70);
    int32_t adj_len = *reinterpret_cast<const int32_t*>(sp + 0x7c);
    if (!adj_ptr || adj_len <= 0) return false;
    auto get_ctrl = reinterpret_cast<BldgContCtrlTagWinFn>(g_exe_base + RVA_BLDG_CONT_CTRL_TAG);
    int32_t cap = (adj_len < 32) ? adj_len : 32;
    for (int32_t j = 0; j < cap; ++j) {
        const uint8_t* adj = adj_ptr + static_cast<size_t>(j) * 0x30;
        if (adj[0x10] == 4) continue;
        int32_t nb_id = *reinterpret_cast<const int32_t*>(adj + 0x08);
        if (nb_id <= 0) continue;
        const uint8_t* nb_p = get_prov(gs_sub, nb_id);
        if (!nb_p) continue;
        const auto* nb_sp = *reinterpret_cast<const uint8_t* const*>(nb_p + OFF_PROV_STATIC_DATA);
        if (!nb_sp || (nb_sp[0xd2] & 1U) == 0) continue;
        const uint32_t* nb_ctrl = get_ctrl(nb_p + OFF_PROV_BLDG_CONTAINER);
        if (nb_ctrl && *nb_ctrl != 0 && *nb_ctrl != tag_id) {
            return true;
        }
    }
    return false;
}

static bool has_hub_or_port_within_3_hops(
    const uint8_t* gs_sub,
    GsGetProvinceWinFn get_prov,
    const uint8_t* start_prov,
    const uint8_t* supply_node,
    const uint8_t* naval_base
) {
    int32_t visited[64];
    size_t vis_len = 0;
    const uint8_t* queue_p[64];
    uint8_t queue_d[64];
    size_t q_head = 0, q_tail = 0;

    visited[vis_len++] = *reinterpret_cast<const int32_t*>(start_prov + OFF_PROV_ID);
    queue_p[q_tail] = start_prov;
    queue_d[q_tail++] = 0;

    while (q_head < q_tail) {
        const uint8_t* cur_p = queue_p[q_head];
        uint8_t cur_d = queue_d[q_head++];

        const uint8_t* cont = cur_p + OFF_PROV_BLDG_CONTAINER;
        if (get_bldg_level_in_prov(supply_node, cont) > 0) return true;
        if (naval_base && get_bldg_level_in_prov(naval_base, cont) > 0) return true;
        if (cur_d >= 3) continue;

        const auto* sp = *reinterpret_cast<const uint8_t* const*>(cur_p + OFF_PROV_STATIC_DATA);
        if (!sp) continue;
        const auto* adj_ptr = *reinterpret_cast<const uint8_t* const*>(sp + 0x70);
        int32_t adj_len = *reinterpret_cast<const int32_t*>(sp + 0x7c);
        if (!adj_ptr || adj_len <= 0) continue;
        int32_t cap = (adj_len < 32) ? adj_len : 32;
        for (int32_t j = 0; j < cap; ++j) {
            const uint8_t* adj = adj_ptr + static_cast<size_t>(j) * 0x30;
            if (adj[0x10] == 4) continue;
            int32_t nb_id = *reinterpret_cast<const int32_t*>(adj + 0x08);
            if (nb_id <= 0) continue;
            bool seen = false;
            for (size_t k = 0; k < vis_len; ++k) {
                if (visited[k] == nb_id) { seen = true; break; }
            }
            if (seen || vis_len >= 64 || q_tail >= 64) continue;
            const uint8_t* nb_p = get_prov(gs_sub, nb_id);
            if (!nb_p) continue;
            const auto* nb_sp = *reinterpret_cast<const uint8_t* const*>(nb_p + OFF_PROV_STATIC_DATA);
            if (!nb_sp || (nb_sp[0xd2] & 1U) == 0) continue;
            visited[vis_len++] = nb_id;
            queue_p[q_tail] = nb_p;
            queue_d[q_tail++] = static_cast<uint8_t>(cur_d + 1);
        }
    }
    return false;
}

static void try_queue_wartime_supply_hub(uint8_t* ai_constr) {
    if (g_exe_base == 0 || !ai_constr) return;
    const auto* supply_node = reinterpret_cast<const uint8_t*>(
        g_supply_node_bldg_ptr.load(std::memory_order_acquire)
    );
    if (!supply_node) return;
    const auto* naval_base = reinterpret_cast<const uint8_t*>(
        g_naval_base_bldg_ptr.load(std::memory_order_acquire)
    );

    const auto* gs = *reinterpret_cast<const uint8_t* const*>(g_exe_base + RVA_GAMESTATE_PTR);
    if (!gs) return;
    const uint8_t* gs_sub = gs + 0x08;
    const auto* gs_sub_vt = *reinterpret_cast<const uintptr_t* const*>(gs_sub);
    if (!gs_sub_vt || gs_sub_vt[0] == 0) return;
    auto get_prov = reinterpret_cast<GsGetProvinceWinFn>(gs_sub_vt[0]);

    auto* obj = *reinterpret_cast<uint8_t* const*>(ai_constr + 0x08);
    if (!obj) return;
    const auto* obj_vt = *reinterpret_cast<const uintptr_t* const*>(obj);
    if (!obj_vt || obj_vt[0x08 / 8] == 0) return;
    uint8_t* base_ai = reinterpret_cast<BaseAiResolveWinFn>(obj_vt[0x08 / 8])(obj);
    if (!base_ai) return;

    const uint32_t* tag_ptr = reinterpret_cast<BaseAiGetTagWinFn>(g_exe_base + RVA_BASE_AI_GET_TAG)(base_ai);
    uint8_t* country = reinterpret_cast<BaseAiResolveWinFn>(g_exe_base + RVA_BASE_AI_GET_COUNTRY)(base_ai);
    if (!tag_ptr || !country) return;
    uint32_t tag_id = *tag_ptr;
    if (tag_id == 0) return;

    using WinHasHumanFn = uint8_t (*)(uint8_t*);
    using WinIsAtWarFn  = uint8_t (*)(const uint8_t*, uint8_t);
    if (reinterpret_cast<WinHasHumanFn>(g_exe_base + RVA_COUNTRY_HAS_HUMAN)(country) != 0) return;
    const auto* diplo = *reinterpret_cast<const uint8_t* const*>(country + OFF_COUNTRY_DIPLO_PTR);
    if (!diplo) return;
    if (reinterpret_cast<WinIsAtWarFn>(g_exe_base + RVA_DIPLO_IS_AT_WAR)(diplo, 0) == 0) return;

    const auto* constr = *reinterpret_cast<const uint8_t* const*>(country + OFF_COUNTRY_CONSTR_PTR);
    if (!constr) return;
    int64_t total_fac = *reinterpret_cast<const int64_t*>(constr + 0x2d0) / 100000LL;
    if (total_fac < 20) return;

    int32_t raw_date = *reinterpret_cast<const int32_t*>(gs + OFF_GS_CDATE_HOURS);
    auto cur_hours = static_cast<uint32_t>(raw_date - 0x029c55c0);
    auto& cd_slot = g_supply_hub_last_hr[static_cast<size_t>(tag_id) & 511U];
    uint32_t last_hr = cd_slot.load(std::memory_order_relaxed);
    if (last_hr != 0 && static_cast<uint32_t>(cur_hours - last_hr) < 720U) return;

    // Strictly max 1 active supply_node in country's construction queue at a time
    const auto* q_ptr = *reinterpret_cast<const uint8_t* const* const*>(constr + 0x70);
    int32_t q_len = *reinterpret_cast<const int32_t*>(constr + 0x7c);
    uintptr_t prod_line_vt = g_exe_base + RVA_BLDG_PROD_LINE_VT;
    if (q_ptr && q_len > 0) {
        int32_t cap = (q_len < 256) ? q_len : 256;
        for (int32_t i = 0; i < cap; ++i) {
            const uint8_t* item = q_ptr[i];
            if (!item) continue;
            if (*reinterpret_cast<const uintptr_t*>(item) == prod_line_vt) {
                const auto* inner = *reinterpret_cast<const uint8_t* const*>(item + OFF_CONSTR_ITEM_INNER);
                if (inner) {
                    const auto* bldg = *reinterpret_cast<const uint8_t* const*>(inner + OFF_CONSTR_BUILDING_PTR);
                    if (bldg == supply_node) return;
                }
            }
        }
    }

    // Scan country's own land divisions for starving clusters (< 50% supply)
    const auto* units_ptr = *reinterpret_cast<const uint8_t* const* const*>(country + OFF_COUNTRY_UNITS_DATA);
    int32_t units_len = *reinterpret_cast<const int32_t*>(country + OFF_COUNTRY_UNITS_LEN);
    if (!units_ptr || units_len < 3) return;

    auto get_ctrl = reinterpret_cast<BldgContCtrlTagWinFn>(g_exe_base + RVA_BLDG_CONT_CTRL_TAG);
    int32_t starv_ids[64];
    const uint8_t* starv_ptrs[64];
    uint16_t starv_cnts[64];
    size_t starv_len = 0;

    int32_t ucap = (units_len < 2048) ? units_len : 2048;
    for (int32_t i = 0; i < ucap; ++i) {
        const uint8_t* unit = units_ptr[i];
        if (!unit) continue;
        int64_t supply_needed = *reinterpret_cast<const int64_t*>(unit + 0x38);
        int64_t supply_ratio  = *reinterpret_cast<const int64_t*>(unit + 0x40);
        if (supply_needed <= 0 || supply_ratio >= 50000LL) continue;
        const auto* prov = *reinterpret_cast<const uint8_t* const*>(unit + 0x1f0);
        if (!prov) continue;
        int32_t prov_id = *reinterpret_cast<const int32_t*>(prov + OFF_PROV_ID);
        if (prov_id <= 0) continue;
        const auto* sp = *reinterpret_cast<const uint8_t* const*>(prov + OFF_PROV_STATIC_DATA);
        if (!sp || (sp[0xd2] & 1U) == 0) continue;
        const uint32_t* ctrl = get_ctrl(prov + OFF_PROV_BLDG_CONTAINER);
        if (!ctrl || *ctrl != tag_id) continue;

        bool found = false;
        for (size_t k = 0; k < starv_len; ++k) {
            if (starv_ids[k] == prov_id) {
                if (starv_cnts[k] < 65535U) ++starv_cnts[k];
                found = true;
                break;
            }
        }
        if (!found && starv_len < 64) {
            starv_ids[starv_len] = prov_id;
            starv_ptrs[starv_len] = prov;
            starv_cnts[starv_len] = 1;
            ++starv_len;
        }
    }
    if (starv_len == 0) return;

    size_t best_idx = 64;
    uint32_t best_score = 0;
    for (size_t i = 0; i < starv_len; ++i) {
        uint32_t score = starv_cnts[i];
        const auto* sp = *reinterpret_cast<const uint8_t* const*>(starv_ptrs[i] + OFF_PROV_STATIC_DATA);
        if (sp) {
            const auto* adj_ptr = *reinterpret_cast<const uint8_t* const*>(sp + 0x70);
            int32_t adj_len = *reinterpret_cast<const int32_t*>(sp + 0x7c);
            if (adj_ptr && adj_len > 0) {
                int32_t acap = (adj_len < 32) ? adj_len : 32;
                for (int32_t j = 0; j < acap; ++j) {
                    const uint8_t* adj = adj_ptr + static_cast<size_t>(j) * 0x30;
                    if (adj[0x10] == 4) continue;
                    int32_t nb_id = *reinterpret_cast<const int32_t*>(adj + 0x08);
                    for (size_t k = 0; k < starv_len; ++k) {
                        if (starv_ids[k] == nb_id) { score += starv_cnts[k]; break; }
                    }
                }
            }
        }
        if (score > best_score) {
            best_score = score;
            best_idx = i;
        }
    }
    if (best_idx >= starv_len || best_score < 3U) return;

    const uint8_t* worst_prov = starv_ptrs[best_idx];
    auto can_constr = reinterpret_cast<BldgContCanConstrWinFn>(g_exe_base + RVA_BLDG_CONT_CAN_CONSTR);

    // Prefer a safe 1-hop friendly rear province if worst_prov borders enemy/non-own land
    const uint8_t* cands[2] = {nullptr, nullptr};
    size_t n_cands = 0;
    if (prov_borders_non_own_land(gs_sub, get_prov, worst_prov, tag_id)) {
        const auto* sp = *reinterpret_cast<const uint8_t* const*>(worst_prov + OFF_PROV_STATIC_DATA);
        if (sp) {
            const auto* adj_ptr = *reinterpret_cast<const uint8_t* const*>(sp + 0x70);
            int32_t adj_len = *reinterpret_cast<const int32_t*>(sp + 0x7c);
            if (adj_ptr && adj_len > 0) {
                int32_t acap = (adj_len < 32) ? adj_len : 32;
                for (int32_t j = 0; j < acap; ++j) {
                    const uint8_t* adj = adj_ptr + static_cast<size_t>(j) * 0x30;
                    if (adj[0x10] == 4) continue;
                    int32_t nb_id = *reinterpret_cast<const int32_t*>(adj + 0x08);
                    if (nb_id <= 0) continue;
                    const uint8_t* nb_p = get_prov(gs_sub, nb_id);
                    if (!nb_p) continue;
                    const auto* nb_sp = *reinterpret_cast<const uint8_t* const*>(nb_p + OFF_PROV_STATIC_DATA);
                    if (!nb_sp || (nb_sp[0xd2] & 1U) == 0) continue;
                    const uint8_t* nb_cont = nb_p + OFF_PROV_BLDG_CONTAINER;
                    const uint32_t* nb_ctrl = get_ctrl(nb_cont);
                    if (!nb_ctrl || *nb_ctrl != tag_id) continue;
                    if (!prov_borders_non_own_land(gs_sub, get_prov, nb_p, tag_id) &&
                        can_constr(nb_cont, supply_node, 0) != 0) {
                        cands[n_cands++] = nb_p;
                        break;
                    }
                }
            }
        }
    }
    cands[n_cands++] = worst_prov;

    const uint8_t* chosen = nullptr;
    for (size_t ci = 0; ci < n_cands; ++ci) {
        const uint8_t* cand = cands[ci];
        if (can_constr(cand + OFF_PROV_BLDG_CONTAINER, supply_node, 0) == 0) continue;
        if (has_hub_or_port_within_3_hops(gs_sub, get_prov, cand, supply_node, naval_base)) continue;
        chosen = cand;
        break;
    }
    if (!chosen) return;

    uint32_t new_hr = (cur_hours == 0) ? 1U : cur_hours;
    if (!cd_slot.compare_exchange_strong(
            last_hr, new_hr, std::memory_order_acq_rel, std::memory_order_relaxed)) {
        return;
    }

    // Emit native MP-sync-safe CAddConstructionCommand (Windows MSVC ABI: container in %rcx, &loc in %rdx, bldg in %r8!)
    const uint8_t* cont = chosen + OFF_PROV_BLDG_CONTAINER;
    auto loc_ctor = reinterpret_cast<SBldgLocCtorWinFn>(g_exe_base + RVA_SBLDG_LOC_CTOR);
    auto op_new   = reinterpret_cast<OpNewWinFn>(g_exe_base + RVA_OP_NEW);
    auto cmd_ctor = reinterpret_cast<AddConstrCmdCtorWinFn>(g_exe_base + RVA_ADD_CONSTR_CMD_CTOR);
    auto post_cmd = reinterpret_cast<SessionPostWinFn>(g_exe_base + RVA_SESSION_POST);

    uint64_t loc_buf[4] = {0, 0, 0, 0};
    loc_ctor(cont, loc_buf, supply_node);
    uint8_t* cmd = op_new(ADD_CONSTR_CMD_SIZE);
    if (!cmd) return;
    cmd_ctor(cmd, 1, tag_ptr, loc_buf, 1);
    post_cmd(cmd, tag_ptr);

    uint64_t total_queued = g_ai_supply_hubs_queued.fetch_add(1, std::memory_order_relaxed) + 1;
    if (total_queued <= 16) {
        int32_t chosen_id = *reinterpret_cast<const int32_t*>(chosen + OFF_PROV_ID);
        char msg[256];
        std::snprintf(
            msg, sizeof(msg),
            "[hoi4-accel] AI queued wartime supply_node #%llu (tag_id=%u, prov=%d, starving_sector=%u, fac=%lld)\r\n",
            static_cast<unsigned long long>(total_queued),
            tag_id,
            static_cast<int>(chosen_id),
            best_score,
            static_cast<long long>(total_fac)
        );
        append_log_line(msg);
    }
}

extern "C" void hook_fix_supply_bottlenecks(uint8_t* ai_constr) {
    auto orig = reinterpret_cast<FixSupplyBottlenecksFn>(
        g_orig_fix_supply_bottlenecks.load(std::memory_order_relaxed)
    );
    orig(ai_constr);
    if (g_ai_smart_supply_enabled.load(std::memory_order_relaxed) &&
        g_ai_smart_supply_applied.load(std::memory_order_acquire)) {
        try_queue_wartime_supply_hub(ai_constr);
    }
}

static void apply_ai_smart_supply_once() {
    bool expected = false;
    if (!g_ai_smart_supply_applied.compare_exchange_strong(
            expected, true, std::memory_order_acq_rel, std::memory_order_relaxed)) {
        return;
    }
    if (g_exe_base == 0) {
        return;
    }
    resolve_supply_buildings_once();
    auto* freq_ptr      = reinterpret_cast<int32_t*>(g_exe_base + RVA_NDEF_SUPPLY_FREQ_HRS);
    auto* sat_ptr       = reinterpret_cast<int64_t*>(g_exe_base + RVA_NDEF_SUPPLY_SAT_THR);
    auto* mot_freq_ptr  = reinterpret_cast<int32_t*>(g_exe_base + RVA_NDEF_SUPPLY_MOT_FREQ);
    auto* div_mot_ptr   = reinterpret_cast<int64_t*>(g_exe_base + RVA_NDEF_DIV_SUPPLY_MOT);
    auto* prio_hub_ptr  = reinterpret_cast<int64_t*>(g_exe_base + RVA_NDEF_PRIO_SUPPLY_BLDG);
    auto* prio_rail_ptr = reinterpret_cast<int64_t*>(g_exe_base + RVA_NDEF_PRIO_RAILWAY);
    auto* depth_ptr     = reinterpret_cast<int64_t*>(g_exe_base + RVA_NDEF_SUPPLY_MAX_DEPTH);
    auto* crisis_ptr    = reinterpret_cast<int64_t*>(g_exe_base + RVA_NDEF_SUPPLY_CRISIS_LIM);

    int32_t old_freq   = *freq_ptr;
    int64_t old_sat    = *sat_ptr;
    int32_t old_mfreq  = *mot_freq_ptr;
    int64_t old_dmot   = *div_mot_ptr;
    int64_t old_hub    = *prio_hub_ptr;
    int64_t old_rail   = *prio_rail_ptr;
    int64_t old_depth  = *depth_ptr;
    int64_t old_crisis = *crisis_ptr;

    if (old_freq > 0 && old_sat > 0 && old_hub > 0 && old_rail > 0) {
        if (old_freq > 24)        *freq_ptr      = 24;
        if (old_sat > 65000LL)    *sat_ptr       = 65000LL;
        if (old_mfreq > 24)       *mot_freq_ptr  = 24;
        if (old_dmot > 0 && old_dmot < 95000LL) *div_mot_ptr = 95000LL;
        if (old_hub < 450000LL)   *prio_hub_ptr  = 450000LL;
        if (old_rail < 800000LL)  *prio_rail_ptr = 800000LL;
        if (old_depth > 0 && old_depth < 900000LL) *depth_ptr  = 900000LL;
        if (old_crisis > 0 && old_crisis < 25000LL) *crisis_ptr = 25000LL;

        char msg[512];
        std::snprintf(
            msg,
            sizeof(msg),
            "[hoi4-accel] AI Smart Supply v2 active: freq=%dh->%dh, sat=%.2f->%.2f, mot_ratio=%.2f->%.2f, hub_prio=%.2f->%.2f, rail_prio=%.2f->%.2f, bfs_depth=%.0f->%.0f, supply_node_ptr=0x%llx\r\n",
            static_cast<int>(old_freq),
            static_cast<int>(*freq_ptr),
            static_cast<double>(old_sat) / 100000.0,
            static_cast<double>(*sat_ptr) / 100000.0,
            static_cast<double>(old_dmot) / 100000.0,
            static_cast<double>(*div_mot_ptr) / 100000.0,
            static_cast<double>(old_hub) / 100000.0,
            static_cast<double>(*prio_hub_ptr) / 100000.0,
            static_cast<double>(old_rail) / 100000.0,
            static_cast<double>(*prio_rail_ptr) / 100000.0,
            static_cast<double>(old_depth) / 100000.0,
            static_cast<double>(*depth_ptr) / 100000.0,
            static_cast<unsigned long long>(g_supply_node_bldg_ptr.load(std::memory_order_relaxed))
        );
        append_log_line(msg);
    }
}

// --- v1.1 Smart Airbase Expansion (CAIConstruction::GetBuildingQueuePrio + Air Coverage NDefines) ---
// MSVC x64 ABI: CFixedPoint64 return uses hidden out_ret pointer in %rdx (`this`=%rcx, `out_ret`=%rdx, `item`=%r8)
using GetBldgQueuePrioFn = int64_t* (*)(uint8_t* self, int64_t* out_ret, const uint8_t* item);

extern "C" int64_t* hook_get_bldg_queue_prio(
    uint8_t* self,
    int64_t* out_ret,
    const uint8_t* item
) {
    auto orig = reinterpret_cast<GetBldgQueuePrioFn>(
        g_orig_get_bldg_queue_prio.load(std::memory_order_relaxed)
    );
    int64_t* res = orig(self, out_ret, item);
    if (res != nullptr && item != nullptr && *res > 0) {
        const auto* constr = *reinterpret_cast<const uint8_t* const*>(item + OFF_CONSTR_ITEM_INNER);
        if (constr != nullptr) {
            const auto* bldg = *reinterpret_cast<const uint8_t* const*>(constr + OFF_CONSTR_BUILDING_PTR);
            if (bldg != nullptr && bldg[OFF_BLDG_IS_AIR_BASE] != 0) {
                // Boost queued air_base priority by 3.2x (vanilla UNSPECIFIED 0.50 -> 1.60),
                // placing needed frontline airbases cleanly above factories (0.70/0.80)
                // while keeping critical supply hubs (4.50) and railways (8.00) strictly first.
                int64_t cur = *res;
                static constexpr int64_t kMaxSafePrio = 2000000000000000LL;
                int64_t boosted = (cur < (kMaxSafePrio / 16)) ? ((cur * 16) / 5) : kMaxSafePrio;
                *res = boosted;
                g_airbase_prio_boosts.fetch_add(1, std::memory_order_relaxed);
            }
        }
    }
    return res;
}

static void apply_ai_smart_airbase_once() {
    bool expected = false;
    if (!g_ai_smart_airbase_defines_applied.compare_exchange_strong(
            expected, true, std::memory_order_acq_rel, std::memory_order_relaxed)) {
        return;
    }
    if (g_exe_base == 0) {
        return;
    }
    auto raise_fp64 = [&](uintptr_t rva, int64_t target) -> int {
        auto* p = reinterpret_cast<int64_t*>(g_exe_base + rva);
        int64_t cur = *p;
        if (cur > 0 && cur < target) {
            *p = target;
            return 1;
        }
        return 0;
    };

    int updated = 0;
    updated += raise_fp64(RVA_NDEF_WANTED_PLANES_FAC, 75000LL);    // 0.50 -> 0.75
    updated += raise_fp64(RVA_NDEF_AIR_COV_STAY_PUT,  75000LL);    // 0.50 -> 0.75 (Sheep AI: rebase closer when coverage < 75%)
    updated += raise_fp64(RVA_NDEF_OUR_COMBATS_AIR,   45000000LL); // 155 -> 450 (focus air & base capacity on active ground combats)
    updated += raise_fp64(RVA_NDEF_FIGHTERS_PER_PLN,  200000LL);   // 1.0 -> 2.0
    updated += raise_fp64(RVA_NDEF_DEF_SUPPLY_AIR,    900000LL);   // 4.0 -> 9.0 (Sheep AI: protect supply hubs from logistics strikes)

    char msg[256];
    std::snprintf(
        msg,
        sizeof(msg),
        "[hoi4-accel] AI Smart Airbase active: air_base queue prio 3.2x (0.50->1.60) + %d/5 air-coverage defines tuned\r\n",
        updated
    );
    append_log_line(msg);
}

// --- v1.1 -> v1.2 Tactical Prudence, Equipment Conservation & AIFC Spearheads ---
// Strictly adjusts NDefines::NAI decision weights (zero player/combat-rule changes, zero cheats).
// Never raises PLAN_ATTACK_MIN_STRENGTH so empty-tile advances are never blocked!
static void apply_ai_tactical_prudence_once() {
    bool expected = false;
    if (!g_ai_tactical_prudence_applied.compare_exchange_strong(
            expected, true, std::memory_order_acq_rel, std::memory_order_relaxed)) {
        return;
    }
    if (g_exe_base == 0) {
        return;
    }
    // Sanity check that NDefines::NAI globals are loaded
    int64_t sanity_def = *reinterpret_cast<const int64_t*>(g_exe_base + RVA_NDEF_ATK_HEAVY_DEF_LIM);
    int32_t sanity_enc = *reinterpret_cast<const int32_t*>(g_exe_base + RVA_NDEF_ENCIRCLE_DISC_HRS);
    if (sanity_def <= 0 || sanity_enc <= 0) {
        return;
    }

    int changed = 0;
    auto raise_fp64 = [&](uintptr_t rva, int64_t min_target) {
        auto* p = reinterpret_cast<int64_t*>(g_exe_base + rva);
        if (*p < min_target) {
            *p = min_target;
            ++changed;
        }
    };
    auto lower_fp64 = [&](uintptr_t rva, int64_t max_target) {
        auto* p = reinterpret_cast<int64_t*>(g_exe_base + rva);
        if (*p > max_target) {
            *p = max_target;
            ++changed;
        }
    };
    auto raise_i32 = [&](uintptr_t rva, int32_t min_target) {
        auto* p = reinterpret_cast<int32_t*>(g_exe_base + rva);
        if (*p < min_target) {
            *p = min_target;
            ++changed;
        }
    };
    auto lower_i32 = [&](uintptr_t rva, int32_t max_target) {
        auto* p = reinterpret_cast<int32_t*>(g_exe_base + rva);
        if (*p > max_target) {
            *p = max_target;
            ++changed;
        }
    };

    // 1. Stop suicidal attacks into defended stacks & abort losing meatgrinders in 12h (never blocks empty tiles!)
    raise_fp64(RVA_NDEF_ATK_HEAVY_DEF_LIM,  95000LL);    // 0.50 -> 0.95 (refuse bad odds vs fortified/entrenched stacks; empty tile def=0!)
    raise_fp64(RVA_NDEF_PLAN_VAL_EXECUTE,   -15000LL);   // -0.50 -> -0.15 (stop launching hopeless front-wide suicide plans)
    lower_fp64(RVA_NDEF_LOC_BAL_ADVANCE,    0LL);        // >0   -> 0.00 (Sheep AI: step into empty/weak tiles without hesitation)
    raise_fp64(RVA_NDEF_STR_UNIT_WEAK,      50000LL);    // 0.30 -> 0.50
    raise_fp64(RVA_NDEF_ORG_UNIT_WEAK,      35000LL);    // 0.25 -> 0.35
    lower_fp64(RVA_NDEF_PLAN_WEAK_ABORT,    45000LL);    // 0.65 -> 0.45 (abort offensive plans once 45% of army drops below 50% str)
    lower_fp64(RVA_NDEF_CANCEL_DISADV_RAT,  115000LL);   // 1.50 -> 1.15 (abort losing combats fast)
    lower_i32(RVA_NDEF_CANCEL_MIN_DUR_HR,   12);         // 48h  -> 12h  (save guns instead of grinding 2 days into forts)
    raise_fp64(RVA_NDEF_DESP_WEAK_STR_LIM,  65000LL);    // 0.35 -> 0.65

    // 2. Equipment Stockpile Discipline (prevent rifle death spiral without starving tank factories!)
    raise_fp64(RVA_NDEF_MIN_FIELD_STR_BLD,  78000LL);    // 0.70 -> 0.78 (v1.3: softened from 0.88 so mid-war tank training continues)
    raise_fp64(RVA_NDEF_START_TRAIN_EQ,     85000LL);    // 0.40 -> 0.85
    raise_fp64(RVA_NDEF_STOP_TRAIN_EQ,      75000LL);    // 0.25 -> 0.75 (stop exercising & burning equipment when stockpile < 75%)
    raise_fp64(RVA_NDEF_DEPLOY_MIN_EQ_WAR,  90000LL);    // 0.85 -> 0.90
    lower_fp64(RVA_NDEF_PROD_EQ_SURPLUS,    55000LL);    // 0.80 -> 0.55 (v1.3: stop hoarding 100% surplus rifles, freeing mils for armor!)

    // 3. Smarter front assessment & terrain discipline (gentle entrenchment weight so divisions stay fluid)
    raise_fp64(RVA_NDEF_FRONT_EVAL_ACC,     150000LL);   // 1.00 -> 1.50 (Sheep AI)
    raise_fp64(RVA_NDEF_FRONT_EVAL_AIR,     200000LL);   // 1.00 -> 2.00 (Sheep AI: respect air superiority)
    raise_fp64(RVA_NDEF_FRONT_EVAL_SUPLCK,  150000LL);   // 1.00 -> 1.50 (Sheep AI: avoid overstacking low-supply pockets)
    raise_fp64(RVA_NDEF_TERRAIN_IMPORT,     1000000LL);  // >= 10.0
    lower_fp64(RVA_NDEF_TANKS_MOUNTAINS,    -1500000LL); // <= -15.0 (Sheep AI: keep tanks out of mountains)
    lower_fp64(RVA_NDEF_TANKS_JUNGLE,       -2000000LL); // <= -20.0 (Sheep AI: keep tanks out of jungles)
    raise_fp64(RVA_NDEF_ENTRENCH_WEIGHT,    220000LL);   // 2.0  -> 2.2  (v1.2+: lowered from 4.0 to stop gluing divisions in place)

    // 4. Daily pocket/bulge detection & UNBLOCKED empty-tile micro capture
    lower_i32(RVA_NDEF_ENCIRCLE_DISC_HRS,   24);         // 72h  -> 24h (daily encirclement discovery)
    raise_fp64(RVA_NDEF_BULGE_LOW_CUTOFF,   120000LL);   // 0.95 -> 1.20 (Sheep AI)
    raise_fp64(RVA_NDEF_BULGE_HIGH_CUTOFF,  180000LL);   // 1.50 -> 1.80 (Sheep AI)
    lower_i32(RVA_NDEF_CUTOFF_EDGE_PROX,    1);          // 2    -> 1    (Sheep AI)
    raise_i32(RVA_NDEF_MAX_MICRO_ATKS,      10);         // 3    -> 10   (v1.3: grab open gaps cleanly)
    lower_fp64(RVA_NDEF_MICRO_INACT_VAL,    -100000LL);  // 0.25 -> -1.00 (v1.3: kills dead-zone against PLAN_VALUE_TO_EXECUTE; occupied tiles still require superiority!)

    // 5. Armored/Shock Force Concentration (AIFC — Sheep AI / Discord AI spearheads)
    lower_i32(RVA_NDEF_AIFC_UPD_DAYS,       4);          // 5d   -> 4d
    raise_fp64(RVA_NDEF_AIFC_UNIT_RATIO,    22000LL);    // 0.15 -> 0.22 (22% shock spearhead allocation)
    raise_fp64(RVA_NDEF_AIFC_ACTIVATE_ORG,  30000LL);    // 0.20 -> 0.30 (v1.2+: balanced from 0.42 so spearheads actually roll)
    lower_i32(RVA_NDEF_AIFC_NUDGE_DAYS,     10);         // 15d  -> 10d
    raise_fp64(RVA_NDEF_AIFC_HARDNESS,      80000000LL); // 300  -> 800  (Sheep AI: pack armor/mech into spearheads)
    raise_fp64(RVA_NDEF_AIFC_ARMOR,         8000000LL);  // 30   -> 80   (Sheep AI)
    raise_fp64(RVA_NDEF_AIFC_SOFT_ATK,      1200000LL);  // 6    -> 12   (Sheep AI)
    raise_fp64(RVA_NDEF_AIFC_HARD_ATK,      1600000LL);  // 8    -> 16   (Sheep AI)
    raise_fp64(RVA_NDEF_AIFC_EXPERIENCE,    50000000LL); // 300  -> 500  (Sheep AI)
    raise_fp64(RVA_NDEF_AIFC_TGT_SUPPLY,    3000000LL);  // 20   -> 30   (Sheep AI: aim spearheads at enemy supply hubs)
    raise_fp64(RVA_NDEF_AIFC_TGT_PORT,      1500000LL);  // 10   -> 15   (Sheep AI)
    raise_fp64(RVA_NDEF_AIFC_TGT_PORT_LVL,  200000LL);   // 1    -> 2    (Sheep AI)
    raise_fp64(RVA_NDEF_AIFC_COST_MTN,      400000LL);   // 3.0  -> 4.0  (Sheep AI: route spearheads around mountains)

    char msg[256];
    std::snprintf(
        msg,
        sizeof(msg),
        "[hoi4-accel] AI Tactical Prudence v3 active (%d/40 NAI defines tuned; empty_tile_patch=%d, inactive_micro=-1.00)\r\n",
        changed,
        g_empty_tile_fix_applied.load(std::memory_order_relaxed) ? 1 : 0
    );
    append_log_line(msg);
}

// --- v1.2 Smart Peace Conference Anti-Bordergore (CPeaceWinnerAi::CalcStateActionScore @ 0x1a8ca20) ---
// MSVC x64 ABI: CFixedPoint64 return uses hidden out_ret pointer in %rcx!
using PeaceCalcStateScoreFn = int64_t* (*)(
    int64_t* out_ret,
    const uint8_t* conf,
    const int32_t* taker,
    const int32_t* giver,
    const int32_t* negotiator,
    int32_t state_id,
    int32_t action_type,
    uintptr_t extra1,
    uintptr_t extra2,
    uintptr_t debug_str
);
using PeaceIsStateClaimedFn = uint8_t (*)(const uint8_t* conf, int32_t state_id, const int32_t* tag);

static int64_t adjust_peace_bordergore_score(
    const uint8_t* conf,
    const int32_t* taker,
    const int32_t* negotiator,
    int32_t state_id,
    int32_t action_type,
    int64_t score
) {
    if (score <= 0 || g_exe_base == 0 || !conf || !taker || state_id < 1) {
        return score;
    }
    bool is_take = (action_type == PEACE_ACT_TAKE_STATES || action_type == PEACE_ACT_GIVE_STATES);
    bool is_puppet = (action_type == PEACE_ACT_PUPPET);
    if (!is_take && !is_puppet) {
        return score;
    }
    int32_t taker_tag = *taker;
    if (taker_tag <= 0) {
        return score;
    }
    const auto* gs = *reinterpret_cast<const uint8_t* const*>(g_exe_base + RVA_GAMESTATE_PTR);
    if (!gs) {
        return score;
    }
    int32_t n_states = *reinterpret_cast<const int32_t*>(gs + OFF_GS_STATES_LEN);
    if (state_id >= n_states) {
        return score;
    }
    const auto* states_arr = *reinterpret_cast<const uint8_t* const* const*>(gs + OFF_GS_STATES_DATA);
    if (!states_arr) {
        return score;
    }
    const uint8_t* state = states_arr[state_id];
    if (!state) {
        return score;
    }

    // Never penalize a nation reclaiming its own core state
    int32_t core_len = *reinterpret_cast<const int32_t*>(state + OFF_STATE_CORES_LEN);
    if (core_len > 0) {
        const auto* cores = *reinterpret_cast<const int32_t* const*>(state + OFF_STATE_CORES_DATA);
        if (cores) {
            int32_t cap = (core_len < 64) ? core_len : 64;
            for (int32_t i = 0; i < cap; ++i) {
                if (cores[i] == taker_tag) {
                    return score;
                }
            }
        }
    }

    const auto* state_data = *reinterpret_cast<const uint8_t* const*>(state + OFF_STATE_DATA_PTR);
    if (!state_data) {
        return score;
    }
    bool is_coastal = (state_data[OFF_STATEDATA_IS_COASTAL] != 0);
    int32_t nb_len = *reinterpret_cast<const int32_t*>(state_data + OFF_STATEDATA_NB_LEN);
    const auto* nb_ptr = *reinterpret_cast<const int32_t* const*>(state_data + OFF_STATEDATA_NB_DATA);
    auto is_claimed = reinterpret_cast<PeaceIsStateClaimedFn>(g_exe_base + RVA_PEACE_IS_STATE_CLAIMED);

    int32_t neg_tag = 0;
    if (is_puppet && negotiator != nullptr) {
        int32_t t = *negotiator;
        if (t > 0 && t != taker_tag) {
            neg_tag = t;
        }
    }

    int32_t adj_taker = 0;
    int32_t adj_overlord = 0;
    if (nb_len > 0 && nb_ptr != nullptr) {
        int32_t cap = (nb_len < 64) ? nb_len : 64;
        for (int32_t i = 0; i < cap; ++i) {
            int32_t nb_id = nb_ptr[i];
            if (nb_id < 1 || nb_id >= n_states) {
                continue;
            }
            const uint8_t* nb_state = states_arr[nb_id];
            if (!nb_state) {
                continue;
            }
            int32_t nb_owner = *reinterpret_cast<const int32_t*>(nb_state + OFF_STATE_OWNER_TAG);
            if (nb_owner == taker_tag || is_claimed(conf, nb_id, taker) != 0) {
                ++adj_taker;
            } else if (neg_tag > 0 &&
                       (nb_owner == neg_tag || is_claimed(conf, nb_id, negotiator) != 0)) {
                ++adj_overlord;
            }
        }
    }

    static constexpr int64_t kMaxSafeScore = 1000000000000000LL;
    int64_t adjusted = score;
    if (adj_taker >= 1) {
        int64_t mult = 2LL + static_cast<int64_t>((adj_taker < 3) ? adj_taker : 3);
        adjusted = (score < (kMaxSafeScore / mult)) ? (score * mult) : kMaxSafeScore;
    } else if (is_puppet && adj_overlord >= 1) {
        adjusted = (score < (kMaxSafeScore / 5LL)) ? ((score * 5LL) / 2LL) : kMaxSafeScore;
    } else if (is_coastal) {
        int64_t s = score / 4LL;
        adjusted = (s > 1LL) ? s : 1LL;
    } else {
        int64_t s = score / 20LL;
        adjusted = (s > 1LL) ? s : 1LL;
    }

    g_peace_bordergore_adjusts.fetch_add(1, std::memory_order_relaxed);
    return adjusted;
}

extern "C" int64_t* hook_peace_calc_state_score(
    int64_t* out_ret,
    const uint8_t* conf,
    const int32_t* taker,
    const int32_t* giver,
    const int32_t* negotiator,
    int32_t state_id,
    int32_t action_type,
    uintptr_t extra1,
    uintptr_t extra2,
    uintptr_t debug_str
) {
    auto orig = reinterpret_cast<PeaceCalcStateScoreFn>(
        g_orig_peace_calc_state_score.load(std::memory_order_relaxed)
    );
    int64_t* res = orig(
        out_ret, conf, taker, giver, negotiator, state_id, action_type, extra1, extra2, debug_str
    );
    if (res != nullptr && *res > 0) {
        *res = adjust_peace_bordergore_score(conf, taker, negotiator, state_id, action_type, *res);
    }
    return res;
}

static void apply_ai_smart_peace_once() {
    bool expected = false;
    if (!g_ai_smart_peace_defines_applied.compare_exchange_strong(
            expected, true, std::memory_order_acq_rel, std::memory_order_relaxed)) {
        return;
    }
    if (g_exe_base == 0) {
        return;
    }
    int changed = 0;
    auto* fold_ptr = reinterpret_cast<int32_t*>(g_exe_base + RVA_NDEF_PEACE_FOLD_TURNS);
    if (*fold_ptr > 1) {
        *fold_ptr = 1;
        ++changed;
    }
    auto* dist_ptr = reinterpret_cast<int64_t*>(g_exe_base + RVA_NDEF_PEACE_TIE_DIST);
    if (*dist_ptr > 0 && *dist_ptr < 400000LL) {
        *dist_ptr = 400000LL; // 1.0 -> 4.0 distance tie-breaker
        ++changed;
    }
    char msg[256];
    std::snprintf(
        msg,
        sizeof(msg),
        "[hoi4-accel] AI Smart Peace active: contiguous border scoring (3x..5x / -95%% inland enclaves) + %d/2 peace defines tuned\r\n",
        changed
    );
    append_log_line(msg);
}

// --- Dormant Peaceful Minor AI Hourly Throttling ---
using OwnerAccessCountryFn = uint8_t* (*)(uint8_t* owner);
using CountryHasHumanFn    = uint8_t (*)(uint8_t* country);
using DiploIsAtWarFn       = uint8_t (*)(const uint8_t* diplo, uint8_t arg);
using CountryAiPassFn      = void (*)(uint8_t* self, uintptr_t a1, uintptr_t a2);

__forceinline uint8_t* get_country_from_ai(uint8_t* self) {
    if (!self) return nullptr;
    auto* owner = *reinterpret_cast<uint8_t* const*>(self + 0x08);
    if (!owner) return nullptr;
    const auto* owner_vt = *reinterpret_cast<const uintptr_t* const*>(owner);
    if (!owner_vt || owner_vt[0x50 / 8] == 0) return nullptr;
    return reinterpret_cast<OwnerAccessCountryFn>(owner_vt[0x50 / 8])(owner);
}

// --- v1.3 Smart Armored & Mechanized AI (CAIStrategyStore::GetStrategyValue + Tank NDefines) ---
struct WinStrView {
    const char* ptr;
    uint64_t len;
};
using CTokenFromStringWinFn = int32_t* (*)(int32_t* out_tok, const WinStrView* sv);
using GetStrategyValueWinFn = int32_t (*)(const uint8_t* store, int32_t stype, int32_t id, int32_t target);

static int32_t resolve_token_win(const char* name) {
    if (g_exe_base == 0 || !name) return -1;
    auto from_str = reinterpret_cast<CTokenFromStringWinFn>(g_exe_base + RVA_CTOKEN_FROM_STRING);
    int32_t tok = -1;
    WinStrView sv{name, static_cast<uint64_t>(std::strlen(name))};
    from_str(&tok, &sv);
    return tok;
}

static void apply_ai_smart_armor_once() {
    bool expected = false;
    if (!g_ai_smart_armor_defines_applied.compare_exchange_strong(
            expected, true, std::memory_order_acq_rel, std::memory_order_relaxed)) {
        return;
    }
    if (g_exe_base == 0) return;

    int32_t t_arm  = resolve_token_win("armor");
    int32_t t_mech = resolve_token_win("mechanized");
    int32_t t_inf  = resolve_token_win("infantry");
    if (t_arm > 0)  g_tok_armor.store(t_arm, std::memory_order_release);
    if (t_mech > 0) g_tok_mechanized.store(t_mech, std::memory_order_release);
    if (t_inf > 0)  g_tok_infantry.store(t_inf, std::memory_order_release);

    int changed = 0;
    auto raise_fp64 = [&](uintptr_t rva, int64_t target) {
        auto* p = reinterpret_cast<int64_t*>(g_exe_base + rva);
        if (*p > 0 && *p < target) { *p = target; ++changed; }
    };
    auto raise_i32 = [&](uintptr_t rva, int32_t target) {
        auto* p = reinterpret_cast<int32_t*>(g_exe_base + rva);
        if (*p > 0 && *p < target) { *p = target; ++changed; }
    };
    auto lower_i32 = [&](uintptr_t rva, int32_t target) {
        auto* p = reinterpret_cast<int32_t*>(g_exe_base + rva);
        if (*p > target) { *p = target; ++changed; }
    };

    raise_i32(RVA_NDEF_UPG_DEFICIT_DAYS,  210);
    raise_fp64(RVA_NDEF_UPG_PCT_FORCES,   35000LL);
    lower_i32(RVA_NDEF_UPG_DIV_RELUCT,    4);
    lower_i32(RVA_NDEF_VAR_XP_MOD_LAND,   20);
    lower_i32(RVA_NDEF_VAR_XP_LEG_LAND,   20);
    lower_i32(RVA_NDEF_VAR_XP_RES_LAND,   30);
    raise_fp64(RVA_NDEF_XP_UPG_LAND_EQ,   200000LL);
    raise_fp64(RVA_NDEF_XP_UPD_LAND_TMPL, 250000LL);
    raise_fp64(RVA_NDEF_ASSIGN_TANKS_WAR, 1000000LL);

    char msg[256];
    std::snprintf(
        msg, sizeof(msg),
        "[hoi4-accel] AI Smart Armor active: %d/9 tank/upgrade defines tuned | tokens(armor=%d, mech=%d, inf=%d)\r\n",
        changed, static_cast<int>(t_arm), static_cast<int>(t_mech), static_cast<int>(t_inf)
    );
    append_log_line(msg);
}

extern "C" int32_t hook_get_strategy_value(
    const uint8_t* store,
    int32_t stype,
    int32_t id,
    int32_t target
) {
    auto orig = reinterpret_cast<GetStrategyValueWinFn>(
        g_orig_get_strategy_value.load(std::memory_order_relaxed)
    );
    int32_t val = orig(store, stype, id, target);

    if ((stype != 0x10 && stype != 0x15) || target != 0 || !store) return val;
    int32_t t_arm  = g_tok_armor.load(std::memory_order_acquire);
    int32_t t_mech = g_tok_mechanized.load(std::memory_order_acquire);
    int32_t t_inf  = g_tok_infantry.load(std::memory_order_acquire);
    if (id != t_arm && id != t_mech && id != t_inf) return val;
    if (g_exe_base == 0) return val;

    // Verify store is embedded inside CCountryAI (+0x68) before casting
    auto* ai_ptr = const_cast<uint8_t*>(store - 0x68);
    if (*reinterpret_cast<const uintptr_t*>(ai_ptr) != g_exe_base + RVA_COUNTRY_AI_VT) {
        return val;
    }

    const auto* gs = *reinterpret_cast<const uint8_t* const*>(g_exe_base + RVA_GAMESTATE_PTR);
    if (!gs) return val;
    int32_t raw_date = *reinterpret_cast<const int32_t*>(gs + OFF_GS_CDATE_HOURS);
    int32_t year = (raw_date - 0x029c55c0) / 8760;
    if (year < 1938) return val;

    uint8_t* country = get_country_from_ai(ai_ptr);
    if (!country) return val;
    if (reinterpret_cast<CountryHasHumanFn>(g_exe_base + RVA_COUNTRY_HAS_HUMAN)(country) != 0) {
        return val;
    }
    const auto* constr = *reinterpret_cast<const uint8_t* const*>(country + OFF_COUNTRY_CONSTR_PTR);
    if (!constr) return val;

    int64_t total_fac = *reinterpret_cast<const int64_t*>(constr + 0x2d0) / 100000LL;
    int64_t civ_fac   = static_cast<int64_t>(*reinterpret_cast<const int32_t*>(constr + 0x310));
    int64_t dock_fac  = static_cast<int64_t>(*reinterpret_cast<const int32_t*>(constr + 0x318));
    int64_t mil = total_fac - civ_fac - dock_fac;
    if (mil < 22) return val;

    int32_t delta = 0;
    if (stype == 0x10) {
        if (id == t_arm && val > -40) {
            delta = (mil >= 55 && year >= 1940) ? 30
                    : (mil >= 35 && year >= 1939) ? 22
                                                  : 14;
        } else if (id == t_mech && mil >= 50 && year >= 1941 && val > -40) {
            delta = 12;
        } else if (id == t_inf && mil >= 40 && year >= 1940 && val > 55) {
            delta = -15;
        }
    } else if (stype == 0x15) {
        if (id == t_arm && val > -40) {
            delta = 40;
        } else if (id == t_mech && mil >= 50 && year >= 1941 && val > -40) {
            delta = 25;
        }
    }

    if (delta != 0) {
        g_ai_armor_strategy_boosts.fetch_add(1, std::memory_order_relaxed);
        int64_t sum = static_cast<int64_t>(val) + static_cast<int64_t>(delta);
        if (sum > INT32_MAX) return INT32_MAX;
        if (sum < INT32_MIN) return INT32_MIN;
        return static_cast<int32_t>(sum);
    }
    return val;
}

extern "C" void hook_country_ai_pass1(uint8_t* self, uintptr_t a1, uintptr_t a2) {
    auto orig = reinterpret_cast<CountryAiPassFn>(
        g_orig_country_ai_pass1.load(std::memory_order_relaxed)
    );
    if (g_exe_base != 0 && self != nullptr && self[0x60] != 0) {
        uint8_t* country = get_country_from_ai(self);
        if (country != nullptr) {
            uint32_t tag_id = *reinterpret_cast<const uint32_t*>(country + OFF_COUNTRY_TAG_ID);
            size_t bit_idx = static_cast<size_t>(tag_id) & 511U;
            size_t word_idx = bit_idx >> 6;
            uint64_t bit_mask = 1ULL << (bit_idx & 63U);

            int32_t num_states = *reinterpret_cast<const int32_t*>(country + OFF_COUNTRY_NUM_STATES);
            uint8_t is_major = country[OFF_COUNTRY_IS_MAJOR];
            const auto* gs = *reinterpret_cast<const uint8_t* const*>(g_exe_base + RVA_GAMESTATE_PTR);

            bool throttle = false;
            if (num_states >= 1 && num_states < 12 && is_major == 0 && gs != nullptr) {
                int32_t raw_date = *reinterpret_cast<const int32_t*>(gs + OFF_GS_CDATE_HOURS);
                auto total_hours = static_cast<uint32_t>(raw_date - 0x029c55c0);
                if ((total_hours % 24U) != 0U && ((total_hours + tag_id) & 3U) != 0U) {
                    auto* c10_ptr = reinterpret_cast<int32_t*>(self + 0x10);
                    auto* c14_ptr = reinterpret_cast<int32_t*>(self + 0x14);
                    int32_t c10 = *c10_ptr;
                    int32_t c14 = *c14_ptr;
                    if (c10 > 1 && c14 > 1) {
                        auto has_human = reinterpret_cast<CountryHasHumanFn>(
                            g_exe_base + RVA_COUNTRY_HAS_HUMAN
                        );
                        if (has_human(country) == 0) {
                            const auto* diplo = *reinterpret_cast<const uint8_t* const*>(
                                country + OFF_COUNTRY_DIPLO_PTR
                            );
                            if (diplo != nullptr) {
                                auto is_at_war = reinterpret_cast<DiploIsAtWarFn>(
                                    g_exe_base + RVA_DIPLO_IS_AT_WAR
                                );
                                if (is_at_war(diplo, 0) == 0) {
                                    *c10_ptr = c10 - 1;
                                    *c14_ptr = c14 - 1;
                                    throttle = true;
                                }
                            }
                        }
                    }
                }
            }

            if (throttle) {
                g_minor_skip_bitmap[word_idx].fetch_or(bit_mask, std::memory_order_relaxed);
                g_minor_ai_skipped.fetch_add(1, std::memory_order_relaxed);
                return;
            } else {
                g_minor_skip_bitmap[word_idx].fetch_and(~bit_mask, std::memory_order_relaxed);
            }
        }
    }
    g_minor_ai_executed.fetch_add(1, std::memory_order_relaxed);
    orig(self, a1, a2);
}

extern "C" void hook_country_ai_pass2(uint8_t* self, uintptr_t a1, uintptr_t a2) {
    if (self != nullptr && self[0x60] != 0) {
        uint8_t* country = get_country_from_ai(self);
        if (country != nullptr) {
            uint32_t tag_id = *reinterpret_cast<const uint32_t*>(country + OFF_COUNTRY_TAG_ID);
            size_t bit_idx = static_cast<size_t>(tag_id) & 511U;
            uint64_t word = g_minor_skip_bitmap[bit_idx >> 6].load(std::memory_order_relaxed);
            if ((word & (1ULL << (bit_idx & 63U))) != 0ULL) {
                g_minor_ai_skipped.fetch_add(1, std::memory_order_relaxed);
                return;
            }
        }
    }
    reinterpret_cast<CountryAiPassFn>(
        g_orig_country_ai_pass2.load(std::memory_order_relaxed)
    )(self, a1, a2);
}

// ============================================================================
// 10. CScopedTimer Hooks (0x25554d0 / 0x2555c20) — Zone Profiler & Hour Counter
//     CRITICAL MSVC x64 ABI: Constructor MUST return `this` in %rax!
// ============================================================================
static void record_zone(uintptr_t name_ptr, uint64_t elapsed_ns) {
    if (name_ptr == 0) {
        return;
    }
    uintptr_t hourly_ptr = g_hourly_ptr.load(std::memory_order_relaxed);
    if (hourly_ptr == 0) {
        if (std::strcmp(reinterpret_cast<const char*>(name_ptr), "gamestate.hourly") == 0) {
            g_hourly_ptr.store(name_ptr, std::memory_order_relaxed);
            hourly_ptr = name_ptr;
        }
    }

    if (name_ptr == hourly_ptr) {
        uint64_t h = g_total_hours.fetch_add(1, std::memory_order_relaxed) + 1;
        uint64_t max_hours = g_bench_max_hours.load(std::memory_order_relaxed);
        if (max_hours > 0) {
            uint64_t warmup = g_bench_warmup_hours.load(std::memory_order_relaxed);
            if (h == warmup + 1) {
                reset_stats();
                g_bench_start_cpu_ns.store(now_cpu_ns(), std::memory_order_relaxed);
                g_bench_start_ns.store(now_ns(), std::memory_order_relaxed);
                g_bench_recording.store(true, std::memory_order_release);
            } else if (h >= warmup + max_hours) {
                if (!g_report_emitted.exchange(true, std::memory_order_acq_rel)) {
                    g_bench_recording.store(false, std::memory_order_release);
                    emit_report(max_hours);
                    TerminateProcess(GetCurrentProcess(), 0);
                }
            }
        } else if (g_collect_zones.load(std::memory_order_relaxed) &&
                   !g_profile_started_sim.exchange(true, std::memory_order_acq_rel)) {
            reset_stats();
            g_bench_start_cpu_ns.store(now_cpu_ns(), std::memory_order_relaxed);
            g_bench_start_ns.store(now_ns(), std::memory_order_relaxed);
            g_bench_recording.store(true, std::memory_order_release);
        }
    }

    if (!g_collect_zones.load(std::memory_order_relaxed)) {
        return;
    }
    if (g_bench_max_hours.load(std::memory_order_relaxed) > 0 &&
        !g_bench_recording.load(std::memory_order_relaxed)) {
        return;
    }

    bool is_main = (GetCurrentThreadId() == g_main_tid);
    size_t idx = hash_uintptr(name_ptr) & (ZONE_CAP - 1);
    for (size_t probe = 0; probe < 64; ++probe) {
        ZoneSlot& slot = g_zones[idx];
        uintptr_t existing = slot.name_ptr.load(std::memory_order_acquire);
        uintptr_t zero = 0;
        if (existing == name_ptr ||
            (existing == 0 &&
             slot.name_ptr.compare_exchange_strong(
                 zero, name_ptr, std::memory_order_acq_rel, std::memory_order_acquire))) {
            slot.calls.fetch_add(1, std::memory_order_relaxed);
            slot.total_ns.fetch_add(elapsed_ns, std::memory_order_relaxed);
            if (is_main) {
                slot.main_ns.fetch_add(elapsed_ns, std::memory_order_relaxed);
            } else {
                slot.worker_ns.fetch_add(elapsed_ns, std::memory_order_relaxed);
            }
            uint64_t cur_max = slot.max_ns.load(std::memory_order_relaxed);
            while (elapsed_ns > cur_max &&
                   !slot.max_ns.compare_exchange_weak(
                       cur_max, elapsed_ns, std::memory_order_relaxed, std::memory_order_relaxed)) {
            }
            return;
        }
        idx = (idx + 1) & (ZONE_CAP - 1);
    }
}

extern "C" uint64_t* hook_scoped_timer_ctor(uint64_t* self, const char* name) {
    self[0] = reinterpret_cast<uintptr_t>(name);
    self[1] = 0;
    self[2] = g_collect_zones.load(std::memory_order_relaxed) ? now_ns() : 0;
    self[3] = 0;
    return self;
}

extern "C" void hook_scoped_timer_dtor(uint64_t* self) {
    auto name_ptr = static_cast<uintptr_t>(self[0]);
    uint64_t start_ns = self[2];
    if (start_ns != UINT64_MAX && name_ptr != 0) {
        uint64_t elapsed = 0;
        if (g_collect_zones.load(std::memory_order_relaxed)) {
            uint64_t now = now_ns();
            elapsed = (now > start_ns) ? (now - start_ns) : 0;
        }
        record_zone(name_ptr, elapsed);
    }
}

// ============================================================================
// 11. Profiler Report Formatter & Configuration Reader (.ini + Env Vars)
// ============================================================================
struct ZoneRow {
    const char* name;
    uint64_t calls;
    uint64_t total;
    uint64_t main_ns;
    uint64_t worker_ns;
};

static void emit_report(uint64_t measured_hours) {
    uint64_t end_ns = now_ns();
    uint64_t start_ns = g_bench_start_ns.load(std::memory_order_relaxed);
    uint64_t wall_ns = (end_ns > start_ns) ? (end_ns - start_ns) : 1;
    double wall_s = static_cast<double>(wall_ns) / 1e9;
    double days = static_cast<double>(measured_hours) / 24.0;
    double hours_per_sec = static_cast<double>(measured_hours) / wall_s;
    double days_per_sec = days / wall_s;
    double ms_per_day = (days > 0.0) ? ((wall_s * 1000.0) / days) : 0.0;

    uint64_t cpu_end = now_cpu_ns();
    uint64_t cpu_start = g_bench_start_cpu_ns.load(std::memory_order_relaxed);
    double cpu_s = static_cast<double>((cpu_end > cpu_start) ? (cpu_end - cpu_start) : 0) / 1e9;
    double active_cores = cpu_s / wall_s;

    SYSTEM_INFO si{};
    GetNativeSystemInfo(&si);

    ZoneRow rows[ZONE_CAP];
    size_t n_rows = 0;
    for (size_t i = 0; i < ZONE_CAP; ++i) {
        uintptr_t p = g_zones[i].name_ptr.load(std::memory_order_acquire);
        uint64_t calls = g_zones[i].calls.load(std::memory_order_relaxed);
        if (p == 0 || calls == 0) {
            continue;
        }
        rows[n_rows++] = ZoneRow{
            reinterpret_cast<const char*>(p),
            calls,
            g_zones[i].total_ns.load(std::memory_order_relaxed),
            g_zones[i].main_ns.load(std::memory_order_relaxed),
            g_zones[i].worker_ns.load(std::memory_order_relaxed),
        };
    }
    // Simple selection sort for top 25 zones (zero STL heap/exceptions)
    size_t top_n = (n_rows < 25) ? n_rows : 25;
    for (size_t i = 0; i < top_n; ++i) {
        size_t best = i;
        for (size_t j = i + 1; j < n_rows; ++j) {
            if (rows[j].total > rows[best].total) {
                best = j;
            }
        }
        if (best != i) {
            ZoneRow tmp = rows[i];
            rows[i] = rows[best];
            rows[best] = tmp;
        }
    }

    char buf[16384];
    int pos = 0;
    auto put = [&](const char* fmt, ...) {
        if (pos >= static_cast<int>(sizeof(buf)) - 256) {
            return;
        }
        va_list ap;
        va_start(ap, fmt);
        int n = std::vsnprintf(buf + pos, sizeof(buf) - static_cast<size_t>(pos), fmt, ap);
        va_end(ap);
        if (n > 0) {
            pos += (pos + n < static_cast<int>(sizeof(buf)))
                       ? n
                       : (static_cast<int>(sizeof(buf)) - 1 - pos);
        }
    };

    uint64_t tf_ns = g_target_frame_ns.load(std::memory_order_relaxed);
    uint64_t tf_fps = (tf_ns > 0) ? (1000000000ULL / tf_ns) : 0;
    uint64_t sf_fps = g_smooth_fps.load(std::memory_order_relaxed);

    put("\r\n================ HOI4 ACCEL v1.3-win64 REPORT ================\r\n");
    put("Measured Hours : %llu (%.2f game days)\r\n",
        static_cast<unsigned long long>(measured_hours), days);
    put("Wall Time      : %.3f s\r\n", wall_s);
    put("Throughput     : %.2f hours/s  |  %.2f days/s  |  %.2f ms/day\r\n",
        hours_per_sec, days_per_sec, ms_per_day);
    put("CPU Utilization: %.2fs CPU in %.2fs wall  =>  %.2f active cores (of %lu logical)\r\n",
        cpu_s, wall_s, active_cores, static_cast<unsigned long>(si.dwNumberOfProcessors));
    put("Config         : target_fps=%llu smooth_fps=%llu fast_alloc=%d fast_mod=%d country_grain=%d civil_war_fix=%d ai_smart_supply=%d ai_smart_airbase=%d ai_tactical_prudence=%d ai_smart_armor=%d ai_smart_peace=%d ai_throttle_minors=%d fast_sched=%d fast_wait=%d fast_yield=%d no_job_clock=%d affinity=%d quiet_log=%d fast_save=%d no_telemetry=%d\r\n",
        static_cast<unsigned long long>(tf_fps),
        static_cast<unsigned long long>(sf_fps),
        g_fast_alloc_enabled.load() ? 1 : 0,
        g_fast_mod_enabled.load() ? 1 : 0,
        g_country_grain_enabled.load() ? 1 : 0,
        g_civil_war_fix_enabled.load() ? 1 : 0,
        g_ai_smart_supply_enabled.load() ? 1 : 0,
        g_ai_smart_airbase_enabled.load() ? 1 : 0,
        g_ai_tactical_prudence_enabled.load() ? 1 : 0,
        g_ai_smart_armor_enabled.load() ? 1 : 0,
        g_ai_smart_peace_enabled.load() ? 1 : 0,
        g_ai_throttle_minors_enabled.load() ? 1 : 0,
        g_fast_sched_enabled.load() ? 1 : 0,
        g_fast_wait_enabled.load() ? 1 : 0,
        g_fast_yield_enabled.load() ? 1 : 0,
        g_no_job_clock_enabled.load() ? 1 : 0,
        g_affinity_enabled.load() ? 1 : 0,
        g_quiet_log_enabled.load() ? 1 : 0,
        g_fast_save_enabled.load() ? 1 : 0,
        g_no_telemetry_enabled.load() ? 1 : 0);

    if (g_orig_update_one_frame.load() != 0) {
        uint64_t r = g_frames_rendered.load(std::memory_order_relaxed);
        uint64_t extra = g_burst_extra_ticks.load(std::memory_order_relaxed);
        double worst_ms = static_cast<double>(g_worst_frame_ns.load(std::memory_order_relaxed)) / 1e6;
        double ema_r_ms = static_cast<double>(g_ema_render_ns.load(std::memory_order_relaxed)) / 1e6;
        put("Frame Pacing   : %llu frames (%.1f FPS) | worst_frame=%.1f ms | avg_render=%.1f ms | extra_ticks=%llu\r\n",
            static_cast<unsigned long long>(r),
            static_cast<double>(r) / wall_s,
            worst_ms,
            ema_r_ms,
            static_cast<unsigned long long>(extra));

        uint64_t act_f = g_active_sim_frames.load(std::memory_order_relaxed);
        uint64_t act_ns = g_active_sim_ns.load(std::memory_order_relaxed);
        if (act_f > 0 && act_ns > 0) {
            double act_s = static_cast<double>(act_ns) / 1e9;
            put("Unpaused Sim   : %llu frames in %.2fs => %.1f FPS | %.2f days/s active\r\n",
                static_cast<unsigned long long>(act_f),
                act_s,
                static_cast<double>(act_f) / act_s,
                days / act_s);
        }
    }
    if (sf_fps > 0) {
        put("Anti-Freeze    : smooth_fps=%llu | mid_tick_checks=%llu (fast_skipped=%llu) | mid_tick_renders=%llu\r\n",
            static_cast<unsigned long long>(sf_fps),
            static_cast<unsigned long long>(g_mid_tick_checks.load()),
            static_cast<unsigned long long>(g_mid_tick_fast_skip.load()),
            static_cast<unsigned long long>(g_mid_tick_renders.load()));
    }
    if (g_fast_sched_enabled.load() || g_fast_yield_enabled.load()) {
        put("FastSched      : %llu zero-lock watcher polls | %llu SwitchToThread syscalls elided\r\n",
            static_cast<unsigned long long>(g_sched_watcher_polls.load()),
            static_cast<unsigned long long>(g_yield_elided.load()));
    }
    if (g_quiet_log_enabled.load()) {
        put("QuietLog       : %llu allowed | %llu spam lines suppressed\r\n",
            static_cast<unsigned long long>(g_log_allowed.load()),
            static_cast<unsigned long long>(g_log_suppressed.load()));
    }
    if (g_fast_save_enabled.load()) {
        put("FastSave       : %llu blocking FlushFileBuffers() stalls eliminated\r\n",
            static_cast<unsigned long long>(g_fsync_skipped.load()));
    }
    if (g_no_telemetry_enabled.load()) {
        put("NoTelemetry    : %llu SDK telemetry payloads suppressed cleanly\r\n",
            static_cast<unsigned long long>(g_telemetry_blocked.load()));
    }
    if (g_fast_mod_enabled.load()) {
        put("FastModifier   : Branchless CMOV binary search active\r\n");
    }
    if (g_country_grain_enabled.load()) {
        put("CountryGrain   : %llu/%llu dispatches rebalanced (17..=220 -> chunk=1)\r\n",
            static_cast<unsigned long long>(g_parfor_rebalanced.load()),
            static_cast<unsigned long long>(g_parfor_total.load()));
    }
    if (g_fast_alloc_enabled.load()) {
        double used_mb = static_cast<double>(g_slab_bump_off.load()) / (1024.0 * 1024.0);
        put("FastAlloc      : %llu slab allocs | %llu slab frees | %llu >512B fallbacks | %.1f/128.0 MB committed\r\n",
            static_cast<unsigned long long>(g_slab_alloc_hits.load()),
            static_cast<unsigned long long>(g_slab_free_hits.load()),
            static_cast<unsigned long long>(g_slab_fallbacks.load()),
            used_mb);
    }
    if (g_civil_war_fix_enabled.load()) {
        put("CivilWarFix    : %llu redundant mid-split map/supply rebuilds coalesced\r\n",
            static_cast<unsigned long long>(g_cw_rebuilds_coalesced.load()));
    }
    if (g_ai_smart_supply_enabled.load()) {
        put("SmartSupply    : %llu wartime land supply_node hubs auto-queued (max 1 active/country, 3-hop BFS)\r\n",
            static_cast<unsigned long long>(g_ai_supply_hubs_queued.load()));
    }
    if (g_ai_smart_airbase_enabled.load()) {
        put("SmartAirbase   : %llu queued air_base priority evaluations boosted (3.2x)\r\n",
            static_cast<unsigned long long>(g_airbase_prio_boosts.load()));
    }
    if (g_ai_tactical_prudence_enabled.load()) {
        put("TacticalAI     : Sheep-AI v3 prudence & total empty-tile fix active (applied=%d, empty_tile_patch=%d)\r\n",
            g_ai_tactical_prudence_applied.load() ? 1 : 0,
            g_empty_tile_fix_applied.load() ? 1 : 0);
    }
    if (g_ai_smart_armor_enabled.load()) {
        put("SmartArmor     : %llu industrial-scaled armor/mech strategy queries adjusted (defines_applied=%d)\r\n",
            static_cast<unsigned long long>(g_ai_armor_strategy_boosts.load()),
            g_ai_smart_armor_defines_applied.load() ? 1 : 0);
    }
    if (g_ai_smart_peace_enabled.load()) {
        put("SmartPeace     : %llu peace conference state scores adjusted for border contiguity\r\n",
            static_cast<unsigned long long>(g_peace_bordergore_adjusts.load()));
    }
    if (g_ai_throttle_minors_enabled.load()) {
        put("MinorAIThrot   : %llu peaceful minor sub-passes skipped | %llu active AI passes executed\r\n",
            static_cast<unsigned long long>(g_minor_ai_skipped.load()),
            static_cast<unsigned long long>(g_minor_ai_executed.load()));
    }

    put("\r\n--- Top Profiler Zones ---\r\n");
    put("%-44s %8s %9s %8s %9s %9s %7s\r\n",
        "Zone", "Calls", "Total ms", "Avg us", "Main ms", "Work ms", "%Wall");
    for (size_t i = 0; i < top_n; ++i) {
        char zname[45] = {0};
        std::snprintf(zname, sizeof(zname), "%.44s", rows[i].name);
        double total_ms = static_cast<double>(rows[i].total) / 1e6;
        double avg_us = (static_cast<double>(rows[i].total) / static_cast<double>(rows[i].calls)) / 1e3;
        double main_ms = static_cast<double>(rows[i].main_ns) / 1e6;
        double work_ms = static_cast<double>(rows[i].worker_ns) / 1e6;
        double pct_wall = (main_ms / (wall_s * 1000.0)) * 100.0;
        put("%-44s %8llu %9.2f %8.1f %9.2f %9.2f %6.1f%%\r\n",
            zname,
            static_cast<unsigned long long>(rows[i].calls),
            total_ms,
            avg_us,
            main_ms,
            work_ms,
            pct_wall);
    }
    put("==============================================================\r\n");

    append_log_line(buf, false);
}

static bool get_cfg_str(const char* env_key, const wchar_t* ini_key, char* out, size_t cap) {
    char env_buf[128] = {0};
    DWORD n = GetEnvironmentVariableA(env_key, env_buf, static_cast<DWORD>(sizeof(env_buf)));
    if (n > 0 && n < sizeof(env_buf)) {
        size_t copy_len = (n < cap - 1) ? static_cast<size_t>(n) : (cap - 1);
        std::memcpy(out, env_buf, copy_len);
        out[copy_len] = '\0';
        return true;
    }
    if (g_ini_path[0] != L'\0') {
        wchar_t wbuf[128] = {0};
        DWORD wn = GetPrivateProfileStringW(L"hoi4_accel", ini_key, L"", wbuf, 127, g_ini_path);
        if (wn > 0) {
            // Strip inline ';' or '#' comments and trailing spaces
            size_t j = 0;
            for (DWORD i = 0; i < wn && j + 1 < cap; ++i) {
                wchar_t wc = wbuf[i];
                if (wc == L';' || wc == L'#') {
                    break;
                }
                out[j++] = static_cast<char>(wc & 0x7f);
            }
            while (j > 0 && (out[j - 1] == ' ' || out[j - 1] == '\t')) {
                --j;
            }
            out[j] = '\0';
            if (j > 0) {
                return true;
            }
        }
    }
    return false;
}

static bool get_cfg_bool(const char* env_key, const wchar_t* ini_key, bool def_val) {
    char val[64] = {0};
    if (!get_cfg_str(env_key, ini_key, val, sizeof(val))) {
        return def_val;
    }
    if (std::strcmp(val, "1") == 0 || _stricmp(val, "true") == 0 || _stricmp(val, "on") == 0) {
        return true;
    }
    if (std::strcmp(val, "0") == 0 || _stricmp(val, "false") == 0 || _stricmp(val, "off") == 0) {
        return false;
    }
    return def_val;
}

static uint64_t get_cfg_u64(const char* env_key, const wchar_t* ini_key, uint64_t def_val) {
    char val[64] = {0};
    if (!get_cfg_str(env_key, ini_key, val, sizeof(val))) {
        return def_val;
    }
    char* endp = nullptr;
    unsigned long long parsed = std::strtoull(val, &endp, 10);
    return (endp != val) ? static_cast<uint64_t>(parsed) : def_val;
}

static void maybe_emit_exit_report() {
    if (!g_active_for_hoi4.load(std::memory_order_relaxed)) {
        return;
    }
    if (g_bench_max_hours.load(std::memory_order_relaxed) == 0 &&
        g_collect_zones.load(std::memory_order_relaxed)) {
        uint64_t h = g_total_hours.load(std::memory_order_relaxed);
        if (h > 0 && !g_report_emitted.exchange(true, std::memory_order_acq_rel)) {
            g_bench_recording.store(false, std::memory_order_release);
            emit_report(h);
        }
    }
}

// ============================================================================
// 12. Initialization for hoi4.exe & DllMain Entry Point
// ============================================================================
static void hoi4_accel_init(HINSTANCE hinst_dll) {
    wchar_t exe_path[MAX_PATH] = {0};
    DWORD n_exe = GetModuleFileNameW(nullptr, exe_path, MAX_PATH);
    if (n_exe == 0 || n_exe >= MAX_PATH) {
        return;
    }
    const wchar_t* base_name = std::wcsrchr(exe_path, L'\\');
    base_name = base_name ? (base_name + 1) : exe_path;
    if (_wcsicmp(base_name, L"hoi4.exe") != 0) {
        // Not hoi4.exe (e.g. dowser.exe or crashreporter.exe) -> pure version.dll passthrough!
        return;
    }

    // Resolve paths next to version.dll / hoi4.exe
    wchar_t dir_path[MAX_PATH] = {0};
    if (GetModuleFileNameW(hinst_dll, dir_path, MAX_PATH) > 0) {
        wchar_t* slash = std::wcsrchr(dir_path, L'\\');
        if (slash) {
            *slash = L'\0';
            std::swprintf(g_ini_path, MAX_PATH, L"%ls\\hoi4_accel.ini", dir_path);
            std::swprintf(g_log_path, MAX_PATH, L"%ls\\hoi4_accel.log", dir_path);
        }
    }
    wchar_t env_log[MAX_PATH] = {0};
    DWORD n_env_log = GetEnvironmentVariableW(L"HOI4_ACCEL_LOG", env_log, MAX_PATH);
    if (n_env_log > 0 && n_env_log < MAX_PATH) {
        std::memcpy(g_log_path, env_log, (n_env_log + 1) * sizeof(wchar_t));
    }

    LARGE_INTEGER freq{};
    if (QueryPerformanceFrequency(&freq) && freq.QuadPart > 0) {
        g_qpc_freq = static_cast<uint64_t>(freq.QuadPart);
    }

    g_exe_base = reinterpret_cast<uintptr_t>(GetModuleHandleW(nullptr));
    g_main_tid = GetCurrentThreadId();
    g_tls_yield_idx = TlsAlloc();
    g_tls_slab_idx = TlsAlloc();
    g_active_for_hoi4.store(true, std::memory_order_release);

    if (std::strcmp(
            reinterpret_cast<const char*>(g_exe_base + RVA_STR_GAMESTATE_HOURLY),
            "gamestate.hourly") == 0) {
        g_hourly_ptr.store(g_exe_base + RVA_STR_GAMESTATE_HOURLY, std::memory_order_relaxed);
    }

    bool noop_mode = get_cfg_bool("HOI4_ACCEL_NOOP", L"noop", false);
    char preset[32] = "smooth";
    if (noop_mode) {
        std::strcpy(preset, "none");
    } else {
        get_cfg_str("HOI4_ACCEL_PRESET", L"preset", preset, sizeof(preset));
        for (char* p = preset; *p; ++p) {
            *p = static_cast<char>(std::tolower(static_cast<unsigned char>(*p)));
        }
    }

    uint64_t def_fps = 0;
    uint64_t def_smooth = 0;
    bool def_core = false;
    if (std::strcmp(preset, "smooth") == 0) {
        def_fps = 26;
        def_smooth = 30;
        def_core = true;
    } else if (std::strcmp(preset, "balanced") == 0) {
        def_fps = 14;
        def_smooth = 14;
        def_core = true;
    } else if (std::strcmp(preset, "turbo") == 0) {
        def_fps = 6;
        def_smooth = 6;
        def_core = true;
    } else if (std::strcmp(preset, "pure") == 0) {
        def_fps = 0;
        def_smooth = 30;
        def_core = true;
    }

    char hdr[512];
    std::snprintf(
        hdr,
        sizeof(hdr),
        "[hoi4-accel v1.3-win64] Attached to hoi4.exe (pid=%lu, base=0x%llx, preset=%s)\r\n",
        static_cast<unsigned long>(GetCurrentProcessId()),
        static_cast<unsigned long long>(g_exe_base),
        preset
    );
    append_log_line(hdr, true);

    bool want_ai_smart_supply = !noop_mode &&
                                get_cfg_bool("HOI4_ACCEL_AI_SMART_SUPPLY", L"ai_smart_supply", def_core);
    if (want_ai_smart_supply) {
        g_ai_smart_supply_enabled.store(true, std::memory_order_release);
        uintptr_t tramp_sb = install_trampoline_hook(
            g_exe_base + RVA_FIX_SUPPLY_BOTTLENECKS,
            SIG_FIX_SUPPLY_BOTTLENECKS,
            sizeof(SIG_FIX_SUPPLY_BOTTLENECKS),
            reinterpret_cast<uintptr_t>(&hook_fix_supply_bottlenecks)
        );
        if (tramp_sb != 0) {
            g_orig_fix_supply_bottlenecks.store(tramp_sb, std::memory_order_release);
            append_log_line("[hoi4-accel] Enabled AiSmartSupply v2 wartime land supply_node builder (FixSupplyBottlenecks @ 0x13a9fc0)\r\n");
        } else {
            append_log_line("[hoi4-accel] SKIPPED AiSmartSupply v2 hub hook (prologue signature mismatch)\r\n");
        }
    }
    bool want_ai_smart_airbase = !noop_mode &&
                                 get_cfg_bool("HOI4_ACCEL_AI_SMART_AIRBASE", L"ai_smart_airbase", def_core);
    if (want_ai_smart_airbase) {
        uintptr_t tramp_ab = install_trampoline_hook(
            g_exe_base + RVA_GET_BLDG_QUEUE_PRIO,
            SIG_GET_BLDG_QUEUE_PRIO,
            sizeof(SIG_GET_BLDG_QUEUE_PRIO),
            reinterpret_cast<uintptr_t>(&hook_get_bldg_queue_prio)
        );
        if (tramp_ab != 0) {
            g_orig_get_bldg_queue_prio.store(tramp_ab, std::memory_order_release);
            g_ai_smart_airbase_enabled.store(true, std::memory_order_release);
            append_log_line("[hoi4-accel] Enabled AiSmartAirbase (CAIConstruction::GetBuildingQueuePrio @ 0x1395220)\r\n");
        } else {
            want_ai_smart_airbase = false;
            append_log_line("[hoi4-accel] SKIPPED AiSmartAirbase (signature mismatch)\r\n");
        }
    }
    bool want_ai_tactical_prudence = !noop_mode &&
                                     get_cfg_bool("HOI4_ACCEL_AI_TACTICAL_PRUDENCE", L"ai_tactical_prudence", def_core);
    if (want_ai_tactical_prudence) {
        g_ai_tactical_prudence_enabled.store(true, std::memory_order_release);
        static constexpr uint8_t kNop2[2] = {0x90, 0x90};
        if (verify_sig(g_exe_base + RVA_EMPTY_TILE_SIG, SIG_EMPTY_TILE_BRANCH, sizeof(SIG_EMPTY_TILE_BRANCH)) &&
            patch_bytes(g_exe_base + RVA_EMPTY_TILE_PATCH1, kNop2, 2) &&
            patch_bytes(g_exe_base + RVA_EMPTY_TILE_PATCH2, kNop2, 2)) {
            g_empty_tile_fix_applied.store(true, std::memory_order_release);
            append_log_line("[hoi4-accel] Enabled Empty-Tile Stall Fix (CUnitOrder @ 0x1573929,0x1573933 -> NOP; CanMoveIntoProvince intact)\r\n");
        } else {
            append_log_line("[hoi4-accel] SKIPPED Empty-Tile Stall binary patch (signature mismatch)\r\n");
        }
    }
    bool want_ai_smart_armor = !noop_mode &&
                               get_cfg_bool("HOI4_ACCEL_AI_SMART_ARMOR", L"ai_smart_armor", def_core);
    if (want_ai_smart_armor) {
        uintptr_t strat_slot = g_exe_base + RVA_STRATEGY_STORE_VT + OFF_STRATEGY_GET_VAL_SLOT;
        uintptr_t cur_strat  = *reinterpret_cast<const uintptr_t*>(strat_slot);
        if (cur_strat == g_exe_base + RVA_STRATEGY_GET_VAL_FN) {
            g_orig_get_strategy_value.store(cur_strat, std::memory_order_release);
            if (patch_data_ptr(strat_slot, reinterpret_cast<uintptr_t>(&hook_get_strategy_value))) {
                g_ai_smart_armor_enabled.store(true, std::memory_order_release);
                append_log_line("[hoi4-accel] Enabled AiSmartArmor (CAIStrategyStore::GetStrategyValue vtable @ 0x27b8be0+0x20)\r\n");
            } else {
                want_ai_smart_armor = false;
            }
        } else {
            want_ai_smart_armor = false;
            append_log_line("[hoi4-accel] SKIPPED AiSmartArmor (vtable mismatch)\r\n");
        }
    }
    bool want_ai_smart_peace = !noop_mode &&
                               get_cfg_bool("HOI4_ACCEL_AI_SMART_PEACE", L"ai_smart_peace", def_core);
    if (want_ai_smart_peace) {
        if (verify_sig(
                g_exe_base + RVA_PEACE_IS_STATE_CLAIMED,
                SIG_PEACE_IS_STATE_CLAIMED,
                sizeof(SIG_PEACE_IS_STATE_CLAIMED))) {
            uintptr_t tramp_pc = install_trampoline_hook(
                g_exe_base + RVA_PEACE_CALC_STATE_SCORE,
                SIG_PEACE_CALC_STATE_SCORE,
                sizeof(SIG_PEACE_CALC_STATE_SCORE),
                reinterpret_cast<uintptr_t>(&hook_peace_calc_state_score)
            );
            if (tramp_pc != 0) {
                g_orig_peace_calc_state_score.store(tramp_pc, std::memory_order_release);
                g_ai_smart_peace_enabled.store(true, std::memory_order_release);
                append_log_line("[hoi4-accel] Enabled AiSmartPeace (anti-bordergore CPeaceWinnerAi::CalcStateActionScore @ 0x1a8ca20)\r\n");
            } else {
                want_ai_smart_peace = false;
                append_log_line("[hoi4-accel] SKIPPED AiSmartPeace (prologue signature mismatch)\r\n");
            }
        } else {
            want_ai_smart_peace = false;
            append_log_line("[hoi4-accel] SKIPPED AiSmartPeace (IsStateClaimedBy signature mismatch)\r\n");
        }
    }

    bool need_scoped_timer = false;
    uint64_t bench_hours = get_cfg_u64("HOI4_BENCH_HOURS", L"bench_hours", 0);
    if (bench_hours > 0) {
        g_bench_max_hours.store(bench_hours, std::memory_order_relaxed);
        g_bench_warmup_hours.store(
            get_cfg_u64("HOI4_BENCH_WARMUP", L"bench_warmup", 24),
            std::memory_order_relaxed
        );
        g_collect_zones.store(true, std::memory_order_relaxed);
        need_scoped_timer = true;
    }
    if (get_cfg_bool("HOI4_ACCEL_PROFILE", L"profile", false)) {
        g_collect_zones.store(true, std::memory_order_relaxed);
        g_bench_start_ns.store(now_ns(), std::memory_order_relaxed);
        g_bench_start_cpu_ns.store(now_cpu_ns(), std::memory_order_relaxed);
        g_bench_recording.store(true, std::memory_order_release);
        need_scoped_timer = true;
    }

    // 1. Frame tracker + Render-Aware Multi-Tick Speed-5 Bursting (also drives lazy AI NDefines init)
    uint64_t target_fps = noop_mode ? 0 : get_cfg_u64("HOI4_ACCEL_TARGET_FPS", L"target_fps", def_fps);
    bool need_frame_hooks = (target_fps >= 1 && target_fps <= 240) ||
                            bench_hours > 0 ||
                            g_collect_zones.load(std::memory_order_relaxed) ||
                            want_ai_smart_supply ||
                            want_ai_smart_airbase ||
                            want_ai_tactical_prudence ||
                            want_ai_smart_armor ||
                            want_ai_smart_peace;
    if (need_frame_hooks) {
        uint64_t max_burst = get_cfg_u64("HOI4_ACCEL_MAX_BURST", L"max_burst", 16);
        if (max_burst < 1) max_burst = 1;
        if (max_burst > 64) max_burst = 64;
        g_max_burst_ticks.store(max_burst, std::memory_order_relaxed);

        uintptr_t tramp_frame = install_trampoline_hook(
            g_exe_base + RVA_UPDATE_ONE_FRAME,
            SIG_UPDATE_ONE_FRAME,
            sizeof(SIG_UPDATE_ONE_FRAME),
            reinterpret_cast<uintptr_t>(&hook_update_one_frame)
        );
        if (tramp_frame != 0) {
            g_orig_update_one_frame.store(tramp_frame, std::memory_order_release);
        }

        if (verify_sig(g_exe_base + RVA_GAMESTATE_TICK_CLOCK, SIG_TICK_CLOCK, sizeof(SIG_TICK_CLOCK))) {
            uintptr_t tramp_sess = install_trampoline_hook(
                g_exe_base + RVA_SESSION_UPDATE,
                SIG_SESSION_UPDATE,
                sizeof(SIG_SESSION_UPDATE),
                reinterpret_cast<uintptr_t>(&hook_session_update)
            );
            if (tramp_sess != 0) {
                g_orig_session_update.store(tramp_sess, std::memory_order_release);
                uint64_t frame_ns = (target_fps >= 1 && target_fps <= 240)
                                        ? (1000000000ULL / target_fps)
                                        : 0;
                g_target_frame_ns.store(frame_ns, std::memory_order_release);
                if (frame_ns > 0) {
                    need_scoped_timer = true;
                    char msg[256];
                    std::snprintf(
                        msg, sizeof(msg),
                        "[hoi4-accel] Enabled Render-Aware Speed-5 Bursting (target=%llu FPS, max_burst=%llu)\r\n",
                        static_cast<unsigned long long>(target_fps),
                        static_cast<unsigned long long>(max_burst)
                    );
                    append_log_line(msg);
                }
            } else {
                append_log_line("[hoi4-accel] SKIPPED SessionUpdate hook (signature mismatch)\r\n");
            }
        }
    }

    // 2. Mid-Tick Anti-Freeze Frame Pacer (ShouldRenderMidTick @ 0x2bacd0)
    uint64_t smooth_fps = noop_mode ? 0 : get_cfg_u64("HOI4_ACCEL_SMOOTH_FPS", L"smooth_fps", def_smooth);
    if (smooth_fps >= 1 && smooth_fps <= 240) {
        if (verify_sig(
                g_exe_base + RVA_SHOULD_RENDER_MIDTICK,
                SIG_SHOULD_RENDER_MIDTICK,
                sizeof(SIG_SHOULD_RENDER_MIDTICK)) &&
            install_abs_jmp(
                g_exe_base + RVA_SHOULD_RENDER_MIDTICK,
                reinterpret_cast<uintptr_t>(&hook_should_render_mid_tick))) {
            g_smooth_fps.store(smooth_fps, std::memory_order_release);
            char msg[256];
            std::snprintf(
                msg, sizeof(msg),
                "[hoi4-accel] Enabled Mid-Tick Anti-Freeze Frame Pacer (smooth=%llu FPS, camera=60 FPS)\r\n",
                static_cast<unsigned long long>(smooth_fps)
            );
            append_log_line(msg);
        } else {
            append_log_line("[hoi4-accel] SKIPPED ShouldRenderMidTick hook (signature mismatch)\r\n");
        }
    }

    // 3. CScopedTimer hooks (required for hour tracking in Speed-5 bursting or profiling)
    if (need_scoped_timer) {
        if (verify_sig(
                g_exe_base + RVA_SCOPED_TIMER_CTOR,
                SIG_SCOPED_TIMER_CTOR,
                sizeof(SIG_SCOPED_TIMER_CTOR)) &&
            verify_sig(
                g_exe_base + RVA_SCOPED_TIMER_DTOR,
                SIG_SCOPED_TIMER_DTOR,
                sizeof(SIG_SCOPED_TIMER_DTOR)) &&
            install_abs_jmp(
                g_exe_base + RVA_SCOPED_TIMER_CTOR,
                reinterpret_cast<uintptr_t>(&hook_scoped_timer_ctor)) &&
            install_abs_jmp(
                g_exe_base + RVA_SCOPED_TIMER_DTOR,
                reinterpret_cast<uintptr_t>(&hook_scoped_timer_dtor))) {
            append_log_line("[hoi4-accel] Installed CScopedTimer ctor/dtor hooks\r\n");
        } else {
            append_log_line("[hoi4-accel] SKIPPED CScopedTimer hooks (signature mismatch)\r\n");
        }
    }

    // 4. Lock-free Zero-Bounce Worker Scheduler (PopTaskOrWait @ 0x255d2a0)
    if (!noop_mode && get_cfg_bool("HOI4_ACCEL_FAST_SCHED", L"fast_sched", def_core)) {
        if (verify_sig(
                g_exe_base + RVA_POP_TASK_OR_WAIT,
                SIG_POP_TASK_OR_WAIT,
                sizeof(SIG_POP_TASK_OR_WAIT)) &&
            install_abs_jmp(
                g_exe_base + RVA_POP_TASK_OR_WAIT,
                reinterpret_cast<uintptr_t>(&hook_pop_task_or_wait))) {
            g_fast_sched_enabled.store(true, std::memory_order_release);
            append_log_line("[hoi4-accel] Enabled FastSched (zero-bounce PopTaskOrWait @ 0x255d2a0)\r\n");
        } else {
            append_log_line("[hoi4-accel] SKIPPED FastSched (signature mismatch)\r\n");
        }
    }

    // Capture original SwitchToThread IAT pointer before patching FastWait / FastYield
    g_orig_switch_to_thread.store(
        *reinterpret_cast<const uintptr_t*>(g_exe_base + RVA_IAT_SWITCH_TO_THREAD),
        std::memory_order_release
    );

    // 5. FastWait (CTask::SpinWait @ 0x255afb0)
    if (!noop_mode && get_cfg_bool("HOI4_ACCEL_FAST_WAIT", L"fast_wait", def_core)) {
        if (verify_sig(g_exe_base + RVA_TASK_WAIT, SIG_TASK_WAIT, sizeof(SIG_TASK_WAIT)) &&
            install_abs_jmp(
                g_exe_base + RVA_TASK_WAIT,
                reinterpret_cast<uintptr_t>(&hook_task_wait))) {
            g_fast_wait_enabled.store(true, std::memory_order_release);
            append_log_line("[hoi4-accel] Enabled FastWait (lock-free CTask::SpinWait @ 0x255afb0)\r\n");
        } else {
            append_log_line("[hoi4-accel] SKIPPED FastWait (signature mismatch)\r\n");
        }
    }

    // 6. FastYield (SwitchToThread IAT @ 0x27892c0, verified by thunk @ 0x25a6e48)
    if (!noop_mode && get_cfg_bool("HOI4_ACCEL_FAST_YIELD", L"fast_yield", def_core)) {
        if (verify_sig(
                g_exe_base + RVA_SWITCH_TO_THREAD_THUNK,
                SIG_SWITCH_TO_THREAD_THUNK,
                sizeof(SIG_SWITCH_TO_THREAD_THUNK)) &&
            patch_data_ptr(
                g_exe_base + RVA_IAT_SWITCH_TO_THREAD,
                reinterpret_cast<uintptr_t>(&hook_switch_to_thread))) {
            g_fast_yield_enabled.store(true, std::memory_order_release);
            append_log_line("[hoi4-accel] Enabled FastYield (SwitchToThread kernel-trap storm elided)\r\n");
        } else {
            append_log_line("[hoi4-accel] SKIPPED FastYield (signature mismatch)\r\n");
        }
    }

    // 7. NoJobClock (strip per-microjob clock + lock add in ExecuteTask)
    if (!noop_mode && get_cfg_bool("HOI4_ACCEL_NO_JOB_CLOCK", L"no_job_clock", def_core)) {
        uint8_t nops49[49];
        std::memset(nops49, 0x90, sizeof(nops49));
        if (verify_sig(g_exe_base + RVA_EXEC_TASK_NOW1, SIG_EXEC_TASK_NOW1, sizeof(SIG_EXEC_TASK_NOW1)) &&
            verify_sig(g_exe_base + RVA_EXEC_TASK_NOW2, SIG_EXEC_TASK_NOW2, sizeof(SIG_EXEC_TASK_NOW2)) &&
            verify_sig(
                g_exe_base + RVA_EXEC_TASK_LOCKADD_END,
                SIG_EXEC_TASK_LOCKADD_END,
                sizeof(SIG_EXEC_TASK_LOCKADD_END)) &&
            patch_bytes(g_exe_base + RVA_EXEC_TASK_NOW1, nops49, 5) &&
            patch_bytes(g_exe_base + RVA_EXEC_TASK_NOW2, nops49, 49)) {
            g_no_job_clock_enabled.store(true, std::memory_order_release);
            append_log_line("[hoi4-accel] Enabled NoJobClock (stripped ExecuteTask per-job clocks + lock add)\r\n");
        } else {
            append_log_line("[hoi4-accel] SKIPPED NoJobClock (signature mismatch)\r\n");
        }
    }

    // 8. Topology-Aware CPU Core Pinning (WorkerThreadInit @ 0x255a110)
    if (!noop_mode && get_cfg_bool("HOI4_ACCEL_AFFINITY", L"affinity", def_core)) {
        if (init_cpu_topology() &&
            verify_sig(
                g_exe_base + RVA_WORKER_THREAD_INIT,
                SIG_WORKER_THREAD_INIT,
                sizeof(SIG_WORKER_THREAD_INIT)) &&
            install_abs_jmp(
                g_exe_base + RVA_WORKER_THREAD_INIT,
                reinterpret_cast<uintptr_t>(&hook_worker_thread_init))) {
            g_affinity_enabled.store(true, std::memory_order_release);
            SetThreadAffinityMask(GetCurrentThread(), 1ULL);
            char msg[256];
            std::snprintf(
                msg, sizeof(msg),
                "[hoi4-accel] Enabled Topology Core Pinning (main=CPU0, %zu worker slots, %zu bg slots)\r\n",
                g_topo_worker_len, g_topo_bg_len
            );
            append_log_line(msg);
        } else {
            append_log_line("[hoi4-accel] Affinity left to Windows OS scheduler (hybrid or unsupported layout)\r\n");
        }
    }

    // 9. QuietLog (SetLogger @ 0x2550050 + MSVC std::string deduplicator)
    if (!noop_mode && get_cfg_bool("HOI4_ACCEL_QUIET_LOG", L"quiet_log", def_core)) {
        if (verify_sig(g_exe_base + RVA_SET_LOGGER, SIG_SET_LOGGER, sizeof(SIG_SET_LOGGER)) &&
            install_abs_jmp(
                g_exe_base + RVA_SET_LOGGER,
                reinterpret_cast<uintptr_t>(&hook_set_logger))) {
            auto* cur_logger = *reinterpret_cast<uint8_t* const*>(g_exe_base + RVA_G_LOGGER_PTR);
            if (cur_logger) {
                wrap_logger_vtable(cur_logger);
            }
            g_quiet_log_enabled.store(true, std::memory_order_release);
            append_log_line("[hoi4-accel] Enabled QuietLog (lock-free error.log spam deduplicator)\r\n");
        } else {
            append_log_line("[hoi4-accel] SKIPPED QuietLog (signature mismatch)\r\n");
        }
    }

    // 10. FastSave (CFile::Flush @ 0x259d9e0)
    if (!noop_mode && get_cfg_bool("HOI4_ACCEL_FAST_SAVE", L"fast_save", def_core)) {
        if (verify_sig(g_exe_base + RVA_FILE_FLUSH, SIG_FILE_FLUSH, sizeof(SIG_FILE_FLUSH)) &&
            install_abs_jmp(
                g_exe_base + RVA_FILE_FLUSH,
                reinterpret_cast<uintptr_t>(&hook_fast_flush))) {
            g_fast_save_enabled.store(true, std::memory_order_release);
            append_log_line("[hoi4-accel] Enabled FastSave (skipped blocking FlushFileBuffers stalls)\r\n");
        } else {
            append_log_line("[hoi4-accel] SKIPPED FastSave (signature mismatch)\r\n");
        }
    }

    // 11. NoTelemetry (IAT QueueAndSendTelemetry @ 0x2789a90 with MSVC payload dtor)
    if (!noop_mode && get_cfg_bool("HOI4_ACCEL_NO_TELEMETRY", L"no_telemetry", def_core)) {
        if (verify_sig(
                g_exe_base + RVA_TELEMETRY_CALL,
                SIG_TELEMETRY_CALL,
                sizeof(SIG_TELEMETRY_CALL)) &&
            patch_data_ptr(
                g_exe_base + RVA_IAT_QUEUE_TELEMETRY,
                reinterpret_cast<uintptr_t>(&hook_queue_and_send_telemetry))) {
            g_no_telemetry_enabled.store(true, std::memory_order_release);
            append_log_line("[hoi4-accel] Enabled NoTelemetry (Paradox SDK telemetry disabled cleanly)\r\n");
        } else {
            append_log_line("[hoi4-accel] SKIPPED NoTelemetry (signature mismatch)\r\n");
        }
    }

    // 12. CountryGrain (CPdxParallelForRange::Init @ 0x2576ac0)
    if (!noop_mode && get_cfg_bool("HOI4_ACCEL_COUNTRY_GRAIN", L"country_grain", def_core)) {
        uintptr_t tramp_pf = install_trampoline_hook(
            g_exe_base + RVA_PARFOR_RANGE_INIT,
            SIG_PARFOR_RANGE_INIT,
            sizeof(SIG_PARFOR_RANGE_INIT),
            reinterpret_cast<uintptr_t>(&hook_parfor_range_init)
        );
        if (tramp_pf != 0) {
            g_orig_parfor_range_init.store(tramp_pf, std::memory_order_release);
            g_country_grain_enabled.store(true, std::memory_order_release);
            append_log_line("[hoi4-accel] Enabled CountryGrain (17..=220 -> chunk=1 @ 0x2576ac0)\r\n");
        } else {
            append_log_line("[hoi4-accel] SKIPPED CountryGrain (signature mismatch)\r\n");
        }
    }

    // 13. FastModifier (Branchless CMOV CModifier::GetValue @ 0x5a63d0)
    if (!noop_mode && get_cfg_bool("HOI4_ACCEL_FAST_MOD", L"fast_mod", def_core)) {
        uintptr_t tramp_mod = install_trampoline_hook(
            g_exe_base + RVA_MODIFIER_GET_VAL,
            SIG_MODIFIER_GET_VAL,
            sizeof(SIG_MODIFIER_GET_VAL),
            reinterpret_cast<uintptr_t>(&hook_modifier_get_value)
        );
        if (tramp_mod != 0) {
            g_orig_modifier_get_val.store(tramp_mod, std::memory_order_release);
            g_fast_mod_enabled.store(true, std::memory_order_release);
            append_log_line("[hoi4-accel] Enabled FastModifier (branchless CMOV binary search @ 0x5a63d0)\r\n");
        } else {
            append_log_line("[hoi4-accel] SKIPPED FastModifier (signature mismatch)\r\n");
        }
    }

    // 14. FastAlloc (128 MB Lock-Free Slab + Win32 TLS Magazine on CPdxNewDeleteAllocator @ 0x2798570)
    if (!noop_mode && get_cfg_bool("HOI4_ACCEL_FAST_ALLOC", L"fast_alloc", def_core)) {
        uintptr_t alloc_slot = g_exe_base + RVA_PDX_ALLOC_VT + OFF_PDX_ALLOC_SLOT;
        uintptr_t free_slot  = g_exe_base + RVA_PDX_ALLOC_VT + OFF_PDX_FREE_SLOT;
        uintptr_t cur_alloc  = *reinterpret_cast<const uintptr_t*>(alloc_slot);
        uintptr_t cur_free   = *reinterpret_cast<const uintptr_t*>(free_slot);
        if (cur_alloc == g_exe_base + RVA_PDX_ALLOC_FN &&
            cur_free  == g_exe_base + RVA_PDX_FREE_FN &&
            init_slab_arena()) {
            g_orig_pdx_alloc.store(cur_alloc, std::memory_order_release);
            g_orig_pdx_free.store(cur_free, std::memory_order_release);
            if (patch_data_ptr(alloc_slot, reinterpret_cast<uintptr_t>(&hook_pdx_alloc)) &&
                patch_data_ptr(free_slot, reinterpret_cast<uintptr_t>(&hook_pdx_free))) {
                g_fast_alloc_enabled.store(true, std::memory_order_release);
                append_log_line("[hoi4-accel] Enabled FastAlloc (128 MB lock-free slab + TLS magazine @ 0x2798570)\r\n");
            }
        } else {
            append_log_line("[hoi4-accel] SKIPPED FastAlloc (vtable mismatch or arena error)\r\n");
        }
    }

    // 15. CivilWarFix (CCivilWarSetup::Execute @ 0x1169250 + CInGameIdler map rebuild coalescer)
    if (!noop_mode && get_cfg_bool("HOI4_ACCEL_CIVIL_WAR_FIX", L"civil_war_fix", def_core)) {
        uintptr_t idler_slot = g_exe_base + RVA_INGAME_IDLER_VT + OFF_IDLER_MAP_REBUILD_SLOT;
        uintptr_t cur_rebuild = *reinterpret_cast<const uintptr_t*>(idler_slot);
        if (cur_rebuild == g_exe_base + RVA_IDLER_MAP_REBUILD_FN) {
            uintptr_t tramp_cw = install_trampoline_hook(
                g_exe_base + RVA_CW_SETUP_EXEC,
                SIG_CW_SETUP_EXEC,
                sizeof(SIG_CW_SETUP_EXEC),
                reinterpret_cast<uintptr_t>(&hook_cw_setup_exec)
            );
            if (tramp_cw != 0) {
                g_orig_cw_setup_exec.store(tramp_cw, std::memory_order_release);
                g_orig_idler_map_rebuild.store(cur_rebuild, std::memory_order_release);
                if (patch_data_ptr(idler_slot, reinterpret_cast<uintptr_t>(&hook_idler_map_rebuild))) {
                    g_civil_war_fix_enabled.store(true, std::memory_order_release);
                    append_log_line("[hoi4-accel] Enabled CivilWarFix (coalesced per-state CInGameIdler map/supply rebuilds)\r\n");
                }
            } else {
                append_log_line("[hoi4-accel] SKIPPED CivilWarFix (prologue signature mismatch)\r\n");
            }
        } else {
            append_log_line("[hoi4-accel] SKIPPED CivilWarFix (vtable mismatch)\r\n");
        }
    }

    // 16. AiThrottleMinors (CCountryAI @ 0x27b8c20)
    if (!noop_mode && get_cfg_bool("HOI4_ACCEL_AI_THROTTLE_MINORS", L"ai_throttle_minors", def_core)) {
        uintptr_t p1_slot = g_exe_base + RVA_COUNTRY_AI_VT + OFF_COUNTRY_AI_PASS1_SLOT;
        uintptr_t p2_slot = g_exe_base + RVA_COUNTRY_AI_VT + OFF_COUNTRY_AI_PASS2_SLOT;
        uintptr_t cur_p1  = *reinterpret_cast<const uintptr_t*>(p1_slot);
        uintptr_t cur_p2  = *reinterpret_cast<const uintptr_t*>(p2_slot);
        if (cur_p1 == g_exe_base + RVA_COUNTRY_AI_PASS1_FN &&
            cur_p2 == g_exe_base + RVA_COUNTRY_AI_PASS2_FN) {
            g_orig_country_ai_pass1.store(cur_p1, std::memory_order_release);
            g_orig_country_ai_pass2.store(cur_p2, std::memory_order_release);
            if (patch_data_ptr(p1_slot, reinterpret_cast<uintptr_t>(&hook_country_ai_pass1)) &&
                patch_data_ptr(p2_slot, reinterpret_cast<uintptr_t>(&hook_country_ai_pass2))) {
                g_ai_throttle_minors_enabled.store(true, std::memory_order_release);
                append_log_line("[hoi4-accel] Enabled AiThrottleMinors (staggered 4h cadence for peaceful <12-state non-major AI minors)\r\n");
            }
        } else {
            append_log_line("[hoi4-accel] SKIPPED AiThrottleMinors (vtable mismatch)\r\n");
        }
    }

    std::atexit(&maybe_emit_exit_report);
}

extern "C" BOOL WINAPI DllMain(HINSTANCE hinstDLL, DWORD fdwReason, LPVOID /*lpvReserved*/) {
    if (fdwReason == DLL_PROCESS_ATTACH) {
        DisableThreadLibraryCalls(hinstDLL);
        load_system_version_dll();
        hoi4_accel_init(hinstDLL);
    } else if (fdwReason == DLL_PROCESS_DETACH) {
        maybe_emit_exit_report();
    }
    return TRUE;
}
