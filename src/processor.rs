//! Program state processor - Migrated to Odra
//! Original Solana lending program migrated to Odra/Casper

use odra::prelude::*;
use odra::casper_types::U256;
use odra::casper_types::{U256, CLTyped};
use odra::prelude::BTreeMap;
use odra::schema::{CustomType, NamedCLTyped, SchemaCustomTypes};
use odra::macros::{FromBytes, ToBytes, OdraSchema, CLTyped};
use crate::math::{TryAdd, TrySub, TryMul, TryDiv};

use crate::error::LendingError;
use crate::math::{
    common::{TryAdd, TryDiv, TryMul, TrySub},
    Decimal, Rate
};

#[odra::module]
pub struct NovaLending {
    // Lending Market State
    pub owner: Var<Address>,
    pub quote_currency: Var<[u8; 32]>,
    pub token_program_id: Var<Address>,
    pub oracle_program_id: Var<Address>,
    
    // Reserves and Obligations storage
    pub reserves: Mapping<Address, Reserve>,
    pub obligations: Mapping<Address, Obligation>,
    
    // Additional state variables
    pub bump_seed: Var<u8>,
    pub last_update_slot: Var<u64>,
    pub reserve_count: Var<u64>,
}

#[odra::module]
impl NovaLending {
    // ===========================================================================
    // CONSTRUCTOR - Init Lending Market
    // ===========================================================================
    #[odra(init)]
    pub fn init(
        &mut self,
        owner: Address,
        quote_currency: [u8; 32],
        token_program_id: Address,
        oracle_program_id: Address
    ) {
        self.owner.set(owner);
        self.quote_currency.set(quote_currency);
        self.token_program_id.set(token_program_id);
        self.oracle_program_id.set(oracle_program_id);
        
        // Generate bump seed (simplified PDA equivalent)
        let bump = self.generate_bump_seed();
        self.bump_seed.set(bump);
        self.last_update_slot.set(0u64);
        self.reserve_count.set(0u64);
    }

    // ===========================================================================
    // LENDING MARKET MANAGEMENT
    // ===========================================================================
    
    pub fn set_lending_market_owner(&mut self, new_owner: Address) -> Result<(), LendingError> {
        let caller = self.env().caller();
        let current_owner = self.owner.get().unwrap();
        
        if caller != current_owner {
            return Err(LendingError::InvalidMarketOwner);
        }
        
        self.owner.set(new_owner);
        Ok(())
    }

    // ===========================================================================
    // RESERVE OPERATIONS
    // ===========================================================================
    
    pub fn init_reserve(
        &mut self,
        liquidity_amount: U256,
        config: ReserveConfig
    ) -> Result<(), LendingError> {
        if liquidity_amount == U256::zero() {
            return Err(LendingError::InvalidAmount);
        }

        config.validate()?;

        let caller = self.env().caller();
        let current_owner = self.owner.get().unwrap();
        
        if caller != current_owner {
            return Err(LendingError::InvalidMarketOwner);
        }

        let clock = self.env().get_block_time();
        let market_price = self.get_oracle_price()?;

        let reserve = Reserve::new(InitReserveParams {
            current_slot: clock,
            lending_market: self.env().self_address(),
            liquidity: ReserveLiquidity::new(NewReserveLiquidityParams {
                mint_pubkey: self.generate_temp_address(),
                mint_decimals: 9,
                supply_pubkey: self.generate_temp_address(),
                fee_receiver: caller,
                oracle_pubkey: self.oracle_program_id.get().unwrap(),
                market_price,
                available_amount: liquidity_amount,
                borrowed_amount_wads: Decimal::zero(),
                cumulative_borrow_rate_wads: Decimal::one(),
            }),
            collateral: ReserveCollateral::new(NewReserveCollateralParams {
                mint_pubkey: self.generate_temp_address(),
                supply_pubkey: self.generate_temp_address(),
                mint_total_supply: U256::zero(),
            }),
            config,
        });

        let reserve_key = self.generate_reserve_key();
        self.reserves.set(&reserve_key, reserve);
        
        // Increment reserve count
        let count = self.reserve_count.get().unwrap_or(0);
        self.reserve_count.set(count + 1);

        Ok(())
    }

    pub fn refresh_reserve(&mut self, reserve_key: Address) -> Result<(), LendingError> {
        let mut reserve = self.reserves.get(&reserve_key)
            .ok_or(LendingError::InvalidAccountInput)?;
        
        let clock = self.env().get_block_time();
        reserve.liquidity.market_price = self.get_oracle_price()?;
        
        reserve.accrue_interest(clock)?;
        reserve.last_update.update_slot(clock);
        
        self.reserves.set(&reserve_key, reserve);
        Ok(())
    }

    pub fn deposit_reserve_liquidity(
        &mut self,
        reserve_key: Address,
        liquidity_amount: U256
    ) -> Result<U256, LendingError> {
        if liquidity_amount == U256::zero() {
            return Err(LendingError::InvalidAmount);
        }

        let mut reserve = self.reserves.get(&reserve_key)
            .ok_or(LendingError::InvalidAccountInput)?;
        
        let clock = self.env().get_block_time();
        if reserve.last_update.is_stale(clock) {
            return Err(LendingError::ReserveStale);
        }

        let collateral_amount = reserve.deposit_liquidity(liquidity_amount)?;
        reserve.last_update.mark_stale();
        
        self.reserves.set(&reserve_key, reserve);
        
        // In Odra, token transfers happen through CEP-18
        self.transfer_tokens(liquidity_amount)?;
        
        Ok(collateral_amount)
    }

