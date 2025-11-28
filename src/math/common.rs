//! Common module for Decimal and Rate (Odra migration)
//!
//! Contains shared scalers and arithmetic trait definitions. Traits return
//! `LendingError` (odra-friendly) instead of `ProgramError` used in Solana.

use crate::error::LendingError;

/// Scale of precision
pub const SCALE: usize = 18;
/// Identity
pub const WAD: u64 = 1_000_000_000_000_000_000;
/// Half of identity
pub const HALF_WAD: u64 = 500_000_000_000_000_000;
/// Scale for percentages
pub const PERCENT_SCALER: u64 = 10_000_000_000_000_000;

/// Try to subtract, return an error on underflow
pub trait TrySub: Sized {
    /// Subtract
    fn try_sub(self, rhs: Self) -> Result<Self, LendingError>;
}

/// Try to add, return an error on overflow
pub trait TryAdd: Sized {
    /// Add
    fn try_add(self, rhs: Self) -> Result<Self, LendingError>;
}

/// Try to divide, return an error on overflow or divide by zero
pub trait TryDiv<RHS>: Sized {
    /// Divide
    fn try_div(self, rhs: RHS) -> Result<Self, LendingError>;
}

/// Try to multiply, return an error on overflow
pub trait TryMul<RHS>: Sized {
    /// Multiply
    fn try_mul(self, rhs: RHS) -> Result<Self, LendingError>;
}

// Notes:
// - This module replaces `solana_program::program_error::ProgramError` with
//   the crate-specific `LendingError` so the math primitives integrate with
//   the Odra/Casper error handling used across migrated modules.
// - Keep the numeric constants (`WAD`, `PERCENT_SCALER`) as `u64` to remain
//   compatible with other migrated files which cast to `u128` when needed.
