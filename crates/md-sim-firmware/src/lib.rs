//! Execution adapters for archived firmware. Source adaptations are separate
//! identities and never establish an original board boot result.
pub mod axi_gpio;
pub mod bitmain_s2;
pub mod bitstream;
pub mod bm1387;
pub mod bm1397;
pub mod esp32s3_adc_calibration;
pub mod esp32s3_uart;
pub mod fan;
pub mod pic16;
pub mod s9io;
pub mod serial8n1;
pub mod tmp421;

pub mod lab;
pub mod prepare;
pub mod schedule;
pub mod session;

mod confinement;

mod closure_bundle;
pub mod execution_suite;

pub mod s9_dashboard_state;
pub mod s9_telemetry;
pub mod s9_update_validation;
pub mod serial_console;
