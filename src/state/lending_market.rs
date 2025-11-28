use odra::prelude::*;

const UNINITIALIZED_VERSION: u8 = 0;
const PROGRAM_VERSION: u8 = 1;

/// Lending market storage for Odra (Casper) migration
#[odra::module]
pub struct LendingMarketModule {
    version: Var<u8>,
    bump_seed: Var<u8>,
    owner: Var<[u8; 32]>,
    quote_currency: Var<[u8; 32]>,
    token_program_id: Var<[u8; 32]>,
    oracle_program_id: Var<[u8; 32]>,
}

#[odra::module]
impl LendingMarketModule {
    /// Initialize lending market
    pub fn init(&mut self, params: InitLendingMarketParams) {
        self.version.set(PROGRAM_VERSION);
        self.bump_seed.set(params.bump_seed);
        self.owner.set(params.owner);
        self.quote_currency.set(params.quote_currency);
        self.token_program_id.set(params.token_program_id);
        self.oracle_program_id.set(params.oracle_program_id);
    }

    /// Read-only accessors
    pub fn get_version(&self) -> u8 { self.version.get_or_default() }
    pub fn get_bump_seed(&self) -> u8 { self.bump_seed.get_or_default() }
    pub fn get_owner(&self) -> [u8; 32] { self.owner.get_or_default() }
    pub fn get_quote_currency(&self) -> [u8; 32] { self.quote_currency.get_or_default() }
    pub fn get_token_program_id(&self) -> [u8; 32] { self.token_program_id.get_or_default() }
    pub fn get_oracle_program_id(&self) -> [u8; 32] { self.oracle_program_id.get_or_default() }
}

/// Parameters for initializing the lending market (transient)
#[odra::odra_type]
pub struct InitLendingMarketParams {
    pub bump_seed: u8,
    pub owner: [u8; 32],
    pub quote_currency: [u8; 32],
    pub token_program_id: [u8; 32],
    pub oracle_program_id: [u8; 32],
}

// Notes:
// - This file migrates the Solana `LendingMarket` state into an Odra module backed
//   by Var<T> storage primitives. Pubkeys are represented as fixed-size arrays [u8;32].
// - The old Pack/IsInitialized/Sealed traits and byte-level packing are removed;
//   Odra manages on-chain storage by keys. If deterministic byte layout is required
//   (for migrations), add explicit serialize/deserialize helpers.
// - If you need to interact with Solana-style `Pubkey` off-chain, convert between
//   `[u8;32]` and `Pubkey` in your client tooling.
// - Consider adding access control (e.g., only_owner) guards using `odr::access_control` macros
//   or manual checks on public methods that should be restricted.

// Suggested next steps:
// 1. Add access control methods (only owner can update certain config fields).
// 2. Add update/setters for mutable fields if lending market supports runtime updates.
// 3. Wire up error mapping to Odra-friendly errors by ensuring `LendingError` is Odra-compatible.
// 4. Port tests from Solana to `cargo odra test` and adjust any client-side conversions.