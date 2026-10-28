//! Functional Bitcoin work. This crate does not establish firmware execution.
//! The engine uses RustCrypto. The verifier uses rust-bitcoin and reconstructs
//! headers from pool jobs; it does not trust an engine-supplied digest or header.
pub mod engine;
pub mod pool;
pub mod regtest;
pub mod stratum;
pub mod target;
pub mod work;
pub use engine::{Computation, ComputedVariant, Engine, EngineEvent};
pub use pool::{PoolJob, PoolVerifier, Share, Verdict};
pub use target::{Filter, Target};
pub use work::{Header, MidstateWork, Work};

pub type Result<T> = std::result::Result<T, String>;

pub mod replay_s2;