    pub fn redeem_reserve_collateral(
        &mut self,
        reserve_key: Address,
        collateral_amount: U256
    ) -> Result<U256, LendingError> {
        if collateral_amount == U256::zero() {
            return Err(LendingError::InvalidAmount);
        }

        let mut reserve = self.reserves.get(&reserve_key)
            .ok_or(LendingError::InvalidAccountInput)?;
        
        let clock = self.env().get_block_time();
        if reserve.last_update.is_stale(clock) {
            return Err(LendingError::ReserveStale);
        }

        let liquidity_amount = reserve.redeem_collateral(collateral_amount)?;
        reserve.last_update.mark_stale();
        
        self.reserves.set(&reserve_key, reserve);
        
        // Transfer tokens back to user
        self.transfer_tokens_to_user(liquidity_amount)?;
        
        Ok(liquidity_amount)
    }

    // ===========================================================================
    // OBLIGATION OPERATIONS
    // ===========================================================================
    
    pub fn init_obligation(&mut self) -> Result<(), LendingError> {
        let caller = self.env().caller();
        
        if self.obligations.get(&caller).is_some() {
            return Err(LendingError::AlreadyInitialized);
        }

        let clock = self.env().get_block_time();
        let obligation = Obligation::new(InitObligationParams {
            current_slot: clock,
            lending_market: self.env().self_address(),
            owner: caller,
            deposits: vec![],
            borrows: vec![],
        });

        self.obligations.set(&caller, obligation);
        Ok(())
    }

    pub fn refresh_obligation(&mut self, user_address: Address) -> Result<(), LendingError> {
        let mut obligation = self.obligations.get(&user_address)
            .ok_or(LendingError::InvalidObligationOwner)?;

        let clock = self.env().get_block_time();
        
        let mut deposited_value = Decimal::zero();
        let mut borrowed_value = Decimal::zero();
        let mut allowed_borrow_value = Decimal::zero();
        let mut unhealthy_borrow_value = Decimal::zero();

        // Refresh deposits
        for collateral in obligation.deposits.iter_mut() {
            let reserve_key = collateral.deposit_reserve;
            let reserve = self.reserves.get(&reserve_key)
                .ok_or(LendingError::InvalidAccountInput)?;

            if reserve.last_update.is_stale(clock) {
                return Err(LendingError::ReserveStale);
            }

            let market_value = self.calculate_market_value(
                collateral.deposited_amount,
                reserve.liquidity.market_price,
                reserve.liquidity.mint_decimals
            )?;
            
            collateral.market_value = market_value;

            let loan_to_value_rate = Rate::from_percent(reserve.config.loan_to_value_ratio);
            let liquidation_threshold_rate = Rate::from_percent(reserve.config.liquidation_threshold);

            deposited_value = deposited_value.try_add(market_value)?;
            allowed_borrow_value = allowed_borrow_value.try_add(market_value.try_mul(loan_to_value_rate)?)?;
            unhealthy_borrow_value = unhealthy_borrow_value.try_add(market_value.try_mul(liquidation_threshold_rate)?)?;
        }

        // Refresh borrows
        for liquidity in obligation.borrows.iter_mut() {
            let reserve_key = liquidity.borrow_reserve;
            let reserve = self.reserves.get(&reserve_key)
                .ok_or(LendingError::InvalidAccountInput)?;

            if reserve.last_update.is_stale(clock) {
                return Err(LendingError::ReserveStale);
            }

            liquidity.accrue_interest(reserve.liquidity.cumulative_borrow_rate_wads)?;

            let market_value = self.calculate_market_value(
                liquidity.borrowed_amount_wads.try_floor_u64()?,
                reserve.liquidity.market_price,
                reserve.liquidity.mint_decimals
            )?;
            
            liquidity.market_value = market_value;
            borrowed_value = borrowed_value.try_add(market_value)?;
        }

        obligation.deposited_value = deposited_value;
        obligation.borrowed_value = borrowed_value;
        obligation.allowed_borrow_value = allowed_borrow_value;
        obligation.unhealthy_borrow_value = unhealthy_borrow_value;
        obligation.last_update.update_slot(clock);
        
        self.obligations.set(&user_address, obligation);
        Ok(())
    }

    pub fn deposit_obligation_collateral(
        &mut self,
        reserve_key: Address,
        collateral_amount: U256
    ) -> Result<(), LendingError> {
        if collateral_amount == U256::zero() {
            return Err(LendingError::InvalidAmount);
        }

        let caller = self.env().caller();
        let mut obligation = self.obligations.get(&caller)
            .ok_or(LendingError::InvalidObligationOwner)?;
            
        let mut reserve = self.reserves.get(&reserve_key)
            .ok_or(LendingError::InvalidAccountInput)?;

        let clock = self.env().get_block_time();
        if reserve.last_update.is_stale(clock) {
            return Err(LendingError::ReserveStale);
        }

        if reserve.config.loan_to_value_ratio == 0 {
            return Err(LendingError::ReserveCollateralDisabled);
        }

        let collateral = obligation.find_or_add_collateral_to_deposits(reserve_key)?;
        collateral.deposit(collateral_amount)?;
            
        obligation.last_update.mark_stale();
        
        self.obligations.set(&caller, obligation);
        self.reserves.set(&reserve_key, reserve);
        
        self.transfer_tokens(collateral_amount)?;
        
        Ok(())
    }

