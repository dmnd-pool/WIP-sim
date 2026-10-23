//! Bounded support-tool budgets for the combined native dashboard experiment.
//! Browser022 measured249s and243s startup, and613s for its two-profile suite.
//! Native HTTP/RPC010 used332s without a browser. These limits allow both
//! paths in one run. Guest clocks, boot readiness and network deadlines keep
//! their existing contracts.
pub const HELPER_WALL_SECONDS: u64 = 780;
pub const SERVICE_CPU_SECONDS: u64 = 900;
pub const POST_READY_SECONDS: u64 = 1200;
// Native015's first content process was created32s after navigation began,
// after the30s WebDriver page deadline. Keep a bounded emulator allowance.
pub const PAGE_LOAD_MILLISECONDS: u32 = 90_000;
pub const COMMAND_SECONDS: u64 = 105;
pub const LOGIN_TELEMETRY_SECONDS: u64 = 30;
