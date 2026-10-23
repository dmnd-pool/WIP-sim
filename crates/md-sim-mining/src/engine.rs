use crate::{
    work::{digest_from_midstate, roll_version, MidstateWork},
    Filter, Header, Result, Work,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::VecDeque,
    time::{Duration, Instant},
};

/// One limiter must be shared by all hashboards in a run. Idle time never
/// creates a burst credit. The first hash is due after one full interval.
#[derive(Debug)]
pub struct RateLimiter {
    interval: Duration,
    next: Instant,
}
impl RateLimiter {
    pub fn new(hps: f64) -> Result<Self> {
        if !hps.is_finite() || hps <= 0.0 || hps > 1_000_000_000.0 {
            return Err("hash cap must be finite, positive and at most 1e9 H/s".into());
        }
        let seconds = 1.0 / hps;
        if seconds > u64::MAX as f64 {
            return Err("hash interval exceeds clock range".into());
        }
        // Round upward: rounding down could exceed the selected cap.
        let interval = Duration::try_from_secs_f64((seconds * 1e9).ceil() / 1e9)
            .map_err(|_| "hash interval exceeds clock range")?;
        let next = Instant::now()
            .checked_add(interval)
            .ok_or("hash interval exceeds clock range")?;
        Ok(Self { interval, next })
    }
    pub fn try_acquire(&mut self) -> bool {
        self.try_acquire_at().is_some()
    }
    /// Return the actual monotonic time used to admit this computation.
    /// Sampling after the hash or after trace delivery would hide its spacing.
    pub fn try_acquire_at(&mut self) -> Option<Instant> {
        let now = Instant::now();
        if now < self.next {
            return None;
        }
        self.next = now
            .checked_add(self.interval)
            .expect("validated clock interval");
        Some(now)
    }
    pub fn wait_duration(&self) -> Duration {
        self.next.saturating_duration_since(Instant::now())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum EngineEvent {
    Solution {
        work_id: String,
        nonce: u32,
        version: u32,
        digest: [u8; 32],
    },
    MidstateSolution {
        work_id: String,
        nonce: u32,
        midstate_index: usize,
        digest: [u8; 32],
    },
    Exhausted {
        work_id: String,
    },
}

enum HashInput {
    Header {
        header: Header,
        version_mask: u32,
    },
    Midstates {
        states: Vec<[u32; 8]>,
        tail: [u8; 12],
    },
}
struct Cursor {
    id: String,
    input: HashInput,
    filter: Filter,
    nonce_start: u32,
    nonce_end: u32,
    nonce: u32,
    version_index: u64,
}
impl Cursor {
    fn variant_count(&self) -> u64 {
        match &self.input {
            HashInput::Header { version_mask, .. } => 1u64 << version_mask.count_ones(),
            HashInput::Midstates { states, .. } => states.len() as u64,
        }
    }

    fn compute(&self) -> ([u8; 32], Option<u32>) {
        match &self.input {
            HashInput::Header {
                header,
                version_mask,
            } => {
                let mut header = header.clone();
                header.nonce = self.nonce;
                header.version = roll_version(header.version, *version_mask, self.version_index)
                    .expect("validated cursor index");
                (header.digest(), Some(header.version))
            }
            HashInput::Midstates { states, tail } => {
                let midstate_index = self.version_index as usize;
                let digest = digest_from_midstate(states[midstate_index], *tail, self.nonce);
                (digest, None)
            }
        }
    }
}

/// A single CPU hashing worker. Queue length is bounded. Each queue entry is
/// visited in round-robin order. Hardware topology belongs to the board model.
pub struct Engine {
    rate: RateLimiter,
    queue: VecDeque<Cursor>,
    queue_limit: usize,
    pub hashes_computed: u64,
}
impl Engine {
    pub fn new(hps: f64, queue_limit: usize) -> Result<Self> {
        if queue_limit == 0 {
            return Err("queue capacity must be positive".into());
        }
        Ok(Self {
            rate: RateLimiter::new(hps)?,
            queue: VecDeque::new(),
            queue_limit,
            hashes_computed: 0,
        })
    }
    pub fn submit(&mut self, work: Work) -> Result<()> {
        work.validate()?;
        self.enqueue(Cursor {
            id: work.id,
            input: HashInput::Header {
                header: work.header,
                version_mask: work.version_mask,
            },
            filter: work.filter,
            nonce_start: work.nonce_start,
            nonce_end: work.nonce_end,
            nonce: work.nonce_start,
            version_index: 0,
        })
    }
    pub fn submit_midstate(&mut self, work: MidstateWork) -> Result<()> {
        work.validate()?;
        self.enqueue(Cursor {
            id: work.id,
            input: HashInput::Midstates {
                states: work.states,
                tail: work.tail,
            },
            filter: work.filter,
            nonce_start: work.nonce_start,
            nonce_end: work.nonce_end,
            nonce: work.nonce_start,
            version_index: 0,
        })
    }
    fn enqueue(&mut self, cursor: Cursor) -> Result<()> {
        if self.queue.len() >= self.queue_limit {
            return Err("work FIFO overflow".into());
        }
        if self.queue.iter().any(|entry| entry.id == cursor.id) {
            return Err("duplicate live work ID".into());
        }
        self.queue.push_back(cursor);
        Ok(())
    }
    pub fn reset(&mut self) {
        self.queue.clear();
    }
    pub fn cancel(&mut self, id: &str) {
        self.queue.retain(|cursor| cursor.id != id);
    }
    pub fn queue_len(&self) -> usize {
        self.queue.len()
    }
    pub fn wait_duration(&self) -> Duration {
        self.rate.wait_duration()
    }

    /// Compute at most one due hash. Calling this early does no work. No
    /// fabricated event or counter is emitted when the rate limiter is idle.
    pub fn poll(&mut self) -> Vec<EngineEvent> {
        if self.queue.is_empty() || !self.rate.try_acquire() {
            return Vec::new();
        }
        let mut cursor = self.queue.pop_front().unwrap();
        let (digest, version) = cursor.compute();
        self.hashes_computed += 1;
        let mut events = Vec::new();
        if cursor.filter.accepts(&digest) {
            events.push(match version {
                Some(version) => EngineEvent::Solution {
                    work_id: cursor.id.clone(),
                    nonce: cursor.nonce,
                    version,
                    digest,
                },
                None => EngineEvent::MidstateSolution {
                    work_id: cursor.id.clone(),
                    nonce: cursor.nonce,
                    midstate_index: cursor.version_index as usize,
                    digest,
                },
            });
        }
        if cursor.nonce == cursor.nonce_end {
            cursor.nonce = cursor.nonce_start;
            cursor.version_index += 1;
        } else {
            cursor.nonce += 1;
        }
        if cursor.version_index == cursor.variant_count() {
            events.push(EngineEvent::Exhausted { work_id: cursor.id });
        } else {
            self.queue.push_back(cursor);
        }
        events
    }
}
impl Default for Engine {
    fn default() -> Self {
        Self::new(10_000.0, 64).unwrap()
    }
}