    pub fn withdraw_obligation_collateral(
        &mut self,
        reserve_key: Address,
        collateral_amount: U256
    ) -> Result<(), LendingError> {
        if collateral_amount == U256::zero() {
            return Err(LendingError::InvalidAmount);
        }

        let caller = self.env().caller();
        let mut obligation = self.obligations.get(&caller)
            .ok_or(LendingError::InvalidObligationOwner)?;
            
        let reserve = self.reserves.get(&reserve_key)
            .ok_or(LendingError::InvalidAccountInput)?;

        let clock = self.env().get_block_time();
        if reserve.last_update.is_stale(clock) || obligation.last_update.is_stale(clock) {
            return Err(LendingError::ReserveStale);
        }

        let (collateral, collateral_index) = obligation.find_collateral_in_deposits(reserve_key)?;
        if collateral.deposited_amount == U256::zero() {
            return Err(LendingError::ObligationCollateralEmpty);
        }

        let withdraw_amount = if obligation.borrows.is_empty() {
            if collateral_amount == U256::max_value() {
                collateral.deposited_amount
            } else {
                collateral.deposited_amount.min(collateral_amount)
            }
        } else {
            // Complex withdrawal logic with borrows
            self.calculate_withdraw_amount(&obligation, &reserve, &collateral, collateral_amount)?
        };

        obligation.withdraw(withdraw_amount, collateral_index)?;
        obligation.last_update.mark_stale();
        
        self.obligations.set(&caller, obligation);
        self.transfer_tokens_to_user(withdraw_amount)?;
        
        Ok(())
    }

    // ===========================================================================
    // BORROW AND REPAY OPERATIONS
    // ===========================================================================
    
    pub fn borrow_obligation_liquidity(
        &mut self,
        reserve_key: Address,
        liquidity_amount: U256,
        slippage_limit: U256
    ) -> Result<(), LendingError> {
        if liquidity_amount == U256::zero() {
            return Err(LendingError::InvalidAmount);
        }

        let caller = self.env().caller();
        let mut obligation = self.obligations.get(&caller)
            .ok_or(LendingError::InvalidObligationOwner)?;
            
        let mut reserve = self.reserves.get(&reserve_key)
            .ok_or(LendingError::InvalidAccountInput)?;

        let clock = self.env().get_block_time();
        if reserve.last_update.is_stale(clock) || obligation.last_update.is_stale(clock) {
            return Err(LendingError::ReserveStale);
        }

        if obligation.deposits.is_empty() {
            return Err(LendingError::ObligationDepositsEmpty);
        }

        let remaining_borrow_value = obligation.remaining_borrow_value()?;
        if remaining_borrow_value == Decimal::zero() {
            return Err(LendingError::BorrowTooLarge);
        }

        let CalculateBorrowResult {
            borrow_amount,
            receive_amount,
            borrow_fee: _,
            host_fee: _,
        } = reserve.calculate_borrow(liquidity_amount, remaining_borrow_value)?;

        if receive_amount == U256::zero() {
            return Err(LendingError::BorrowTooSmall);
        }

        if liquidity_amount == U256::max_value() && receive_amount < slippage_limit {
            return Err(LendingError::ExceededSlippage);
        }

        reserve.liquidity.borrow(borrow_amount)?;
        reserve.last_update.mark_stale();
        
        let liquidity = obligation.find_or_add_liquidity_to_borrows(reserve_key)?;
        liquidity.borrow(borrow_amount.try_floor_u64()?)?;
        obligation.last_update.mark_stale();
        
        self.reserves.set(&reserve_key, reserve);
        self.obligations.set(&caller, obligation);
        
        // Distribute borrowed amount minus fees
        self.transfer_tokens_to_user(receive_amount)?;
        
        Ok(())
    }

    pub fn repay_obligation_liquidity(
        &mut self,
        reserve_key: Address,
        liquidity_amount: U256
    ) -> Result<(), LendingError> {
        if liquidity_amount == U256::zero() {
            return Err(LendingError::InvalidAmount);
        }

        let caller = self.env().caller();
        let mut obligation = self.obligations.get(&caller)
            .ok_or(LendingError::InvalidObligationOwner)?;
            
        let mut reserve = self.reserves.get(&reserve_key)
            .ok_or(LendingError::InvalidAccountInput)?;

        let clock = self.env().get_block_time();
        if reserve.last_update.is_stale(clock) || obligation.last_update.is_stale(clock) {
            return Err(LendingError::ReserveStale);
        }

        let (liquidity, liquidity_index) = obligation.find_liquidity_in_borrows(reserve_key)?;
        if liquidity.borrowed_amount_wads == Decimal::zero() {
            return Err(LendingError::ObligationLiquidityEmpty);
        }

        let CalculateRepayResult {
            settle_amount,
            repay_amount,
        } = reserve.calculate_repay(liquidity_amount, liquidity.borrowed_amount_wads)?;

        if repay_amount == U256::zero() {
            return Err(LendingError::RepayTooSmall);
        }

        reserve.liquidity.repay(repay_amount, settle_amount)?;
        reserve.last_update.mark_stale();
        
        obligation.repay(settle_amount, liquidity_index)?;
        obligation.last_update.mark_stale();
        
        self.reserves.set(&reserve_key, reserve);
        self.obligations.set(&caller, obligation);
        
        self.transfer_tokens(repay_amount)?;
        
        Ok(())
    }

    // ===========================================================================
    // LIQUIDATION OPERATIONS
    // ===========================================================================
    
