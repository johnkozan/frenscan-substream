#[path = "src/frensfile/mod.rs"]
mod frensfile;

use anyhow::{Ok, Result};
use indoc::formatdoc;
use std::fs::File;
use std::io::Write;
use std::path::Path;
use substreams_ethereum::Abigen;

use frensfile::{parse_frens_file, FrensFile};

fn main() -> Result<(), anyhow::Error> {
    // Generate ABIs:
    abigen()?;

    // Generate Addresses
    let frens_file = parse_frens_file("./frens.yaml".to_string()).unwrap();
    write_settings_file(&frens_file);
    write_schema_settings_file(&frens_file);

    Ok(())
}

fn abigen() -> Result<(), anyhow::Error> {
    Abigen::new("ERC1155", "abi/erc1155.json")?
        .generate()?
        .write_to_file("src/abi/erc1155.rs")?;

    Abigen::new("ERC721", "abi/erc721.json")?
        .generate()?
        .write_to_file("src/abi/erc721.rs")?;

    Abigen::new("ERC20", "abi/erc20.json")?
        .generate()?
        .write_to_file("src/abi/erc20.rs")?;

    Abigen::new("WETH", "abi/weth.json")?
        .generate()?
        .write_to_file("src/abi/weth.rs")?;

    Ok(())
}

// Create src/settings/mod.rs with constants
fn write_settings_file(frens_file: &FrensFile) {
    let out_dir = "./src/settings";
    let dest_path = Path::new(&out_dir).join("mod.rs");
    let mut f = File::create(&dest_path).unwrap();

    let my_addresses: Vec<String> = frens_file
        .all_addresses()
        .iter()
        .map(|t| normalize_address(&t))
        .collect();

    let treasury_hex_lines: Vec<String> = my_addresses
        .iter()
        .map(|a| format!("hex!(\"{}\"),", a))
        .collect();
    let issued_lines: Vec<String> = frens_file
        .tokens_issued
        .iter()
        .map(|t| {
            let token_id: String = match &t.token_id {
                Some(t) => t.to_string(),
                None => "".to_string(),
            };
            let network: String = match &t.network {
                Some(n) => n.to_string(),
                None => "mainnet".to_string(),
            };
            let schema: String = match &t.schema {
                Some(s) => s.to_string(),
                None => "erc20".to_string(),
            };
            format!(
                "TokenIssued {{
            token_address: hex!(\"{}\"),
            token_id: Some(\"{}\".to_string()),
            schema: Some(\"{}\".to_string()),
            name: \"{}\".to_string(),
            network: Some(\"{}\".to_string()),
            address: \"{}\".to_string(),
            initial_block: {},
        }},",
                normalize_address(&t.address),
                token_id,
                schema,
                t.name,
                network,
                &t.address,
                t.initial_block
            )
        })
        .collect();

    let output = formatdoc! {"
        // @generated
        use crate::frensfile::{{TokenIssued}};
        use substreams::hex;

        lazy_static! {{
            pub static ref TREASURY_ADDRESSES: [[u8;20] ; {}] = [
            {}
            ];

            pub static ref TOKENS_ISSUED: [TokenIssued ; {}] = [
            {}
            ];
        }}
    ",
    treasury_hex_lines.len(), treasury_hex_lines.join("\n"),
    issued_lines.len(), issued_lines.join("\n"),
    };

    f.write_all(output.as_bytes()).unwrap();
}

// Create schema_settings.sql file to be loaded into the DB
fn write_schema_settings_file(frens_file: &FrensFile) {
    let out_dir = ".";
    let dest_path = Path::new(&out_dir).join("schema_settings.sql");
    let mut f = File::create(&dest_path).unwrap();

    let all_addrs = frens_file.all_addresses();
    let address_lines: Vec<String> = all_addrs
        .iter()
        .enumerate()
        .map(|(i, v)| {
            if i == (all_addrs.len() - 1) {
                format!("('{}')", normalize_address(&v))
            } else {
                format!("('{}'),", normalize_address(&v))
            }
        })
        .collect();

    let tokens_issued = &frens_file.tokens_issued;
    let issued_lines: Vec<String> = tokens_issued
        .iter()
        .enumerate()
        .map(|(i, v)| {
            let token_id: String = match &v.token_id {
                Some(tid) => tid.to_string(),
                None => "".to_string(),
            };
            let l = format!("('{}', {})", normalize_address(&v.address), token_id);
            if i == (tokens_issued.len() - 1) {
                return l;
            }
            format!("{},", l)
        })
        .collect();

    let output = formatdoc! {"
        -- @generated
        begin;
        insert into substream1.accounts values
        {}
        on conflict do nothing;

        insert into substream1.tokens_issued values
        {}
        on conflict do nothing;
        commit;
    ",  address_lines.join("\n"), issued_lines.join("\n")
    };

    f.write_all(output.as_bytes()).unwrap();
}

fn normalize_address(addr: &String) -> String {
    if addr.starts_with("0x") {
        return addr[2..].to_lowercase();
    }
    addr.to_string().to_lowercase()
}
