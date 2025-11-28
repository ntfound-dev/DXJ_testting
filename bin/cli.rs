// FILE: bin/cli.rs
// FIXED: Replaced Flipper with Comptroller & Added necessary traits

use odra::host::{HostEnv, NoArgs};
// FIX: Import Addressable agar fungsi .address() bisa dipakai
use odra::prelude::Addressable; 

use odra_cli::{
    deploy::DeployScript,
    scenario::{Args, Error, Scenario, ScenarioMetadata},
    CommandArg, DeployedContractsContainer, OdraCli, 
    // FIX: Import trait ini wajib agar fungsi load_or_deploy dan contract_ref jalan
    DeployerExt, ContractProvider, 
};

// FIX: Import Kontrak Comptroller Anda, BUKAN Flipper
use my_project::contracts::comptroller::Comptroller;

/// Script untuk deploy Comptroller
pub struct ComptrollerDeployScript;

impl DeployScript for ComptrollerDeployScript {
    fn deploy(
        &self,
        env: &HostEnv,
        container: &mut DeployedContractsContainer
    ) -> Result<(), odra_cli::deploy::Error> {
        
        println!("🚀 Starting deployment of NOVA Comptroller...");

        // 2. DEPLOY COMPTROLLER
        // Menggunakan load_or_deploy dari trait DeployerExt
        let comptroller = Comptroller::load_or_deploy(
            &env,
            NoArgs, // Init function Comptroller tidak butuh argumen
            container,
            200_000_000_000 // Gas limit (200 CSPR - aman untuk testnet)
        )?;

        println!("✅ Comptroller Deployed at: {:?}", comptroller.address());

        Ok(())
    }
}

/// Scenario sederhana: Cek Admin Comptroller
pub struct CheckAdminScenario;

impl Scenario for CheckAdminScenario {
    fn args(&self) -> Vec<CommandArg> {
        vec![] // Tidak butuh argumen input dari terminal
    }

    fn run(
        &self,
        env: &HostEnv,
        container: &DeployedContractsContainer,
        _args: Args
    ) -> Result<(), Error> {
        // Ambil referensi kontrak yang sudah dideploy
        let contract = container.contract_ref::<Comptroller>(env)?;
        
        println!("🔍 Running Scenario: Check Admin...");
        let admin = contract.admin();
        println!("👑 Admin Address is: {:?}", admin);

        Ok(())
    }
}

impl ScenarioMetadata for CheckAdminScenario {
    const NAME: &'static str = "check-admin";
    const DESCRIPTION: &'static str = "Checks the admin of the comptroller contract";
}

/// Main function
pub fn main() {
    OdraCli::new()
        .about("CLI tool for NOVA Protocol")
        // Daftarkan Script Deploy Baru
        .deploy(ComptrollerDeployScript) 
        // Daftarkan Kontrak Comptroller
        .contract::<Comptroller>() 
        // Daftarkan Skenario Test
        .scenario(CheckAdminScenario) 
        .build()
        .run();
}