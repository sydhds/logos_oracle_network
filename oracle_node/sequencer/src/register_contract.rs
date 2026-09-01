use anyhow::{anyhow, Context};
use serde::{Serialize, Deserialize};
use sha2::{Sha256, Digest};
use tracing::{info, debug};
use spel_framework::prelude::{seed_from_str, AccountId, ProgramId, compute_pda_multi, ToSeed};
use wallet::WalletCore;
// internal
use oracle_register_client::{
    OracleRegisterClient,
    RegisterAccounts,
    RegisterState as OracleRegisterState
};

pub async fn sequencer_register(
    rc_info: RegisterContractInfo,
) -> anyhow::Result<()> {
    
    let wallet_core = WalletCore::from_env().context("Getting wallet accounts from env")?;
    let oracle_register_program_id = ProgramId::from(rc_info.oracle_register_program_id);
    let client = OracleRegisterClient::new(&wallet_core, oracle_register_program_id);

    debug!("Fetching oracle register state...");
    debug!("client program id: {:?}", client.program_id);
    let register_store = client.fetch_register::<OracleRegisterState>().await
        .map_err(|err| anyhow!("{}", err))?;

    debug!("register store fetched...");

    if !register_store.registered.contains(&rc_info.oracle_node_id) {
        info!("Oracle node (ID {:?}) not registered", &rc_info.oracle_node_id);
        info!("Registering...");

        let to_account = compute_vault_pda(&oracle_register_program_id, &rc_info.oracle_node_id);
        let to_account_pda_seed = vault_pda_seed_bytes(&rc_info.oracle_node_id);

        let accounts = RegisterAccounts {
            // Oracle register contract account
            register: AccountId::new(rc_info.oracle_register_account),
            // An account owned by the oracle node (holding tokens; that will be transferred for staking)
            from: AccountId::new(rc_info.oracle_node_funding_account),
            // The account was will receive the tokens (owned by oracle_register contract)
            // to: AccountId::new(rc_info.oracle_register_to),
            to: to_account,
            // The account holding the token definition
            token_def_account: AccountId::new(rc_info.token_definition_account),
        };

        let response = client.register(
            accounts,
            rc_info.oracle_node_id,
            // rc_info.oracle_register_to_pda_seed
            to_account_pda_seed
        )
            .await
            .map_err(|err| anyhow!("{}", err))?;

        debug!("Tx response: {}", response);

        info!("Waiting for confirmation...");
        let poller = wallet::poller::TxPoller::new(
            wallet_core.config(),
            wallet_core.sequencer_client.clone(),
        );

        let mut tx_hash = [0u8; 32];
        hex::decode_to_slice(response, &mut tx_hash)?;
        match poller.poll_tx(tx_hash.into()).await {
            Ok(_) => {
                debug!("Transaction confirmed — included in a block.")
            },
            Err(err) => {
                return Err(anyhow!("Transaction not confirmed: {}", err))
            },
        }

        info!("Register success!");
    } else {
        info!("Already registered, nothing to do...");
    }

    Ok(())
}

const ORACLE_REGISER_LITERAL: &str = "oracle_register__";

/// Raw seed bytes for the vault PDA, for inclusion in a `Register` instruction's `pda_seeds`.
/// Vault seed: [literal("oracle_register__"), arg("create_key")]
pub fn vault_pda_seed_bytes(create_key: &[u8; 32]) -> [u8; 32] {

    let tag = seed_from_str(ORACLE_REGISER_LITERAL);
    let mut hasher = Sha256::new();
    hasher.update(tag);
    hasher.update(create_key);
    hasher.finalize().into()
}

/// PDA for the multisig vault account.
pub fn compute_vault_pda(program_id: &ProgramId, create_key: &[u8; 32]) -> AccountId {
    let tag = seed_from_str(ORACLE_REGISER_LITERAL);
    compute_pda_multi(program_id, &[&tag as &dyn ToSeed, create_key])
}

#[derive(Serialize, Deserialize, Debug)]
pub struct RegisterContractInfo {
    // Uses [u32; 8]
    #[serde(with = "hex_u32_8")]
    pub oracle_register_program_id: [u32; 8],

    // Uses [u8; 32]
    #[serde(with = "hex_32_bytes")]
    pub oracle_node_id: [u8; 32],

    #[serde(with = "bs58_32_bytes")]
    pub oracle_register_account: [u8; 32],

    #[serde(with = "bs58_32_bytes")]
    pub oracle_node_funding_account: [u8; 32],

    // #[serde(with = "bs58_32_bytes")]
    // pub oracle_register_to: [u8; 32],

    #[serde(with = "bs58_32_bytes")]
    pub token_definition_account: [u8; 32],

    // #[serde(with = "hex_32_bytes")]
    // pub oracle_register_to_pda_seed: [u8; 32],
}

pub mod hex_u32_8 {

    use serde::{Deserialize, Deserializer, Serializer};

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
        println!("AA de on {:?}", s);
        let clean_s = s.strip_prefix("0x").unwrap_or(&s);
        let decoded = hex::decode(clean_s).map_err(serde::de::Error::custom)?;

        if decoded.len() != 32 {
            return Err(serde::de::Error::custom(
                "Hex string must represent exactly 32 bytes (64 hex characters)",
            ));
        }

        let mut array = [0u32; 8];
        for (i, chunk) in decoded.chunks_exact(4).enumerate() {
            array[i] = u32::from_le_bytes(chunk.try_into().unwrap());
        }
        Ok(array)
    }
}

pub mod hex_32_bytes {
    use serde::{Deserialize, Deserializer, Serializer};

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
        println!("AB de on {:?}", s);

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

pub mod bs58_32_bytes {
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S>(bytes: &[u8; 32], serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let s = bs58::encode(bytes).into_string();
        serializer.serialize_str(&s)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<[u8; 32], D::Error>
    where
        D: Deserializer<'de>,
    {
        let s: String = Deserialize::deserialize(deserializer)?;
        let decoded = bs58::decode(&s)
            .into_vec()
            .map_err(serde::de::Error::custom)?;

        decoded.try_into().map_err(|_| {
            serde::de::Error::custom("Base58 string must decode to exactly 32 bytes")
        })
    }
}
