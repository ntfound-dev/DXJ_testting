use odra::prelude::*;
use odra::casper_types::U256;
use crate::error::LendingError;
use crate::math::{decimal::Decimal, rate::Rate, TryAdd, TrySub, TryDiv, TryMul}; // ADDED TRAIT IMPORTS

// Konstanta Batas
pub const MAX_OBLIGATION_RESERVES: usize = 10;
pub const LIQUIDATION_CLOSE_FACTOR: u8 = 50; // 50%

// --- Obligation State ---

#[derive(Debug, Clone, PartialEq)]
pub struct Obligation {
    pub version: u8,
    pub last_update_slot: u64,
    pub last_update_stale: bool,
    pub lending_market: Address,
    pub owner: Address,
    pub deposits: Vec<ObligationCollateral>,
    pub borrows: Vec<ObligationLiquidity>,
    pub deposited_value: Decimal,
    pub borrowed_value: Decimal,
    pub allowed_borrow_value: Decimal,
    pub unhealthy_borrow_value: Decimal,
}

impl Obligation {
    pub fn new(owner: Address, lending_market: Address, current_slot: u64) -> Self {
        Self {
            version: 1,
            last_update_slot: current_slot,
            last_update_stale: true,
            lending_market,
            owner,
            deposits: Vec::new(),
            borrows: Vec::new(),
            deposited_value: Decimal::zero(),
            borrowed_value: Decimal::zero(),
            allowed_borrow_value: Decimal::zero(),
            unhealthy_borrow_value: Decimal::zero(),
        }
    }

    /// Menghitung Loan to Value (LTV) saat ini
    /// Ratio = Borrowed Value / Deposited Value
    pub fn loan_to_value(&self) -> Result<Decimal, LendingError> {
        if self.deposited_value == Decimal::zero() {
            return Ok(Decimal::zero());
        }
        self.borrowed_value.try_div(self.deposited_value)
    }

    /// Melunasi hutang (Repay) dan menghapusnya dari list jika lunas total
    pub fn repay(&mut self, settle_amount: Decimal, liquidity_index: usize) -> Result<(), LendingError> {
        if liquidity_index >= self.borrows.len() {
            return Err(LendingError::InvalidObligationLiquidity);
        }

        let liquidity = &mut self.borrows[liquidity_index];
        if settle_amount == liquidity.borrowed_amount_wads {
            self.borrows.remove(liquidity_index);
        } else {
            liquidity.repay(settle_amount)?;
        }
        Ok(())
    }

    /// Menarik kolateral (Withdraw) dan menghapusnya dari list jika kosong
    pub fn withdraw(&mut self, withdraw_amount: U256, collateral_index: usize) -> Result<(), LendingError> {
        if collateral_index >= self.deposits.len() {
            return Err(LendingError::InvalidObligationCollateral);
        }

        let collateral = &mut self.deposits[collateral_index];
        if withdraw_amount == collateral.deposited_amount {
            self.deposits.remove(collateral_index);
        } else {
            collateral.withdraw(withdraw_amount)?;
        }
        Ok(())
    }

    /// Menghitung nilai maksimal kolateral yang boleh ditarik
    pub fn max_withdraw_value(&self, withdraw_collateral_ltv: Rate) -> Result<Decimal, LendingError> {
        if self.allowed_borrow_value <= self.borrowed_value {
            return Ok(Decimal::zero());
        }
        if withdraw_collateral_ltv == Rate::zero() {
            return Ok(self.deposited_value);
        }
        
        // Konversi Rate ke Decimal secara eksplisit
        let ltv_decimal: Decimal = withdraw_collateral_ltv.into();
        
        // (Allowed - Borrowed) / LTV Collateral
        self.allowed_borrow_value
            .try_sub(self.borrowed_value)?
            .try_div(ltv_decimal)
    }

    /// Menghitung sisa limit pinjaman
    pub fn remaining_borrow_value(&self) -> Result<Decimal, LendingError> {
        self.allowed_borrow_value.try_sub(self.borrowed_value)
    }

    /// Mencari atau Menambahkan Collateral baru ke dalam List
    pub fn find_or_add_collateral_to_deposits(
        &mut self,
        deposit_reserve: Address,
    ) -> Result<&mut ObligationCollateral, LendingError> {
        // Cari index jika ada
        let index_opt = self.deposits.iter().position(|c| c.deposit_reserve == deposit_reserve);

        if let Some(index) = index_opt {
            return Ok(&mut self.deposits[index]);
        }

        // Jika tidak ada, cek limit
        if self.deposits.len() + self.borrows.len() >= MAX_OBLIGATION_RESERVES {
            return Err(LendingError::ObligationReserveLimit);
        }

        // Tambahkan baru
        self.deposits.push(ObligationCollateral::new(deposit_reserve));
        Ok(self.deposits.last_mut().unwrap())
    }

