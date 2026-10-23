//! One functional hashing budget for every chain in a run.
//! Elapsed host time limits grants and never accumulates idle credit.
//! Chains take turns, weighted by enabled cores. Virtual polling has its
//! own interval; it is not a claim about guest or firmware hash rate.
//! Bus and protection timers do not use this budget.
use md_sim_mining::engine::RateLimiter;
use std::{
    sync::{Arc, Mutex},
    time::Instant,
};

/// One actual admission from the shared run budget. This clock is separate
/// from virtual bus time and remains continuous across device resets.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Grant {
    pub ordinal: u64,
    pub elapsed_ns: u64,
}

pub struct Budget {
    rate: RateLimiter,
    origin: Instant,
    grants: u64,
    interval_ns: u64,
    members: Vec<(bool, usize)>, // attached, enabled cores
    turn: usize,
    round: usize,
}

/// The shared object belongs to one simulation run, not a process global.
pub type Shared = Arc<Mutex<Budget>>;

pub struct Slot {
    budget: Shared,
    index: usize,
    interval_ns: u64,
}

impl Budget {
    pub fn new(hps: f64) -> Result<Shared, String> {
        // Bound timer conversion before constructing a host Instant.
        let ns = (1_000_000_000.0 / hps).ceil();
        if !hps.is_finite()
            || hps <= 0.0
            || hps > 1_000_000_000.0
            || !ns.is_finite()
            || ns >= u64::MAX as f64
        {
            return Err("BM1387 hash cap must be positive and fit the timer range".into());
        }
        Ok(Arc::new(Mutex::new(Self {
            origin: Instant::now(),
            rate: RateLimiter::new(hps)?,
            grants: 0,
            interval_ns: ns as u64,
            members: Vec::new(),
            turn: 0,
            round: 0,
        })))
    }

    /// A slot cannot be cloned. Detaching it releases its scheduling turn.
    pub fn attach(budget: &Shared) -> Result<Slot, String> {
        let mut state = budget.lock().map_err(|_| "hash budget poisoned")?;
        let index = state
            .members
            .iter()
            .position(|(attached, _)| !attached)
            .unwrap_or(state.members.len());
        if index == state.members.len() {
            if index == 64 {
                return Err("hash budget has64 chain slots".into());
            }
            state.members.push((true, 0));
        } else {
            state.members[index] = (true, 0);
        }
        Ok(Slot {
            budget: Arc::clone(budget),
            index,
            interval_ns: state.interval_ns,
        })
    }

    fn next_active(&self) -> Option<(usize, usize, usize)> {
        let maximum = self.members.iter().map(|(_, cores)| *cores).max()?;
        if maximum == 0 {
            return None;
        }
        let mut turn = self.turn;
        let mut round = self.round % maximum;
        // The maximum-weight member is eligible in every round.
        for _ in 0..self.members.len() * 2 {
            let index = turn;
            let ready = self.members[index].0 && self.members[index].1 > round;
            turn += 1;
            if turn == self.members.len() {
                turn = 0;
                round = (round + 1) % maximum;
            }
            if ready {
                return Some((index, turn, round));
            }
        }
        unreachable!("at least one active chain in each round")
    }
}

impl Slot {
    pub fn interval_ns(&self) -> u64 {
        self.interval_ns
    }

    pub fn set_weight(&self, enabled_cores: usize) {
        let mut state = self.budget.lock().expect("exclusive model budget");
        state.members[self.index].1 = enabled_cores;
    }

    pub(super) fn try_acquire(&self) -> Option<Grant> {
        let mut state = self.budget.lock().expect("exclusive model budget");
        let (index, turn, round) = state.next_active()?;
        if index != self.index {
            return None;
        }
        let at = state.rate.try_acquire_at()?;
        state.turn = turn;
        state.round = round;
        state.grants = state
            .grants
            .checked_add(1)
            .expect("run grant count overflow");
        Some(Grant {
            ordinal: state.grants,
            elapsed_ns: at
                .duration_since(state.origin)
                .as_nanos()
                .try_into()
                .expect("run monotonic duration exceeds584 years"),
        })
    }
}

impl Drop for Slot {
    fn drop(&mut self) {
        if let Ok(mut state) = self.budget.lock() {
            state.members[self.index] = (false, 0);
        }
    }
}
