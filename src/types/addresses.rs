//! Network addresses and pool definitions

use alloy::primitives::{Address, address};

// Network-specific constants
pub const WETH_MAINNET: Address = address!("4200000000000000000000000000000000000006");
pub const USDC_MAINNET: Address = address!("833589fCD6eDb6E08f4c7C32D4f71b54bdA02913");
pub const USDBC_MAINNET: Address = address!("d9aAEc86B65D86f6A7B5B1b0c42FFA531710b6CA");

// Base Sepolia testnet addresses
pub const WETH_SEPOLIA: Address = address!("4200000000000000000000000000000000000006");
pub const USDC_SEPOLIA: Address = address!("AF33ADd7918F685B2A82C1077bd8c07d220FFA04"); // Base Sepolia USDC
#[allow(dead_code)]
pub const UNISWAP_V2_ROUTER_SEPOLIA: Address = address!("0xC532a74256D3Db42D0Bf7a0400fEFDbad7694008");

// Mainnet pools
pub const POOLS_MAINNET: &[(&str, Address)] = &[
    ("vAMM-WETH/USDbC", address!("B4885Bc63399BF5518b994c1d0C153334Ee579D0")),
    ("WETH/USDC", address!("cDAC0d6c6C59727a65F871236188350531885C43")),
];

// Sepolia testnet pools (for execution)
pub const POOLS_SEPOLIA: &[(&str, Address)] = &[
    ("WETH/USDC-Sepolia", address!("92b8274aba7ab667bee7eb776ec1de32438d90bf")), 
];
