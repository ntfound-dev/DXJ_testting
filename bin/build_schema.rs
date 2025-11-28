// File: my-project/bin/build_schema.rs
#![allow(unused_imports)]

use odra::prelude::*;
// Import semua kontrak yang sudah Anda export di lib.rs
use my_project::{
    // Core
    Comptroller, ComptrollerStorage,
    
    // Tokens
    CErc20, CErc20Immutable, CEther, CToken,
    
    // Oracles
    NovaPriceOracle, MockPriceOracle,
    
    // Interest Rate Models
    BaseJumpRateModel, JumpRateModelV2, WhitePaperInterestRateModel,
    
    // Mocks
    MockToken
};

fn main() {
    // Kita menggabungkan (merge) schema dari semua kontrak.
    // Urutan tidak masalah, yang penting semua yang ingin Anda deploy ada di sini.
    let schema = Comptroller::module_schema()
        // Storage (Opsional, biasanya menyatu dengan logic)
        .merge(ComptrollerStorage::module_schema())
        
        // Tokens
        .merge(CErc20::module_schema())
        .merge(CErc20Immutable::module_schema())
        .merge(CEther::module_schema())
        .merge(CToken::module_schema()) // Base logic
        
        // Oracles
        .merge(NovaPriceOracle::module_schema())
        .merge(MockPriceOracle::module_schema())
        
        // Interest Rates
        .merge(BaseJumpRateModel::module_schema())
        .merge(JumpRateModelV2::module_schema())
        .merge(WhitePaperInterestRateModel::module_schema())
        
        // Mocks
        .merge(MockToken::module_schema());

    // Cetak output JSON ke stdout (agar ditangkap oleh odra-cli)
    println!("{}", schema.as_json().expect("Failed to generate schema"));
}