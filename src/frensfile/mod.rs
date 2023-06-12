use serde::{Deserialize, Serialize};
use serde_yaml::{self};

#[derive(Debug, Serialize, Deserialize)]
pub struct FrensFile {
    pub version: String,
    pub tokens_issued: Vec<TokenIssued>,
    pub treasury_accounts: Vec<TreasuryAccount>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TokenIssued {
    pub name: String,
    pub address: String, // token addres is deserialized into address, then converted into
    // token_address
    #[serde(skip_deserializing)]
    pub token_address: [u8; 20],
    pub token_id: Option<String>,
    pub network: Option<String>, // optional, defaults to mainnet
    pub schema: Option<String>,  // optional, defaults to erc20  TODO: use enum?
    pub initial_block: u64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TreasuryAccount {
    pub name: String,
    pub address: String,
    pub network: Option<String>, // TODO: Not used.  implement multinetwork support
    pub initial_block: u64,
}

impl FrensFile {
    #[allow(dead_code)]
    pub fn all_addresses(&self) -> Vec<String> {
        let mut all_addrs: Vec<String> = Vec::new();
        let treasury_addrs: Vec<String> = self
            .treasury_accounts
            .iter()
            .map(|a| a.address.clone())
            .collect();
        let issued_addrs: Vec<String> = self
            .tokens_issued
            .iter()
            .map(|a| a.address.clone())
            .collect();
        all_addrs.extend(treasury_addrs);
        all_addrs.extend(issued_addrs);
        all_addrs
    }
}

#[allow(dead_code)]
pub fn parse_frens_file(file_name: String) -> Option<FrensFile> {
    let f = std::fs::File::open(file_name).expect("Could not open frens.yaml");
    let frens_file: FrensFile = serde_yaml::from_reader(f).expect("Could not read values.");

    Some(frens_file)
}
