use super::process_identity;
use crate::{isolation::ProcessSpec, store::ObjectId};
use anyhow::{ensure, Context, Result};
use serde_json::{json, Value};
use std::{
    fs::File,
    sync::{
        atomic::{AtomicU8, Ordering},
        mpsc::SyncSender,
        Arc,
    },
};

const WAITING: u8 = 0;
const ACTIVE: u8 = 1;
const CLOSED: u8 = 2;
const FAILED: u8 = 3;

/// Frozen outer-provider identities. The trusted coordinator validates the
/// full lock and exact policy membership before constructing this contract.
#[derive(Clone, Debug)]
pub struct PrivateChannelLeaseSpec {
    pub locked_run: ObjectId,
    pub process: ObjectId,
    pub policy: ObjectId,
    pub support: ObjectId,
    pub configuration: ObjectId,
}

impl PrivateChannelLeaseSpec {
    fn validate(&self) -> Result<()> {
        for object in [
            &self.locked_run,
            &self.process,
            &self.policy,
            &self.support,
            &self.configuration,
        ] {
            object.validate()?;
            ensure!(object.size > 0, "empty provider lease identity");
        }
        Ok(())
    }
}

struct Lease {
    spec: PrivateChannelLeaseSpec,
    run: Option<(String, String)>,
    state: Arc<AtomicU8>,
    sequence: u64,
}

/// Owned by the real provider thread. An unfinished exit, including unwinding,
/// is failed. Only that thread may declare handoff or a completed relay exit.
pub struct PrivateChannelLifecycle {
    state: Arc<AtomicU8>,
    finished: bool,
}

impl PrivateChannelLifecycle {
    pub fn active(&self) -> Result<()> {
        ensure!(
            self.state
                .compare_exchange(WAITING, ACTIVE, Ordering::AcqRel, Ordering::Acquire)
                .is_ok(),
            "provider handoff already completed or failed"
        );
        Ok(())
    }

    /// Call only after the actual handoff/relay operation has returned.
    pub fn finish(&mut self, failed: bool) {
        if !self.finished {
            self.state
                .store(if failed { FAILED } else { CLOSED }, Ordering::Release);
            self.finished = true;
        }
    }
}

impl Drop for PrivateChannelLifecycle {
    fn drop(&mut self) {
        if !self.finished {
            self.state.store(FAILED, Ordering::Release);
        }
    }
}

