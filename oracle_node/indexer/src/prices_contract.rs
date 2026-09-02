use anyhow::{anyhow, Context};
// third-party - LEZ
use wallet::WalletCore;
use spel_framework::prelude::{seed_from_str, AccountId, ProgramId, compute_pda_multi, ToSeed};
// internal
use common::PricesContractInfo;
use oracle_prices_client::{OraclePricesClient, PublishPriceAccounts};
use crate::indexer::lon::AttestedPrice;

pub async fn publish_attested_price(pc_info: &PricesContractInfo, attested_price: AttestedPrice) -> anyhow::Result<()> {

    let wallet_core = WalletCore::from_env()
        .context("Getting wallet accounts from env")?;
    let client = OraclePricesClient::new(&wallet_core, pc_info.oracle_prices_program_id);

    let accounts = PublishPriceAccounts {
        feed_price: AccountId::default(),
    };

    client.publish_price(accounts,
                         attested_price.feed_id.as_slice().try_into()?,
                         attested_price.price.try_into()?,
                         attested_price.decimals.try_into()?,
                         attested_price.valid_count,
                         attested_price.round.try_into()?,
                         attested_price.confidence.try_into()?,
    )
        .await
        .map_err(|err| anyhow!(err))?;

    Ok(())
}