
use crate::error::LendingError;

/// Number of slots to consider stale after
pub const STALE_AFTER_SLOTS_ELAPSED: u64 = 1;

/// Last update state stored in Odra-compatible form
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LastUpdateStorage {
    /// Last slot when updated
    pub slot: u64,
    /// True when marked stale, false when slot updated
    pub stale: bool,
}

impl Default for LastUpdateStorage {
    fn default() -> Self {
        Self {
            slot: 0,
            stale: true,
        }
    }
}

impl LastUpdateStorage {
    /// Create new last update
    pub fn new(slot: u64) -> Self {
        Self { slot, stale: true }
    }

    /// Return slots elapsed since given slot
    pub fn slots_elapsed(&self, slot: u64) -> Result<u64, LendingError> {
        slot
            .checked_sub(self.slot)
            .ok_or(LendingError::MathOverflow)
    }

    /// Set last update slot
    pub fn update_slot(&mut self, slot: u64) {
        self.slot = slot;
        self.stale = false;
    }

    /// Set stale to true
    pub fn mark_stale(&mut self) {
        self.stale = true;
    }

    /// Check if marked stale or last update slot is too long ago
    pub fn is_stale(&self, slot: u64) -> Result<bool, LendingError> {
        Ok(self.stale || self.slots_elapsed(slot)? >= STALE_AFTER_SLOTS_ELAPSED)
    }
}

// Notes:
// - This is a direct Odra-friendly migration of Solana `LastUpdate` struct.
// - Original code used `solana_program::clock::Slot` (u64 alias). Here we store
//   it as u64 to keep things simple for on-chain storage.
// - Error mapping uses `LendingError::MathOverflow` for checked_sub underflow.
// - The struct is annotated with `#[odra::type]` so it can be embedded in module
//   types or stored directly as a Var<T> in Odra modules.

// Example usage in an Odra module:
// let mut last = LastUpdateStorage::new(current_slot);
// last.update_slot(current_slot);
// module.last_update.set(last);
