// File: my-project/bin/build_contract.rs
#![no_std]
#![no_main]

#[allow(unused_imports)]
// Mengimpor library utama kita. 
// Ini memicu macro #[odra::module] di lib.rs untuk men-generate entry points Wasm.
use my_project; 