    pub fn liquidate_obligation(
        &mut self,
        borrower: Address,
        repay_reserve_key: Address,
        withdraw_reserve_key: Address,
        liquidity_amount: U256
    ) -> Result<(), LendingError> {
        if liquidity_amount == U256::zero() {
            return Err(LendingError::InvalidAmount);
        }

        let mut obligation = self.obligations.get(&borrower)
            .ok_or(LendingError::InvalidObligationOwner)?;
            
        let mut repay_reserve = self.reserves.get(&repay_reserve_key)
            .ok_or(LendingError::InvalidAccountInput)?;
        let mut withdraw_reserve = self.reserves.get(&withdraw_reserve_key)
            .ok_or(LendingError::InvalidAccountInput)?;

        let clock = self.env().get_block_time();
        if repay_reserve.last_update.is_stale(clock) || 
           withdraw_reserve.last_update.is_stale(clock) || 
           obligation.last_update.is_stale(clock) {
            return Err(LendingError::ReserveStale);
        }

        if obligation.borrowed_value < obligation.unhealthy_borrow_value {
            return Err(LendingError::ObligationHealthy);
        }

        let (liquidity, liquidity_index) = obligation.find_liquidity_in_borrows(repay_reserve_key)?;
        let (collateral, collateral_index) = obligation.find_collateral_in_deposits(withdraw_reserve_key)?;

        let CalculateLiquidationResult {
            settle_amount,
            repay_amount,
            withdraw_amount,
        } = withdraw_reserve.calculate_liquidation(
            liquidity_amount,
            &obligation,
            &liquidity,
            &collateral,
        )?;

        if repay_amount == U256::zero() || withdraw_amount == U256::zero() {
            return Err(LendingError::LiquidationTooSmall);
        }

        repay_reserve.liquidity.repay(repay_amount, settle_amount)?;
        repay_reserve.last_update.mark_stale();
        
        obligation.repay(settle_amount, liquidity_index)?;
        obligation.withdraw(withdraw_amount, collateral_index)?;
        obligation.last_update.mark_stale();
        
        self.reserves.set(&repay_reserve_key, repay_reserve);
        self.reserves.set(&withdraw_reserve_key, withdraw_reserve);
        self.obligations.set(&borrower, obligation);
        
        // Transfer logic for liquidation
        self.handle_liquidation_transfers(repay_amount, withdraw_amount)?;
        
        Ok(())
    }

    // ===========================================================================
    // FLASH LOAN OPERATIONS
    // ===========================================================================
    
    pub fn flash_loan(
        &mut self,
        reserve_key: Address,
        amount: U256
    ) -> Result<(), LendingError> {
        if amount == U256::zero() {
            return Err(LendingError::InvalidAmount);
        }

        let mut reserve = self.reserves.get(&reserve_key)
            .ok_or(LendingError::InvalidAccountInput)?;

        let flash_loan_amount = if amount == U256::max_value() {
            reserve.liquidity.available_amount
        } else {
            amount
        };

        let (origination_fee, host_fee) = reserve.config.fees
            .calculate_flash_loan_fees(Decimal::from(flash_loan_amount.as_u128()))?;

        let returned_amount_required = flash_loan_amount
            .checked_add(origination_fee.try_floor_u64()?)
            .ok_or(LendingError::MathOverflow)?;

        reserve.liquidity.borrow(Decimal::from(flash_loan_amount.as_u128()))?;
        self.reserves.set(&reserve_key, reserve);
        
        // Execute flash loan logic
        self.execute_flash_loan(flash_loan_amount, returned_amount_required)?;
        
        let mut reserve = self.reserves.get(&reserve_key).unwrap();
        reserve.liquidity.repay(flash_loan_amount, Decimal::from(flash_loan_amount.as_u128()))?;
        self.reserves.set(&reserve_key, reserve);
        
        // Handle fees
        self.distribute_flash_loan_fees(origination_fee.try_floor_u64()?, host_fee.try_floor_u64()?)?;
        
        Ok(())
    }

    // ===========================================================================
    // CONFIGURATION OPERATIONS
    // ===========================================================================
    
    pub fn modify_reserve_config(
        &mut self,
        reserve_key: Address,
        new_config: ReserveConfig
    ) -> Result<(), LendingError> {
        new_config.validate()?;

        let caller = self.env().caller();
        let current_owner = self.owner.get().unwrap();
        
        if caller != current_owner {
            return Err(LendingError::InvalidMarketOwner);
        }

        let mut reserve = self.reserves.get(&reserve_key)
            .ok_or(LendingError::InvalidAccountInput)?;

        // Validate reserve belongs to this lending market
        if reserve.lending_market != self.env().self_address() {
            return Err(LendingError::InvalidAccountInput);
        }

        reserve.config = new_config;
        self.reserves.set(&reserve_key, reserve);
        
        Ok(())
    }

    // ===========================================================================
    // HELPER FUNCTIONS
    // ===========================================================================
    
    fn generate_bump_seed(&self) -> u8 {
        // Simplified bump seed generation for Odra
        255
    }
    
    fn generate_reserve_key(&self) -> Address {
        // Generate a unique key for each reserve
        let count = self.reserve_count.get().unwrap_or(0);
        let mut data = self.env().self_address().to_bytes().unwrap().to_vec();
        data.extend_from_slice(&count.to_le_bytes());

        // Use hash to create deterministic address
        let hash = self.env().hash(&data);
        Address::from_bytes(&hash).unwrap()
    }

    fn generate_temp_address(&self) -> Address {
        // Generate temporary address for mock data
        let mut data = self.env().self_address().to_bytes().unwrap().to_vec();
        data.extend_from_slice(&self.env().get_block_time().to_le_bytes());

        let hash = self.env().hash(&data);
        Address::from_bytes(&hash).unwrap()
    }
    
    fn get_oracle_price(&self) -> Result<Decimal, LendingError> {
        // Simplified oracle price fetch
        // In production, you would call an oracle contract
        Ok(Decimal::from(1_000_000_000u64)) // Mock price
    }
    
    fn transfer_tokens(&self, _amount: U256) -> Result<(), LendingError> {
        // Simplified token transfer - in production use CEP-18
        Ok(())
    }
    
