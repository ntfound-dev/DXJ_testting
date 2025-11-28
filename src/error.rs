// Correct imports for Odra v2.4

use odra::prelude::*;
use core::fmt;  // ADD THIS IMPORT

// Use the 'odra_error' attribute to define your error enum
#[odra::odra_error]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LendingError {
    // 0
    InstructionUnpackError = 0,
    AlreadyInitialized = 1,
    NotRentExempt = 2,
    InvalidMarketAuthority = 3,
    InvalidMarketOwner = 4,

    // 5
    InvalidAccountOwner = 5,
    InvalidTokenOwner = 6,
    InvalidTokenAccount = 7,
    InvalidTokenMint = 8,
    InvalidTokenProgram = 9,

    // 10
    InvalidAmount = 10,
    InvalidConfig = 11,
    InvalidSigner = 12,
    InvalidAccountInput = 13,
    MathOverflow = 14,

    // 15
    TokenInitializeMintFailed = 15,
    TokenInitializeAccountFailed = 16,
    TokenTransferFailed = 17,
    TokenMintToFailed = 18,
    TokenBurnFailed = 19,

    // 20
    InsufficientLiquidity = 20,
    ReserveCollateralDisabled = 21,
    ReserveStale = 22,
    WithdrawTooSmall = 23,
    WithdrawTooLarge = 24,

    // 25
    BorrowTooSmall = 25,
    BorrowTooLarge = 26,
    RepayTooSmall = 27,
    LiquidationTooSmall = 28,
    ObligationHealthy = 29,

    // 30
    ObligationStale = 30,
    ObligationReserveLimit = 31,
    InvalidObligationOwner = 32,
    ObligationDepositsEmpty = 33,
    ObligationBorrowsEmpty = 34,

    // 35
    ObligationDepositsZero = 35,
    ObligationBorrowsZero = 36,
    InvalidObligationCollateral = 37,
    InvalidObligationLiquidity = 38,
    ObligationCollateralEmpty = 39,

    // 40
    ObligationLiquidityEmpty = 40,
    NegativeInterestRate = 41,
    InvalidOracleConfig = 42,
    InvalidFlashLoanReceiverProgram = 43,
    NotEnoughLiquidityAfterFlashLoan = 44,

    // 45
    ExceededSlippage = 45,
    //MathOverflow = 46,
    InsufficientCollateral = 47,
}

impl LendingError {
    pub fn message(&self) -> &str {
        match self {
            LendingError::InstructionUnpackError => "Failed to unpack instruction data",
            LendingError::AlreadyInitialized => "Account is already initialized",
            LendingError::NotRentExempt => "Lamport balance below rent-exempt threshold",
            LendingError::InvalidMarketAuthority => "Market authority is invalid",
            LendingError::InvalidMarketOwner => "Market owner is invalid",
            LendingError::InvalidAccountOwner => "Input account owner is not the program address",
            LendingError::InvalidTokenOwner => "Input token account is not owned by the correct token program id",
            LendingError::InvalidTokenAccount => "Input token account is not valid",
            LendingError::InvalidTokenMint => "Input token mint account is not valid",
            LendingError::InvalidTokenProgram => "Input token program account is not valid",
            LendingError::InvalidAmount => "Input amount is invalid",
            LendingError::InvalidConfig => "Input config value is invalid",
            LendingError::InvalidSigner => "Input account must be a signer",
            LendingError::InvalidAccountInput => "Invalid account input",
            LendingError::MathOverflow => "Math operation overflow",
            LendingError::TokenInitializeMintFailed => "Token initialize mint failed",
            LendingError::TokenInitializeAccountFailed => "Token initialize account failed",
            LendingError::TokenTransferFailed => "Token transfer failed",
            LendingError::TokenMintToFailed => "Token mint to failed",
            LendingError::TokenBurnFailed => "Token burn failed",
            LendingError::InsufficientLiquidity => "Insufficient liquidity available",
            LendingError::ReserveCollateralDisabled => "Input reserve has collateral disabled",
            LendingError::ReserveStale => "Reserve state needs to be refreshed",
            LendingError::WithdrawTooSmall => "Withdraw amount too small",
            LendingError::WithdrawTooLarge => "Withdraw amount too large",
            LendingError::BorrowTooSmall => "Borrow amount too small to receive liquidity after fees",
            LendingError::BorrowTooLarge => "Borrow amount too large for deposited collateral",
            LendingError::RepayTooSmall => "Repay amount too small to transfer liquidity",
            LendingError::LiquidationTooSmall => "Liquidation amount too small to receive collateral",
            LendingError::ObligationHealthy => "Cannot liquidate healthy obligations",
            LendingError::ObligationStale => "Obligation state needs to be refreshed",
            LendingError::ObligationReserveLimit => "Obligation reserve limit exceeded",
            LendingError::InvalidObligationOwner => "Obligation owner is invalid",
            LendingError::ObligationDepositsEmpty => "Obligation deposits are empty",
            LendingError::ObligationBorrowsEmpty => "Obligation borrows are empty",
            LendingError::ObligationDepositsZero => "Obligation deposits have zero value",
            LendingError::ObligationBorrowsZero => "Obligation borrows have zero value",
            LendingError::InvalidObligationCollateral => "Invalid obligation collateral",
            LendingError::InvalidObligationLiquidity => "Invalid obligation liquidity",
            LendingError::ObligationCollateralEmpty => "Obligation collateral is empty",
            LendingError::ObligationLiquidityEmpty => "Obligation liquidity is empty",
            LendingError::NegativeInterestRate => "Interest rate is negative",
            LendingError::InvalidOracleConfig => "Input oracle config is invalid",
            LendingError::InvalidFlashLoanReceiverProgram => "Input flash loan receiver program account is not valid",
            LendingError::NotEnoughLiquidityAfterFlashLoan => "Not enough liquidity after flash loan",
            LendingError::ExceededSlippage  => "Amount smaller than desired slippage limit",
            LendingError::InsufficientCollateral => "kolekteral abis",
            //LendingError::MathOverflow =>"mate",
        }
    }
}

impl core::fmt::Display for LendingError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message())
    }
}