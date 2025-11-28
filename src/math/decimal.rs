//! Odra-friendly Decimal (scaled) implementation migrated from Solana project.

#![allow(clippy::assign_op_pattern)]
#![allow(clippy::ptr_offset_with_cast)]
#![allow(clippy::manual_range_contains)]
#![allow(missing_docs)]

use {
    crate::{error::LendingError, math::{common::*}},
    odra::casper_types::U256,
    core::fmt,
    alloc::{
        string::ToString,
        vec,
    }
};

/// Large decimal values, precise to 18 digits
#[derive(Clone, Copy, Debug, Default, PartialEq, PartialOrd, Eq, Ord)]
pub struct Decimal(pub U256);

// Manual Odra implementations for Decimal
impl odra::casper_types::bytesrepr::ToBytes for Decimal {
    fn to_bytes(&self) -> Result<alloc::vec::Vec<u8>, odra::casper_types::bytesrepr::Error> {
        self.0.to_bytes()
    }

    fn serialized_length(&self) -> usize {
        self.0.serialized_length()
    }
}

impl odra::casper_types::bytesrepr::FromBytes for Decimal {
    fn from_bytes(bytes: &[u8]) -> Result<(Self, &[u8]), odra::casper_types::bytesrepr::Error> {
        let (value, remainder) = U256::from_bytes(bytes)?;
        Ok((Decimal(value), remainder))
    }
}

impl odra::casper_types::CLTyped for Decimal {
    fn cl_type() -> odra::casper_types::CLType {
        odra::casper_types::CLType::U256
    }
}

impl Decimal {
    /// One
    pub fn one() -> Self { 
        Self(U256::from(WAD)) 
    }

    /// Zero
    pub fn zero() -> Self { 
        Self(U256::zero()) 
    }

    fn wad() -> U256 { 
        U256::from(WAD) 
    }
    
    fn half_wad() -> U256 { 
        U256::from(HALF_WAD) 
    }

    /// Create scaled decimal from percent value
    pub fn from_percent(percent: u8) -> Self { 
        Self(U256::from(percent as u64 * PERCENT_SCALER)) 
    }

    /// Return raw scaled value as u128 (assumes value fits into u128)
    #[allow(clippy::wrong_self_convention)]
    pub fn to_scaled_val(&self) -> u128 { 
        self.0.as_u128()
    }

    /// Create decimal from scaled value
    pub fn from_scaled_val(scaled_val: u128) -> Self { 
        Self(U256::from(scaled_val)) 
    }

    /// Round scaled decimal to u64
    pub fn try_round_u64(&self) -> Result<u64, LendingError> {
        let rounded_val = Self::half_wad()
            .checked_add(self.0)
            .ok_or(LendingError::MathOverflow)?
            .checked_div(Self::wad())
            .ok_or(LendingError::MathOverflow)?;
        
        if rounded_val > U256::from(u64::MAX) {
            return Err(LendingError::MathOverflow);
        }
        Ok(rounded_val.as_u64())
    }

    /// Ceiling scaled decimal to u64
    pub fn try_ceil_u64(&self) -> Result<u64, LendingError> {
        let ceil_val = Self::wad()
            .checked_sub(U256::from(1u64))
            .ok_or(LendingError::MathOverflow)?
            .checked_add(self.0)
            .ok_or(LendingError::MathOverflow)?
            .checked_div(Self::wad())
            .ok_or(LendingError::MathOverflow)?;
        
        if ceil_val > U256::from(u64::MAX) {
            return Err(LendingError::MathOverflow);
        }
        Ok(ceil_val.as_u64())
    }

    /// Floor scaled decimal to u64
    pub fn try_floor_u64(&self) -> Result<u64, LendingError> {
        let floor_val = self.0.checked_div(Self::wad()).ok_or(LendingError::MathOverflow)?;
        
        if floor_val > U256::from(u64::MAX) {
            return Err(LendingError::MathOverflow);
        }
        Ok(floor_val.as_u64())
    }
}

impl fmt::Display for Decimal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut scaled_val = self.0.to_string();
        if scaled_val.len() <= SCALE {
            scaled_val.insert_str(0, &vec!["0"; SCALE - scaled_val.len()].join(""));
            scaled_val.insert_str(0, "0.");
        } else {
            scaled_val.insert(scaled_val.len() - SCALE, '.');
        }
        f.write_str(&scaled_val)
    }
}

impl From<u64> for Decimal { 
    fn from(val: u64) -> Self { 
        Self(Self::wad().checked_mul(U256::from(val)).unwrap_or(U256::zero())) 
    } 
}

impl From<u128> for Decimal { 
    fn from(val: u128) -> Self { 
        Self(Self::wad().checked_mul(U256::from(val)).unwrap_or(U256::zero())) 
    } 
}

// NEW: Add conversion from Rate to Decimal
impl From<crate::math::Rate> for Decimal {
    fn from(rate: crate::math::Rate) -> Self {
        Self::from_scaled_val(rate.to_scaled_val())
    }
}

// NEW: Implement division with Rate
impl crate::math::TryDiv<crate::math::Rate> for Decimal {
    fn try_div(self, rhs: crate::math::Rate) -> Result<Self, LendingError> {
        let rhs_decimal = Decimal::from(rhs);
        self.try_div(rhs_decimal)
    }
}

// NEW: Implement multiplication with Rate
impl crate::math::TryMul<crate::math::Rate> for Decimal {
    fn try_mul(self, rhs: crate::math::Rate) -> Result<Self, LendingError> {
        let rhs_decimal = Decimal::from(rhs);
        self.try_mul(rhs_decimal)
    }
}

impl crate::math::TryAdd for Decimal {
    fn try_add(self, rhs: Self) -> Result<Self, LendingError> {
        Ok(Self(self.0.checked_add(rhs.0).ok_or(LendingError::MathOverflow)?))
    }
}

impl crate::math::TrySub for Decimal {
    fn try_sub(self, rhs: Self) -> Result<Self, LendingError> {
        Ok(Self(self.0.checked_sub(rhs.0).ok_or(LendingError::MathOverflow)?))
    }
}

impl crate::math::TryDiv<u64> for Decimal {
    fn try_div(self, rhs: u64) -> Result<Self, LendingError> {
        Ok(Self(self.0.checked_div(U256::from(rhs)).ok_or(LendingError::MathOverflow)?))
    }
}

impl crate::math::TryDiv<Decimal> for Decimal {
    fn try_div(self, rhs: Self) -> Result<Self, LendingError> {
        Ok(Self(
            self.0
                .checked_mul(Self::wad())
                .ok_or(LendingError::MathOverflow)?
                .checked_div(rhs.0)
                .ok_or(LendingError::MathOverflow)?
        ))
    }
}

impl crate::math::TryMul<u64> for Decimal {
    fn try_mul(self, rhs: u64) -> Result<Self, LendingError> {
        Ok(Self(self.0.checked_mul(U256::from(rhs)).ok_or(LendingError::MathOverflow)?))
    }
}

impl crate::math::TryMul<Decimal> for Decimal {
    fn try_mul(self, rhs: Self) -> Result<Self, LendingError> {
        Ok(Self(
            self.0
                .checked_mul(rhs.0)
                .ok_or(LendingError::MathOverflow)?
                .checked_div(Self::wad())
                .ok_or(LendingError::MathOverflow)?
        ))
    }
}

#[cfg(test)]
mod test { 
    use super::*; 
    
    #[test] 
    fn test_scaler() { 
        assert_eq!(U256::from(WAD), Decimal::wad()); 
    } 
}