    fn transfer_tokens_to_user(&self, _amount: U256) -> Result<(), LendingError> {
        // Simplified token transfer to user - in production use CEP-18
        Ok(())
    }
    
    fn calculate_market_value(
        &self, 
        amount: U256, 
        price: Decimal, 
        decimals: u8
    ) -> Result<Decimal, LendingError> {
        let decimals_factor = 10u64
            .checked_pow(decimals as u32)
            .ok_or(LendingError::MathOverflow)?;
            
        let amount_decimal = Decimal::from(amount.as_u128());
        amount_decimal
            .try_mul(price)?
            .try_div(Decimal::from(decimals_factor))
    }
    
    fn calculate_withdraw_amount(
        &self,
        obligation: &Obligation,
        reserve: &Reserve,
        collateral: &Collateral,
        collateral_amount: U256
    ) -> Result<U256, LendingError> {
        // Complex withdrawal calculation when user has borrows
        if obligation.deposited_value == Decimal::zero() {
            return Err(LendingError::ObligationDepositsZero);
        }

        let max_withdraw_value = obligation.max_withdraw_value(Rate::from_percent(
            reserve.config.loan_to_value_ratio
        ))?;

        if max_withdraw_value == Decimal::zero() {
            return Err(LendingError::WithdrawTooLarge);
        }

        let withdraw_amount = if collateral_amount == U256::max_value() {
            let withdraw_value = max_withdraw_value.min(collateral.market_value);
            let withdraw_pct = withdraw_value.try_div(collateral.market_value)?;
            (withdraw_pct
                .try_mul(Decimal::from(collateral.deposited_amount.as_u128()))?
                .try_floor_u64()?)
            .min(collateral.deposited_amount.as_u64())
            .into()
        } else {
            let withdraw_amount = collateral_amount.min(collateral.deposited_amount);
            let withdraw_pct = Decimal::from(withdraw_amount.as_u128())
                .try_div(Decimal::from(collateral.deposited_amount.as_u128()))?;
            let withdraw_value = collateral.market_value.try_mul(withdraw_pct)?;
            if withdraw_value > max_withdraw_value {
                return Err(LendingError::WithdrawTooLarge);
            }
            withdraw_amount
        };

        if withdraw_amount == U256::zero() {
            return Err(LendingError::WithdrawTooSmall);
        }

        Ok(withdraw_amount)
    }
    
    fn handle_liquidation_transfers(
        &self,
        _repay_amount: U256,
        _withdraw_amount: U256
    ) -> Result<(), LendingError> {
        // Handle token transfers for liquidation
        Ok(())
    }
    
    fn execute_flash_loan(
        &self,
        _loan_amount: U256,
        _required_repayment: U256
    ) -> Result<(), LendingError> {
        // Execute flash loan callback
        Ok(())
    }
    
    fn distribute_flash_loan_fees(
        &self,
        _origination_fee: U256,
        _host_fee: U256
    ) -> Result<(), LendingError> {
        // Distribute flash loan fees
        Ok(())
    }

    // ===========================================================================
    // QUERY FUNCTIONS
    // ===========================================================================
    
    pub fn get_reserve(&self, reserve_key: Address) -> Option<Reserve> {
        self.reserves.get(&reserve_key)
    }
    
    pub fn get_obligation(&self, user_address: Address) -> Option<Obligation> {
        self.obligations.get(&user_address)
    }
    
    pub fn get_owner(&self) -> Option<Address> {
        self.owner.get()
    }
    
    pub fn get_reserve_count(&self) -> u64 {
        self.reserve_count.get().unwrap_or(0)
    }
}

// ===========================================================================
// SUPPORTING STRUCTS AND IMPLEMENTATIONS
// ===========================================================================

#[derive(OdraSchema, Debug, Clone, ToBytes, FromBytes, CLTyped)]
pub struct Reserve {
    pub lending_market: Address,
    pub liquidity: ReserveLiquidity,
    pub collateral: ReserveCollateral,
    pub config: ReserveConfig,
    pub last_update: LastUpdate,
}

impl Reserve {
    pub fn new(params: InitReserveParams) -> Self {
        Self {
            lending_market: params.lending_market,
            liquidity: params.liquidity,
            collateral: params.collateral,
            config: params.config,
            last_update: LastUpdate {
                slot: params.current_slot,
                stale: false,
            },
        }
    }
    
    pub fn deposit_liquidity(&mut self, amount: U256) -> Result<U256, LendingError> {
        let exchange_rate = self.collateral_exchange_rate()?;
        let collateral_amount = exchange_rate
            .try_mul(Decimal::from(amount.as_u128()))?
            .try_floor_u64()?;

        self.liquidity.deposit(amount)?;
        self.collateral.mint(collateral_amount.into())?;

        Ok(collateral_amount.into())
    }
    
    pub fn redeem_collateral(&mut self, amount: U256) -> Result<U256, LendingError> {
        let exchange_rate = self.collateral_exchange_rate()?;
        let liquidity_amount = Decimal::from(amount.as_u128())
            .try_div(exchange_rate)?
            .try_floor_u64()?;

        if liquidity_amount > self.liquidity.available_amount.as_u64() {
            return Err(LendingError::InsufficientLiquidity);
        }

        self.collateral.burn(amount)?;
        self.liquidity.withdraw(liquidity_amount.into())?;

        Ok(liquidity_amount.into())
    }
    
    pub fn accrue_interest(&mut self, _slot: u64) -> Result<(), LendingError> {
        // Simplified interest accrual
        // In production, implement compound interest calculation
        Ok(())
    }
    
