//! One BM1387 work cursor over an explicit chip/core nonce map.
//! The caller shares one rate limiter across every chain in a run.
//! This component uses the existing SHA-256d implementation and preserves
//! the native programmed ticket filter. It does not establish pool validity.
use super::{filter::Policy, nonce::Layout, Error, Job};
use md_sim_mining::engine::RateLimiter;
use std::ops::RangeInclusive;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Attempt {
    pub job_id: u8,
    pub nonce: u32,
    pub midstate_index: usize,
    pub chain_position: usize,
    pub core_address: u8,
    pub digest: [u8; 32],
    /// Present only for a computed digest that meets the ASIC filter.
    pub response: Option<[u8; 7]>,
}

pub struct Search {
    job: Job,
    layout: Layout,
    ordinal: u64,
    last_ordinal: u64,
    midstate: usize,
    tickets: Vec<u32>,
    hashes: u64,
    exhausted: bool,
    filter_policy: Policy,
}

impl Search {
    /// Ordinals select a subset of the already permitted nonce map.
    /// The device uses its full map; a fixed regression may use a small
    /// interval. No answer or nonce is supplied by this constructor.
    pub fn new(
        job: Job,
        layout: Layout,
        ticket: u32,
        ordinals: RangeInclusive<u64>,
    ) -> Result<Self, Error> {
        job.validate()?;
        if ordinals.is_empty() || *ordinals.end() >= layout.candidate_count() {
            return Err(Error::NonceRange);
        }
        Ok(Self {
            job,
            tickets: vec![ticket; layout.chip_count()],
            layout,
            ordinal: *ordinals.start(),
            last_ordinal: *ordinals.end(),
            midstate: 0,
            hashes: 0,
            exhausted: false,
            filter_policy: Policy::default(),
        })
    }

    pub fn exhausted(&self) -> bool {
        self.exhausted
    }

    pub fn hashes_computed(&self) -> u64 {
        self.hashes
    }

    pub fn set_ticket_mask(&mut self, ticket: u32) {
        self.tickets.fill(ticket);
    }

    pub(super) fn set_filter_policy(&mut self, policy: Policy) {
        self.filter_policy = policy;
    }

    pub fn set_chip_ticket(&mut self, chip: usize, ticket: u32) -> Result<(), Error> {
        *self.tickets.get_mut(chip).ok_or(Error::NonceRange)? = ticket;
        Ok(())
    }

    /// Compute at most one due attempt. Idle/exhausted cursors consume no
    /// rate grant. Diagnostic digests are never labelled accepted shares.
    pub fn poll(&mut self, budget: &mut RateLimiter) -> Result<Option<Attempt>, Error> {
        if self.exhausted || !budget.try_acquire() {
            return Ok(None);
        }
        self.compute().map(Some)
    }

    /// Internal caller must already hold one aggregate rate grant.
    /// One invocation performs exactly one SHA-256d candidate computation.
    pub(super) fn compute(&mut self) -> Result<Attempt, Error> {
        if self.exhausted {
            return Err(Error::NonceRange);
        }
        let nonce = self.layout.at(self.ordinal).ok_or(Error::NonceRange)?;
        let (chain_position, core_address) = self.layout.owner(nonce).ok_or(Error::NonceRange)?;
        let digest = self.job.digest(self.midstate, nonce)?;
        self.hashes += 1;
        let response = if self
            .filter_policy
            .programmed_filter(self.tickets[chain_position])
            .accepts(&digest)
        {
            Some(self.job.encode_response(self.midstate, nonce, 0))
        } else {
            None
        };
        let attempt = Attempt {
            job_id: self.job.id,
            nonce,
            midstate_index: self.midstate,
            chain_position,
            core_address,
            digest,
            response,
        };
        self.midstate += 1;
        if self.midstate == self.job.states.len() {
            self.midstate = 0;
            if self.ordinal == self.last_ordinal {
                self.exhausted = true;
            } else {
                self.ordinal += 1;
            }
        }
        Ok(attempt)
    }
}
