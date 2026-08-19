use serde_json::Value;
use spel_framework::prelude::AccountId;
// use tracing_subscriber::registry::Data;
use wallet::WalletCore;
use spel_framework_core::decode::decode_account_data;
use spel_framework_core::idl::SpelIdl;

async fn fetch_account_data(wallet_core: &WalletCore, account_id: &AccountId) -> anyhow::Result<Vec<u8>> {
    let account = wallet_core
        .get_account_public(*account_id)
        .await?;
    let res = account.data;
    Ok(res.to_vec())
}

async fn fetch_registered(wallet_core: &WalletCore, account_id: &AccountId, idl: &SpelIdl) -> anyhow::Result<Value> {
    let data = fetch_account_data(wallet_core, account_id).await?;
    let decoded = decode_account_data(data.as_slice(), "reg", idl)
        .map_err(|err| anyhow::anyhow!("error decoding account data: {}", err))?;
    Ok(decoded)
}