    pub fn calculate_borrow(
        &self,
        amount: U256,
        remaining: Decimal
    ) -> Result<CalculateBorrowResult, LendingError> {
        let remaining_u64 = remaining.try_floor_u64()?;
        let borrow_amount: U256 = if amount == U256::max_value() {
            remaining_u64.into()
        } else {
            U256::min(amount, remaining_u64.into())
        };

        let borrow_fee = borrow_amount / 100; // 1% borrow fee
        let host_fee = borrow_fee / 10; // 10% of borrow fee to host
        let receive_amount = borrow_amount - borrow_fee;

        Ok(CalculateBorrowResult {
            borrow_amount: Decimal::from(borrow_amount.as_u128()),
            receive_amount,
            borrow_fee,
            host_fee
        })
    }
    
    pub fn calculate_repay(
        &self,
        amount: U256,
        borrowed: Decimal
    ) -> Result<CalculateRepayResult, LendingError> {
        let borrowed_u256 = borrowed.try_floor_u64()?;
        let repay_amount: U256 = if amount == U256::max_value() {
            borrowed_u256.into()
        } else {
            U256::min(amount, borrowed_u256.into())
        };

        let settle_amount = Decimal::from(repay_amount.as_u128());

        Ok(CalculateRepayResult {
            settle_amount,
            repay_amount
        })
    }
    
    pub fn calculate_liquidation(
        &self,
        amount: U256,
        obligation: &Obligation,
        liquidity: &Liquidity,
        collateral: &Collateral,
    ) -> Result<CalculateLiquidationResult, LendingError> {
        // Simplified liquidation calculation
        let max_repay = obligation.borrowed_value.try_sub(obligation.unhealthy_borrow_value)?;
        let repay_value = Decimal::from(amount.as_u128()).min(max_repay);
        
        let liquidation_premium = Rate::from_percent(105); // 5% liquidation premium
        let withdraw_value = repay_value.try_mul(liquidation_premium)?;
        
        let repay_amount = repay_value.try_floor_u64()?;
        let withdraw_amount = withdraw_value.try_div(collateral.market_value)?.try_floor_u64()?;

        Ok(CalculateLiquidationResult {
            settle_amount: repay_value,
            repay_amount: repay_amount.into(),
            withdraw_amount: withdraw_amount.into()
        })
    }
    
    fn collateral_exchange_rate(&self) -> Result<Decimal, LendingError> {
        if self.collateral.mint_total_supply.is_zero() {
            return Ok(Decimal::one());
        }
        
        Decimal::from(self.liquidity.total_supply().as_u128())
            .try_div(Decimal::from(self.collateral.mint_total_supply.as_u128()))
    }
}

#[derive(OdraSchema, Debug, Clone, ToBytes, FromBytes, CLTyped)]
pub struct Obligation {
    pub lending_market: Address,
    pub owner: Address,
    pub deposits: Vec<Collateral>,
    pub borrows: Vec<Liquidity>,
    pub deposited_value: Decimal,
    pub borrowed_value: Decimal,
    pub allowed_borrow_value: Decimal,
    pub unhealthy_borrow_value: Decimal,
    pub last_update: LastUpdate,
}

impl Obligation {
    pub fn new(params: InitObligationParams) -> Self {
        Self {
            lending_market: params.lending_market,
            owner: params.owner,
            deposits: params.deposits,
            borrows: params.borrows,
            deposited_value: Decimal::zero(),
            borrowed_value: Decimal::zero(),
            allowed_borrow_value: Decimal::zero(),
            unhealthy_borrow_value: Decimal::zero(),
            last_update: LastUpdate {
                slot: params.current_slot,
                stale: false,
            },
        }
    }
    
    pub fn find_or_add_collateral_to_deposits(
        &mut self,
        reserve: Address
    ) -> Result<&mut Collateral, LendingError> {
        let has_collateral = self.deposits
            .iter_mut()
            .any(|c| c.deposit_reserve == reserve);

        if !has_collateral {
            self.deposits.push(Collateral {
                deposit_reserve: reserve,
                deposited_amount: U256::zero(),
                market_value: Decimal::zero()
            });
        }

        Ok(self.deposits
            .iter_mut()
            .find(|c| c.deposit_reserve == reserve)
            .unwrap())
    }
    
    pub fn find_collateral_in_deposits(&self, reserve: Address) -> Result<(Collateral, usize), LendingError> {
        for (index, collateral) in self.deposits.iter().enumerate() {
            if collateral.deposit_reserve == reserve {
                return Ok((collateral.clone(), index));
            }
        }
        Err(LendingError::ObligationCollateralEmpty)
    }
    
    pub fn find_or_add_liquidity_to_borrows(
        &mut self,
        reserve: Address
    ) -> Result<&mut Liquidity, LendingError> {
        let has_liquidity = self.borrows.iter().any(|l| l.borrow_reserve == reserve);

        if !has_liquidity {
            // Add new liquidity
            self.borrows.push(Liquidity {
                borrow_reserve: reserve,
                borrowed_amount_wads: Decimal::zero(),
                market_value: Decimal::zero(),
                cumulative_borrow_rate_wads: Decimal::one()
            });
        }

        Ok(self.borrows
            .iter_mut()
            .find(|l| l.borrow_reserve == reserve)
            .unwrap())
    }
    
    pub fn find_liquidity_in_borrows(&self, reserve: Address) -> Result<(Liquidity, usize), LendingError> {
        for (index, liquidity) in self.borrows.iter().enumerate() {
            if liquidity.borrow_reserve == reserve {
                return Ok((liquidity.clone(), index));
            }
        }
        Err(LendingError::ObligationLiquidityEmpty)
    }
    
