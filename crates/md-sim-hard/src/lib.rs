//! Hard-mode tools for faithful hardware simulation.
//!
//! The first tool compares the S9 FPGA circuit, rebuilt from the original
//! bitstream, with the Rust S9IO model that the S9 simulator uses. Both get
//! the same bus steps and serial input. A run prepares and calibrates once,
//! then compares random sequences in parallel until a deadline and keeps a
//! small JSON summary up to date. See `docs/hard-mode.md`.
pub mod bench;
pub mod calibrate;
pub mod circuit;
pub mod compare;
pub mod generate;
pub mod model;
pub mod rng;
pub mod run;
pub mod sequence;
