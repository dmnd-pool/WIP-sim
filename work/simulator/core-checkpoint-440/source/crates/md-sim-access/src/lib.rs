//! Validated dashboard access configuration. Existing OpenSSH and Nginx provide
//! SSH, TLS and HTTP framing; this crate binds them to one recorded run.
pub mod browser;
pub mod carrier;
pub mod carrier_gateway;
pub mod carrier_inputs;
pub mod credentials;
pub mod gateway;
pub mod remote;
pub mod remote_carrier;
pub mod selected;
