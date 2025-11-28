pub mod common;
pub mod decimal;
pub mod rate;

pub use decimal::Decimal;
pub use rate::Rate;

pub trait TryAdd: Sized {
    fn try_add(self, rhs: Self) -> Result<Self, crate::error::LendingError>;
}

pub trait TrySub: Sized {
    fn try_sub(self, rhs: Self) -> Result<Self, crate::error::LendingError>;
}

pub trait TryDiv<Rhs = Self>: Sized {
    fn try_div(self, rhs: Rhs) -> Result<Self, crate::error::LendingError>;
}

pub trait TryMul<Rhs = Self>: Sized {
    fn try_mul(self, rhs: Rhs) -> Result<Self, crate::error::LendingError>;
}