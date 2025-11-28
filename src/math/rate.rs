//! Rate type for interest rate calculations using U256

use {
    crate::{error::LendingError, math::{common::*, TryMul}},
    odra::casper_types::U256,
    core::fmt,
    alloc::{
        format,
        string::ToString,
    }
};

/// Interest rate as a scaled value
#[derive(Clone, Copy, Debug, Default, PartialEq, PartialOrd, Eq, Ord)]
pub struct Rate(pub U256);

// Manual Odra implementations for Rate
impl odra::casper_types::bytesrepr::ToBytes for Rate {
    fn to_bytes(&self) -> Result<alloc::vec::Vec<u8>, odra::casper_types::bytesrepr::Error> {
        self.0.to_bytes()
    }

    fn serialized_length(&self) -> usize {
        self.0.serialized_length()
    }
}

impl odra::casper_types::bytesrepr::FromBytes for Rate {
    fn from_bytes(bytes: &[u8]) -> Result<(Self, &[u8]), odra::casper_types::bytesrepr::Error> {
        let (value, remainder) = U256::from_bytes(bytes)?;
        Ok((Rate(value), remainder))
    }
}

impl odra::casper_types::CLTyped for Rate {
    fn cl_type() -> odra::casper_types::CLType {
        odra::casper_types::CLType::U256
    }
}

impl Rate {
    /// One (100%)
    pub fn one() -> Self { 
        Self(Self::wad()) 
    }

    /// Zero (0%)
    pub fn zero() -> Self { 
        Self(U256::zero()) 
    }

    fn wad() -> U256 { 
        U256::from(WAD) 
    }

    /// Create rate from percent value (0-100)
    pub fn from_percent(percent: u8) -> Self { 
        Self(U256::from(percent as u64 * PERCENT_SCALER)) 
    }

    /// Return raw scaled value as u128
    pub fn to_scaled_val(&self) -> u128 { 
        self.0.as_u128()
    }

    /// Create rate from scaled value
    pub fn from_scaled_val(scaled_val: u128) -> Self { 
        Self(U256::from(scaled_val)) 
    }

    /// Calculate power (for compound interest)
    pub fn try_pow(&self, exponent: u64) -> Result<Self, LendingError> {
        if exponent == 0 {
            return Ok(Self::one());
        }

        let mut result = Self::one();
        let mut base = *self;
        let mut exp = exponent;

        while exp > 0 {
            if exp % 2 == 1 {
                result = result.try_mul(base)?;
            }
            base = base.try_mul(base)?;
            exp /= 2;
        }

        Ok(result)
    }
}

impl fmt::Display for Rate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut scaled_val = self.0.to_string();
        if scaled_val.len() <= SCALE {
            let padding = "0".repeat(SCALE - scaled_val.len());
            scaled_val = format!("0.{}{}", padding, scaled_val);
        } else {
            scaled_val.insert(scaled_val.len() - SCALE, '.');
        }
        f.write_str(&scaled_val)
    }
}

impl From<u64> for Rate { 
    fn from(val: u64) -> Self { 
        Self(Self::wad().checked_mul(U256::from(val)).unwrap_or(U256::zero())) 
    } 
}

// NEW: Add conversion from Decimal to Rate
impl From<crate::math::Decimal> for Rate {
    fn from(decimal: crate::math::Decimal) -> Self {
        Self::from_scaled_val(decimal.to_scaled_val())
    }
}

impl crate::math::TryAdd for Rate {
    fn try_add(self, rhs: Self) -> Result<Self, LendingError> {
        Ok(Self(self.0.checked_add(rhs.0).ok_or(LendingError::MathOverflow)?))
    }
}

impl crate::math::TrySub for Rate {
    fn try_sub(self, rhs: Self) -> Result<Self, LendingError> {
        Ok(Self(self.0.checked_sub(rhs.0).ok_or(LendingError::MathOverflow)?))
    }
}

impl crate::math::TryDiv<u64> for Rate {
    fn try_div(self, rhs: u64) -> Result<Self, LendingError> {
        Ok(Self(self.0.checked_div(U256::from(rhs)).ok_or(LendingError::MathOverflow)?))
    }
}

impl crate::math::TryDiv<Rate> for Rate {
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

impl crate::math::TryMul<u64> for Rate {
    fn try_mul(self, rhs: u64) -> Result<Self, LendingError> {
        Ok(Self(self.0.checked_mul(U256::from(rhs)).ok_or(LendingError::MathOverflow)?))
    }
}

impl crate::math::TryMul<Rate> for Rate {
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
    fn test_rate_percent() {
        let rate = Rate::from_percent(5);
        assert_eq!(rate.to_scaled_val(), 50_000_000_000_000_000);
    }

    #[test]
    fn test_rate_pow() {
        let rate = Rate::from_percent(10);
        let squared = rate.try_pow(2).unwrap();
        assert!(squared.0 < rate.0); // Logic: 0.1 * 0.1 = 0.01 (lebih kecil)
    }
}