    pub fn withdraw(&mut self, amount: U256, index: usize) -> Result<(), LendingError> {
        if index >= self.deposits.len() {
            return Err(LendingError::InvalidAccountInput);
        }
        
        if amount > self.deposits[index].deposited_amount {
            return Err(LendingError::WithdrawTooLarge);
        }
        
        self.deposits[index].deposited_amount = self.deposits[index].deposited_amount - amount;
        Ok(())
    }
    
    pub fn repay(&mut self, amount: Decimal, index: usize) -> Result<(), LendingError> {
        if index >= self.borrows.len() {
            return Err(LendingError::InvalidAccountInput);
        }
        
        if amount > self.borrows[index].borrowed_amount_wads {
            return Err(LendingError::RepayTooSmall);
        }
        
        self.borrows[index].borrowed_amount_wads = self.borrows[index].borrowed_amount_wads.try_sub(amount)?;
        Ok(())
    }
    
    pub fn remaining_borrow_value(&self) -> Result<Decimal, LendingError> {
        if self.borrowed_value >= self.allowed_borrow_value {
            return Ok(Decimal::zero());
        }
        self.allowed_borrow_value.try_sub(self.borrowed_value)
    }
    
    pub fn max_withdraw_value(&self, rate: Rate) -> Result<Decimal, LendingError> {
        if self.borrows.is_empty() {
            return Ok(self.deposited_value);
        }
        
        let available_value = self.deposited_value.try_mul(rate)?;
        if available_value <= self.borrowed_value {
            return Ok(Decimal::zero());
        }
        
        available_value.try_sub(self.borrowed_value)
    }
}

#[derive(Debug, Clone, ToBytes, FromBytes, CLTyped)]
pub struct LastUpdate {
    pub slot: u64,
    pub stale: bool,
}

impl LastUpdate {
    pub fn update_slot(&mut self, slot: u64) {
        self.slot = slot;
        self.stale = false;
    }
    
    pub fn mark_stale(&mut self) {
        self.stale = true;
    }
    
    pub fn is_stale(&self, current_slot: u64) -> bool {
        self.stale || self.slot < current_slot
    }
}

#[derive(Debug, Clone, ToBytes, FromBytes, CLTyped)]
pub struct Collateral {
    pub deposit_reserve: Address,
    pub deposited_amount: U256,
    pub market_value: Decimal,
}

impl TryAdd for U256 {
    fn try_add(self, rhs: Self) -> Result<Self, LendingError> {
        self.checked_add(rhs).ok_or(LendingError::MathOverflow)
    }
}

impl TrySub for U256 {
    fn try_sub(self, rhs: Self) -> Result<Self, LendingError> {
        self.checked_sub(rhs).ok_or(LendingError::MathOverflow)
    }
}

impl Collateral {
    pub fn deposit(&mut self, amount: U256) -> Result<(), LendingError> {
        self.deposited_amount = self.deposited_amount.try_add(amount)?;
        Ok(())
    }
}

impl SchemaCustomTypes for LendingError {
    fn schema_custom_types() -> BTreeMap<String, odra::schema::casper_contract_schema::CustomType> {
        BTreeMap::new()
    }
}

#[derive(Debug, Clone, ToBytes, FromBytes, CLTyped)]
pub struct Liquidity {
    pub borrow_reserve: Address,
    pub borrowed_amount_wads: Decimal,
    pub market_value: Decimal,
    pub cumulative_borrow_rate_wads: Decimal,
}

impl Liquidity {
    pub fn borrow(&mut self, amount: U256) -> Result<(), LendingError> {
        let amount_decimal = Decimal::from(amount.as_u128());
        self.borrowed_amount_wads = self.borrowed_amount_wads.try_add(amount_decimal)?;
        Ok(())
    }
    
    pub fn accrue_interest(&mut self, cumulative_borrow_rate: Decimal) -> Result<(), LendingError> {
        let compounded_interest = cumulative_borrow_rate.try_div(self.cumulative_borrow_rate_wads)?;
        self.borrowed_amount_wads = self.borrowed_amount_wads.try_mul(compounded_interest)?;
        self.cumulative_borrow_rate_wads = cumulative_borrow_rate;
        Ok(())
    }
}

// ===========================================================================
// PARAMETER STRUCTS
// ===========================================================================

#[derive(OdraSchema, Debug, Clone, ToBytes, FromBytes, CLTyped)]
pub struct InitReserveParams {
    pub current_slot: u64,
    pub lending_market: Address,
    pub liquidity: ReserveLiquidity,
    pub collateral: ReserveCollateral,
    pub config: ReserveConfig,
}

#[derive(OdraSchema, Debug, Clone, ToBytes, FromBytes, CLTyped)]
pub struct InitObligationParams {
    pub current_slot: u64,
    pub lending_market: Address,
    pub owner: Address,
    pub deposits: Vec<Collateral>,
    pub borrows: Vec<Liquidity>,
}

#[derive(OdraSchema, Debug, Clone, ToBytes, FromBytes, CLTyped)]
pub struct NewReserveLiquidityParams {
    pub mint_pubkey: Address,
    pub mint_decimals: u8,
    pub supply_pubkey: Address,
    pub fee_receiver: Address,
    pub oracle_pubkey: Address,
    pub market_price: Decimal,
    pub available_amount: U256,
    pub borrowed_amount_wads: Decimal,
    pub cumulative_borrow_rate_wads: Decimal,
}

#[derive(OdraSchema, Debug, Clone, ToBytes, FromBytes, CLTyped)]
pub struct NewReserveCollateralParams {
    pub mint_pubkey: Address,
    pub supply_pubkey: Address,
    pub mint_total_supply: U256,
}

#[derive(OdraSchema, Debug, Clone, ToBytes, FromBytes, CLTyped)]
pub struct ReserveConfig {
    pub loan_to_value_ratio: u8,
    pub liquidation_threshold: u8,
    pub liquidation_bonus: u8,
    pub fees: ReserveFees,
}

