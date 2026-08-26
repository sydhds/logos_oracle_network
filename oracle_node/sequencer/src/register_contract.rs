// use std::collections::HashMap;
// use std::path::Path;
use oracle_register_client::{OracleRegisterClient, RegisterAccounts as OracleRegisterAccounts, RegisterAccounts, RegisterState as OracleRegisterState};
use anyhow::{anyhow, Context};
use serde::{Serialize, Deserialize};
// use spel::tx::execute_instruction;
// use spel_framework::idl::SpelIdl;
use spel_framework::prelude::{AccountId, ProgramId};
use tracing::info;
use wallet::WalletCore;

pub async fn sequencer_register(
    rc_info: RegisterContractInfo,
) -> anyhow::Result<()> {
    
    let wallet_core = WalletCore::from_env()?;
    let client = OracleRegisterClient::new(&wallet_core, 
                                           ProgramId::from(rc_info.oracle_register_program_id));

    let register_store = client.fetch_register::<OracleRegisterState>().await
        .map_err(|err| anyhow!("{}", err))?;

    if !register_store.registered.contains(&rc_info.oracle_node_id) {
        info!("Oracle node (ID {:?}) not registered", &rc_info.oracle_node_id);
        info!("Registering...");

        let accounts = RegisterAccounts {
            // Oracle register contract account
            register: AccountId::from(rc_info.oracle_register_account),
            // An account owned by the oracle node (with tokens; to stake)
            from: AccountId::from(rc_info.oracle_node_funding_account),
            // The account was will receive the stake
            to: AccountId::from(rc_info.oracle_register_to),
            // The account holding the token definition
            token_def_account: AccountId::from(rc_info.token_definition_account),
        };

        client.register(accounts, rc_info.oracle_register_to_pda_seed).await
            .map_err(|err| anyhow!("{}", err))?;

        info!("Register success!");
    }

    Ok(())
}


/*
async fn idl_parse(idl_path: &Path) -> anyhow::Result<SpelIdl> {
    let idl_content = std::fs::read_to_string(idl_path)
        .context(format!("Error reading IDL file: {}", idl_path.display()))?;
    let idl: SpelIdl = serde_json::from_str(&idl_content)?;
    Ok(idl)
}

async fn sequencer_register(idl: &SpelIdl, program_id_hex: &str) -> anyhow::Result<()> {
    info!("Stating sequencer register...");
    let ix_name = "register";
    let ix = idl
        .instructions
        .iter()
        .find(|ix| ix.name == ix_name)
        .ok_or(anyhow!("Unable to find instruction {}", ix_name))?;
    let args = HashMap::new();
    let program_path = None;
    let dry_run = None;
    let extra_bins = HashMap::new();

    // Note:
    // * execute_instruction will call process::exit on failure - TODO: rewrite function
    //  * on a rewrite - separate the tx submission from tx waiting
    execute_instruction(idl, ix, &args, program_path, Some(program_id_hex), dry_run, &extra_bins).await;

    info!("Register complete");

    Ok(())
}
*/


#[derive(Serialize, Deserialize, Debug)]
pub struct RegisterContractInfo {
    // Uses [u32; 8]
    #[serde(with = "hex_u32_8")]
    pub oracle_register_program_id: [u32; 8],

    // Uses [u8; 32]
    #[serde(with = "hex_32_bytes")]
    pub oracle_node_id: [u8; 32],

    #[serde(with = "hex_32_bytes")]
    pub oracle_register_account: [u8; 32],

    #[serde(with = "hex_32_bytes")]
    pub oracle_node_funding_account: [u8; 32],

    #[serde(with = "hex_32_bytes")]
    pub oracle_register_to: [u8; 32],

    #[serde(with = "hex_32_bytes")]
    pub token_definition_account: [u8; 32],

    #[serde(with = "hex_32_bytes")]
    pub oracle_register_to_pda_seed: [u8; 32],
}

pub mod hex_u32_8 {

    use serde::{Deserialize, Deserializer, Serializer};
    use std::convert::TryInto;

    // Converts [u32; 8] -> 32 bytes -> hex string
    pub fn serialize<S>(array: &[u32; 8], serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut bytes = [0u8; 32];
        for (i, &val) in array.iter().enumerate() {
            bytes[i * 4..(i + 1) * 4].copy_from_slice(&val.to_be_bytes());
        }
        let hex_string = hex::encode(bytes);
        serializer.serialize_str(&hex_string)
    }

    // Converts hex string -> 32 bytes -> [u32; 8]
    pub fn deserialize<'de, D>(deserializer: D) -> Result<[u32; 8], D::Error>
    where
        D: Deserializer<'de>,
    {
        let s: String = Deserialize::deserialize(deserializer)?;
        let clean_s = s.strip_prefix("0x").unwrap_or(&s);
        let decoded = hex::decode(clean_s).map_err(serde::de::Error::custom)?;

        if decoded.len() != 32 {
            return Err(serde::de::Error::custom(
                "Hex string must represent exactly 32 bytes (64 hex characters)",
            ));
        }

        let mut array = [0u32; 8];
        for (i, chunk) in decoded.chunks_exact(4).enumerate() {
            array[i] = u32::from_be_bytes(chunk.try_into().unwrap());
        }
        Ok(array)
    }
}

pub mod hex_32_bytes {
    use serde::{Deserialize, Deserializer, Serializer};
    use std::convert::TryInto;

    // Converts the [u8; 32] array into a hex string for JSON
    pub fn serialize<S>(bytes: &[u8; 32], serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let hex_string = hex::encode(bytes);
        // Use format!("0x{}", hex_string) if your config requires the 0x prefix!
        serializer.serialize_str(&hex_string)
    }

    // Converts the JSON hex string back into a [u8; 32] array
    pub fn deserialize<'de, D>(deserializer: D) -> Result<[u8; 32], D::Error>
    where
        D: Deserializer<'de>,
    {
        let s: String = Deserialize::deserialize(deserializer)?;

        // Strip the "0x" prefix if it exists so hex::decode doesn't panic
        let clean_s = s.strip_prefix("0x").unwrap_or(&s);

        // Decode the hex string into a Vec<u8>
        let decoded = hex::decode(clean_s).map_err(serde::de::Error::custom)?;

        // Ensure it is exactly 32 bytes and convert it to a fixed array
        decoded.try_into().map_err(|_| {
            serde::de::Error::custom("Hex string must represent exactly 32 bytes")
        })
    }
}