fn strict_token(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

/// A trusted worker's one-use provider for an exact canonical process identity.
/// This is local configuration. It is never accepted through the request wire.
/// The binding is consumed before run creation. A later storage or launch
/// failure does not restore it; a new provider needs explicit configuration.
pub struct PrivateChannelBinding {
    identity: ObjectId,
    provider: Option<SyncSender<File>>,
    lease: Option<Lease>,
}

impl PrivateChannelBinding {
    pub fn new(identity: ObjectId, provider: SyncSender<File>) -> Result<Self> {
        identity.validate()?;
        ensure!(identity.size > 0, "empty private channel process identity");
        Ok(Self {
            identity,
            provider: Some(provider),
            lease: None,
        })
    }

    pub fn with_lease(
        mut self,
        spec: PrivateChannelLeaseSpec,
    ) -> Result<(Self, PrivateChannelLifecycle)> {
        spec.validate()?;
        ensure!(
            spec.process == self.identity && self.lease.is_none(),
            "provider lease process differs or lease already exists"
        );
        let state = Arc::new(AtomicU8::new(WAITING));
        self.lease = Some(Lease {
            spec,
            run: None,
            state: state.clone(),
            sequence: 0,
        });
        Ok((
            self,
            PrivateChannelLifecycle {
                state,
                finished: false,
            },
        ))
    }

    pub(crate) fn check_token(&self, token: Option<&str>) -> Result<()> {
        if self.lease.is_some() {
            ensure!(
                token.is_some_and(strict_token),
                "provider lease requires a strict execution token"
            );
        }
        Ok(())
    }

    pub(crate) fn bind_run(&mut self, token: Option<&str>, run_id: &str) -> Result<()> {
        if let Some(lease) = &mut self.lease {
            let token = token.context("provider lease token absent")?;
            ensure!(
                strict_token(token) && run_id == format!("run-{token}") && lease.run.is_none(),
                "provider lease run binding differs"
            );
            lease.run = Some((token.into(), run_id.into()));
        }
        Ok(())
    }

    pub(crate) fn lease(&mut self, token: &str, challenge: &str, found: &Value) -> Result<Value> {
        ensure!(
            strict_token(token) && strict_token(challenge),
            "provider token or challenge is not lowercase 64-digit hex"
        );
        let lease = self
            .lease
            .as_mut()
            .context("private channel lease is not configured")?;
        let (bound_token, run_id) = lease
            .run
            .as_ref()
            .context("provider has no committed execution token")?;
        ensure!(
            bound_token == token
                && found["found"] == true
                && found["token"] == token
                && found["run_id"] == *run_id
                && found["token_process_identity"] == serde_json::to_value(&lease.spec.process)?,
            "provider query does not match the existing selected execution"
        );
        lease.sequence = lease
            .sequence
            .checked_add(1)
            .context("provider observation sequence exhausted")?;
        let state = match lease.state.load(Ordering::Acquire) {
            WAITING => "waiting",
            ACTIVE => "active",
            CLOSED => "closed",
            _ => "failed",
        };
        Ok(json!({"schema":"md-sim-carrier-provider-lease/1",
            "identity":{"locked_run":lease.spec.locked_run,"process":lease.spec.process,"policy":lease.spec.policy,
                "support":lease.spec.support,"token":token,"run_id":run_id},
            "configuration":lease.spec.configuration,"challenge":challenge,"sequence":lease.sequence,"state":state}))
    }

    pub(crate) fn check(&self, spec: &ProcessSpec) -> Result<()> {
        ensure!(
            spec.private_channel.is_some(),
            "private channel policy is required"
        );
        ensure!(
            process_identity(spec)? == self.identity,
            "private channel process identity mismatch"
        );
        ensure!(
            self.provider.is_some(),
            "private channel binding already consumed"
        );
        Ok(())
    }

    pub(crate) fn take(&mut self, spec: &ProcessSpec) -> Result<SyncSender<File>> {
        self.check(spec)?;
        self.provider
            .take()
            .context("private channel binding already consumed")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn selected() -> (
        PrivateChannelBinding,
        PrivateChannelLifecycle,
        Value,
        String,
    ) {
        let object = ObjectId {
            sha256: "a".repeat(64),
            size: 1,
        };
        let (sender, _receiver) = std::sync::mpsc::sync_channel(1);
        let (mut binding, lifecycle) = PrivateChannelBinding::new(object.clone(), sender)
            .unwrap()
            .with_lease(PrivateChannelLeaseSpec {
                locked_run: object.clone(),
                process: object.clone(),
                policy: object.clone(),
                support: object.clone(),
                configuration: object.clone(),
            })
            .unwrap();
        let token = "b".repeat(64);
        let run = format!("run-{token}");
        binding.bind_run(Some(&token), &run).unwrap();
        let found =
            json!({"found":true,"token":token,"run_id":run,"token_process_identity":object});
        (binding, lifecycle, found, token)
    }

    #[test]
    fn provider_observation_requires_fresh_valid_selection_and_keeps_terminal_state() {
        let (mut binding, mut lifecycle, found, token) = selected();
        let challenge = "c".repeat(64);
        assert_eq!(
            binding.lease(&token, &challenge, &found).unwrap()["state"],
            "waiting"
        );
        for invalid in [
            String::new(),
            "C".repeat(64),
            "g".repeat(64),
            "c".repeat(63),
            "c".repeat(65),
        ] {
            assert!(binding.lease(&token, &invalid, &found).is_err());
        }
        assert!(binding.lease(&"d".repeat(64), &challenge, &found).is_err());
        let mut changed = found.clone();
        changed["token_process_identity"]["size"] = json!(2);
        assert!(binding.lease(&token, &challenge, &changed).is_err());
        lifecycle.active().unwrap();
        let active = binding.lease(&token, &challenge, &found).unwrap();
        assert_eq!(active["state"], "active");
        assert_eq!(active["sequence"], 2);
        assert_eq!(active["challenge"], challenge);
        lifecycle.finish(false);
        assert!(lifecycle.active().is_err());
        drop(lifecycle);
        assert_eq!(
            binding.lease(&token, &challenge, &found).unwrap()["state"],
            "closed"
        );
    }

    #[test]
    fn provider_thread_panic_irreversibly_fails_its_own_monitor() {
        let (mut binding, lifecycle, found, token) = selected();
        let thread = std::thread::spawn(move || {
            lifecycle.active().unwrap();
            panic!("injected provider-thread panic");
        });
        assert!(thread.join().is_err());
        let lease = binding.lease(&token, &"c".repeat(64), &found).unwrap();
        assert_eq!(lease["state"], "failed");
        assert_eq!(lease["sequence"], 1);
    }
}
