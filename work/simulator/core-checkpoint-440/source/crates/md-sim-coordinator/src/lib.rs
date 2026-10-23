//! Locked execution plans and durable, capability-scoped suite attempts.
//! A source-interface fixture never establishes exact physical-target coverage.
pub mod executor;
pub mod plan;
pub mod queue;
mod remote_trace;
pub mod ssh_transport;

pub use executor::{
    Executor, ExecutorIdentity, LocalExecutor, RetentionIdentity, RetentionRequest, WorkerExecutor,
    WorkerTransport,
};
pub use plan::*;
pub use queue::{request_cancel, Queue, QueueReport};
pub use ssh_transport::{OwnedSshChild, SshConfig, SshTransport, SshTunnel};