impl ReserveConfig {
    pub fn validate(&self) -> Result<(), LendingError> {
        if self.loan_to_value_ratio > 100 {
            return Err(LendingError::InvalidConfig);
        }
        if self.liquidation_threshold > 100 {
            return Err(LendingError::InvalidConfig);
        }
        if self.liquidation_bonus > 100 {
            return Err(LendingError::InvalidConfig);
        }
        Ok(())
    }
}

#[derive(OdraSchema, Debug, Clone, ToBytes, FromBytes, CLTyped)]
pub struct ReserveFees {
    pub borrow_fee_wad: U256,
    pub flash_loan_fee_wad: U256,
    pub host_fee_percentage: u8,
}

impl ReserveFees {
    pub fn calculate_flash_loan_fees(&self, amount: Decimal) -> Result<(Decimal, Decimal), LendingError> {
        let fee = amount.try_mul(Decimal::from(self.flash_loan_fee_wad.as_u128()))?;
        let host_fee = fee.try_mul(Decimal::from(self.host_fee_percentage as u64))?;
        let origination_fee = fee.try_sub(host_fee)?;
        Ok((origination_fee, host_fee))
    }
}

#[derive(OdraSchema, Debug, Clone, ToBytes, FromBytes, CLTyped)]
pub struct ReserveLiquidity {
    pub mint_pubkey: Address,
    pub mint_decimals: u8,
    pub supply_pubkey: Address,
    pub fee_receiver: Address,
    pub oracle_pubkey: Address,
    pub market_price: Decimal,
    pub available_amount: U256,
    pub borrowed_amount_wads: Decimal,
    pub cumulative_borrow_rate_wads: Decimal,
}

impl ReserveLiquidity {
    pub fn new(params: NewReserveLiquidityParams) -> Self {
        Self {
            mint_pubkey: params.mint_pubkey,
            mint_decimals: params.mint_decimals,
            supply_pubkey: params.supply_pubkey,
            fee_receiver: params.fee_receiver,
            oracle_pubkey: params.oracle_pubkey,
            market_price: params.market_price,
            available_amount: params.available_amount,
            borrowed_amount_wads: params.borrowed_amount_wads,
            cumulative_borrow_rate_wads: params.cumulative_borrow_rate_wads,
        }
    }

    pub fn deposit(&mut self, amount: U256) -> Result<(), LendingError> {
        self.available_amount = self.available_amount.try_add(amount)?;
        Ok(())
    }

    pub fn withdraw(&mut self, amount: U256) -> Result<(), LendingError> {
        if amount > self.available_amount {
            return Err(LendingError::InsufficientLiquidity);
        }
        self.available_amount = self.available_amount.try_sub(amount)?;
        Ok(())
    }

    pub fn borrow(&mut self, amount: Decimal) -> Result<(), LendingError> {
        let amount_u256: U256 = amount.try_floor_u64()?.into();
        if amount_u256 > self.available_amount {
            return Err(LendingError::InsufficientLiquidity);
        }
        self.available_amount = self.available_amount.try_sub(amount_u256)?;
        self.borrowed_amount_wads = self.borrowed_amount_wads.try_add(amount)?;
        Ok(())
    }

    pub fn repay(
        &mut self,
        repay_amount: U256,
        settle_amount: Decimal
    ) -> Result<(), LendingError> {
        self.available_amount = self.available_amount.try_add(repay_amount)?;
        self.borrowed_amount_wads = self.borrowed_amount_wads.try_sub(settle_amount)?;
        Ok(())
    }

    pub fn total_supply(&self) -> U256 {
        self.available_amount
            .try_add(
                self.borrowed_amount_wads
                    .try_floor_u64()
                    .unwrap_or(0)
                    .into()
            )
            .unwrap_or(self.available_amount)
    }
}

#[derive(OdraSchema, Debug, Clone, ToBytes, FromBytes, CLTyped)]
pub struct ReserveCollateral {
    pub mint_pubkey: Address,
    pub supply_pubkey: Address,
    pub mint_total_supply: U256,
}

impl ReserveCollateral {
    pub fn new(params: NewReserveCollateralParams) -> Self {
        Self {
            mint_pubkey: params.mint_pubkey,
            supply_pubkey: params.supply_pubkey,
            mint_total_supply: params.mint_total_supply,
        }
    }

    pub fn mint(&mut self, amount: U256) -> Result<(), LendingError> {
        self.mint_total_supply = self.mint_total_supply.try_add(amount)?;
        Ok(())
    }

    pub fn burn(&mut self, amount: U256) -> Result<(), LendingError> {
        if amount > self.mint_total_supply {
            return Err(LendingError::InsufficientCollateral);
        }
        self.mint_total_supply = self.mint_total_supply.try_sub(amount)?;
        Ok(())
    }
}

// ===========================================================================
// RESULT STRUCTS
// ===========================================================================

#[derive(OdraSchema, Debug, Clone, ToBytes, FromBytes, CLTyped)]
pub struct CalculateBorrowResult {
    pub borrow_amount: Decimal,
    pub receive_amount: U256,
    pub borrow_fee: U256,
    pub host_fee: U256,
}

#[derive(OdraSchema, Debug, Clone, ToBytes, FromBytes, CLTyped)]
pub struct CalculateRepayResult {
    pub settle_amount: Decimal,
    pub repay_amount: U256,
}

#[derive(OdraSchema, Debug, Clone, ToBytes, FromBytes, CLTyped)]
pub struct CalculateLiquidationResult {
    pub settle_amount: Decimal,
    pub repay_amount: U256,
    pub withdraw_amount: U256,
}