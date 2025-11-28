// Odra-compatible state module root for `state` package

pub mod last_update;
pub mod lending_market;
pub mod obligation;
pub mod reserve;

pub use lending_market::*;
pub use reserve::*;

use crate::math::common::WAD; // FIXED IMPORT

/// Collateral tokens are initially valued at a ratio of 5:1
/// (collateral:liquidity)
// @FIXME: restore to 5
pub const INITIAL_COLLATERAL_RATIO: u64 = 1;
/// Scaled representation used for storage/Rate initialization
pub const INITIAL_COLLATERAL_RATE: u128 = INITIAL_COLLATERAL_RATIO as u128 * WAD as u128;

/// Current version of the program and all new accounts created
pub const PROGRAM_VERSION: u8 = 1;

/// Accounts are created with data zeroed out, so uninitialized state instances
/// will have the version set to 0.
pub const UNINITIALIZED_VERSION: u8 = 0;

/// Number of slots per year (used for interest compounding math)
pub const SLOTS_PER_YEAR: u64 = 31_536_000; // 365 * 24 * 60 * 60 — keep semantics simple for Odra

// Helper helpers for converting Decimal <-> scaled storage values are
// implemented in `crate::math::Decimal` as `to_scaled_val()` / `from_scaled_val()`.
// If you need byte-level packing for cross-chain migration, implement explicit
// serialize/deserialize helpers here.

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn initial_collateral_rate_sanity() {
        assert_eq!(INITIAL_COLLATERAL_RATE, INITIAL_COLLATERAL_RATIO as u128 * WAD as u128);
    }
}