    /// Mencari Collateral (Read/Check)
    pub fn find_collateral_in_deposits(
        &self,
        deposit_reserve: Address,
    ) -> Result<(&ObligationCollateral, usize), LendingError> {
        if self.deposits.is_empty() {
            return Err(LendingError::ObligationDepositsEmpty);
        }
        let index = self.deposits
            .iter()
            .position(|c| c.deposit_reserve == deposit_reserve)
            .ok_or(LendingError::InvalidObligationCollateral)?;
            
        Ok((&self.deposits[index], index))
    }

    /// Mencari atau Menambahkan Liquidity (Hutang) baru ke dalam List
    pub fn find_or_add_liquidity_to_borrows(
        &mut self,
        borrow_reserve: Address,
    ) -> Result<&mut ObligationLiquidity, LendingError> {
        let index_opt = self.borrows.iter().position(|l| l.borrow_reserve == borrow_reserve);

        if let Some(index) = index_opt {
            return Ok(&mut self.borrows[index]);
        }

        if self.deposits.len() + self.borrows.len() >= MAX_OBLIGATION_RESERVES {
            return Err(LendingError::ObligationReserveLimit);
        }

        self.borrows.push(ObligationLiquidity::new(borrow_reserve));
        Ok(self.borrows.last_mut().unwrap())
    }

    /// Mencari Liquidity (Hutang)
    pub fn find_liquidity_in_borrows(
        &self,
        borrow_reserve: Address,
    ) -> Result<(&ObligationLiquidity, usize), LendingError> {
        if self.borrows.is_empty() {
            return Err(LendingError::ObligationBorrowsEmpty);
        }
        let index = self.borrows
            .iter()
            .position(|l| l.borrow_reserve == borrow_reserve)
            .ok_or(LendingError::InvalidObligationLiquidity)?;
            
        Ok((&self.borrows[index], index))
    }
}

// --- Obligation Collateral (Deposit) ---

#[derive(Debug, Clone, PartialEq)]
pub struct ObligationCollateral {
    pub deposit_reserve: Address,
    pub deposited_amount: U256,
    pub market_value: Decimal,
}

impl ObligationCollateral {
    pub fn new(deposit_reserve: Address) -> Self {
        Self {
            deposit_reserve,
            deposited_amount: U256::zero(),
            market_value: Decimal::zero(),
        }
    }

    pub fn deposit(&mut self, amount: U256) -> Result<(), LendingError> {
        self.deposited_amount = self.deposited_amount
            .checked_add(amount)
            .ok_or(LendingError::MathOverflow)?;
        Ok(())
    }

    pub fn withdraw(&mut self, amount: U256) -> Result<(), LendingError> {
        if amount > self.deposited_amount {
            return Err(LendingError::InvalidObligationCollateral); // Gunakan error yang sudah ada
        }
        self.deposited_amount = self.deposited_amount
            .checked_sub(amount)
            .ok_or(LendingError::MathOverflow)?;
        Ok(())
    }
}

// --- Obligation Liquidity (Borrow) ---

#[derive(Debug, Clone, PartialEq)]
pub struct ObligationLiquidity {
    pub borrow_reserve: Address,
    pub cumulative_borrow_rate_wads: Decimal,
    pub borrowed_amount_wads: Decimal,
    pub market_value: Decimal,
}

impl ObligationLiquidity {
    pub fn new(borrow_reserve: Address) -> Self {
        Self {
            borrow_reserve,
            cumulative_borrow_rate_wads: Decimal::one(),
            borrowed_amount_wads: Decimal::zero(),
            market_value: Decimal::zero(),
        }
    }

    pub fn repay(&mut self, settle_amount: Decimal) -> Result<(), LendingError> {
        if settle_amount > self.borrowed_amount_wads {
            return Err(LendingError::MathOverflow);
        }
        self.borrowed_amount_wads = self.borrowed_amount_wads.try_sub(settle_amount)?;
        Ok(())
    }

    pub fn borrow(&mut self, borrow_amount: Decimal) -> Result<(), LendingError> {
        self.borrowed_amount_wads = self.borrowed_amount_wads.try_add(borrow_amount)?;
        Ok(())
    }

    pub fn accrue_interest(&mut self, cumulative_borrow_rate_wads: Decimal) -> Result<(), LendingError> {
        if cumulative_borrow_rate_wads < self.cumulative_borrow_rate_wads {
            return Err(LendingError::NegativeInterestRate);
        }

        let compounded_interest_rate: Decimal = cumulative_borrow_rate_wads
            .try_div(self.cumulative_borrow_rate_wads)?;

        self.borrowed_amount_wads = self.borrowed_amount_wads
            .try_mul(compounded_interest_rate)?;
            
        self.cumulative_borrow_rate_wads = cumulative_borrow_rate_wads;
        Ok(())
    }
}