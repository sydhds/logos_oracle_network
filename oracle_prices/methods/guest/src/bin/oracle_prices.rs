#![no_main]

use spel_framework::prelude::*;

risc0_zkvm::guest::entry!(main);

/*
/// The counter state stored on-chain.
///
/// `#[account_type]` registers this in the IDL so `spel inspect <PDA> --type CounterState`
/// can decode raw account bytes into readable JSON.
#[account_type]
#[derive(Debug, Clone, Default, BorshSerialize, BorshDeserialize)]
pub struct CounterState {
    /// The current count value.
    pub count: u64,
    /// The owner who can increment.
    pub owner: [u8; 32],
}
*/

#[account_type]
#[derive(BorshSerialize, BorshDeserialize, Default)]
pub struct OraclePricesState {
    // TODO: for now, everybody can initialize a feed
    //       idea: restrict initialize_feed to registered oracle node
    // owner: [u8; 32],
    feeds: Vec<[u8; 32]>,
}

#[account_type]
#[derive(Debug, Clone, Default, BorshSerialize, BorshDeserialize)]
pub struct PriceState {
    feed_id: [u8; 32], // asset pair identifier, e.g. hash("BTC/USDT")
    price: u64,        // attested median, real value = price * 10^(-decimals)
    decimals: u32,     // number of decimal places in `price`
    valid_count: u32,  // number of observations aggregated in this round
    round: u64,        // round identifier, in Bedrock block-height terms
    confidence: u64,   // OPTIONAL: dispersion of observations, scaled like `price`
}

#[lez_program]
mod my_counter {
    #[allow(unused_imports)]
    use super::*;

    #[instruction]
    pub fn initialize(
        #[account(init, pda = literal("oracle_prices"))]
        mut oracle_prices_account: AccountWithMetadata,
    ) -> SpelResult {

        let state = OraclePricesState {
            feeds: vec![],
        };
        let bytes = borsh::to_vec(&state).map_err(|e| SpelError::SerializationError {
            message: e.to_string(),
        })?;
        oracle_prices_account.account.data = bytes.try_into().unwrap();

        Ok(SpelOutput::execute(vec![oracle_prices_account], vec![]))
    }

    /*
    /// Initialize the counter with an owner.
    ///
    /// Creates a new PDA account derived from the literal seed "counter".
    /// The owner is the signer who can later increment the counter.
    #[instruction]
    pub fn initialize(
        #[account(init, pda = literal("counter"))]
        mut counter: AccountWithMetadata,
        #[account(signer)]
        owner: AccountWithMetadata,
    ) -> SpelResult {
        let state = CounterState {
            count: 0,
            owner: *owner.account_id.value(),
        };
        let bytes = borsh::to_vec(&state).map_err(|e| SpelError::SerializationError {
            message: e.to_string(),
        })?;
        counter.account.data = bytes.try_into().unwrap();

        Ok(SpelOutput::execute(vec![counter, owner], vec![]))
    }
    */

    /*
    /// Increment the counter by a given amount. Only the owner can increment.
    #[instruction]
    pub fn increment(
        #[account(mut, pda = literal("counter"))]
        mut counter: AccountWithMetadata,
        #[account(signer)]
        owner: AccountWithMetadata,
        amount: u64,
    ) -> SpelResult {
        let data: Vec<u8> = counter.account.data.clone().into();
        let mut state: CounterState = borsh::from_slice(&data).map_err(|e| {
            SpelError::DeserializationError {
                account_index: 0,
                message: e.to_string(),
            }
        })?;

        if *owner.account_id.value() != state.owner {
            return Err(SpelError::Unauthorized {
                message: "Only the owner can increment".to_string(),
            });
        }

        state.count = state.count.checked_add(amount).ok_or(SpelError::Overflow {
            operation: "counter increment".to_string(),
        })?;

        let bytes = borsh::to_vec(&state).map_err(|e| SpelError::SerializationError {
            message: e.to_string(),
        })?;
        counter.account.data = bytes.try_into().unwrap();

        Ok(SpelOutput::execute(vec![counter, owner], vec![]))
    }
    */

    #[instruction]
    pub fn initialize_feed(
        #[account(mut, pda = [literal("oracle_prices")])]
        mut oracle_prices_account: AccountWithMetadata,
        #[account(init, pda = [literal("oracle_prices__"), arg("feed_id")])]
        mut feed_price: AccountWithMetadata,
        feed_id: [u8; 32],
    ) -> SpelResult {
        let price = PriceState::default();
        let bytes = borsh::to_vec(&price).map_err(|e| SpelError::SerializationError {
            message: e.to_string(),
        })?;
        feed_price.account.data = bytes.try_into().unwrap();

        // Add feed to oracle prices state
        let data: Vec<u8> = oracle_prices_account.account.data.clone().into();
        let mut state: OraclePricesState = borsh::from_slice(&data).map_err(|e| {
            SpelError::DeserializationError {
                account_index: 0,
                message: e.to_string(),
            }
        })?;
        state.feeds.push(feed_id);
        let bytes = borsh::to_vec(&state).map_err(|e| SpelError::SerializationError {
            message: e.to_string(),
        })?;
        oracle_prices_account.account.data = bytes.try_into().unwrap();

        Ok(SpelOutput::execute(vec![oracle_prices_account, feed_price], vec![]))
    }

    #[instruction]
    pub fn publish_price(
        #[account(mut, pda = [literal("oracle_prices__"), arg("feed_id")])]
        mut feed_price: AccountWithMetadata,
        feed_id: [u8; 32],
        price: u64,
        decimals: u32,
        valid_count: u32,
        round: u64,
        confidence: u64,
    ) -> SpelResult {

        // TODO:
        // check "round" (only allow increasing round)?
        // pb: how to avoid an attacker to set a round too high?

        let price = PriceState {
            feed_id,
            price,
            decimals,
            valid_count,
            round,
            confidence,
        };

        println!("publish price: {:?}", price);

        let bytes = borsh::to_vec(&price).map_err(|e| SpelError::SerializationError {
            message: e.to_string(),
        })?;
        feed_price.account.data = bytes.try_into().unwrap();

        Ok(SpelOutput::execute(vec![feed_price], vec![]))
    }

    /*
    /// Get the current count value (read-only).
    ///
    /// The caller inspects the counter account after the transaction to read the count —
    /// see Step 6 for the `spel inspect … --type CounterState` flow.
    #[instruction]
    pub fn get_count(
        #[account(pda = literal("counter"))]
        counter: AccountWithMetadata,
    ) -> SpelResult {
        Ok(SpelOutput::execute(vec![counter], vec![]))
    }
    */

    /*
    #[instruction]
    pub fn get_published_price(
        #[account(pda = [literal("oracle_prices__"), arg("feed_id")])]
        feed_price: AccountWithMetadata,
        feed_id: [u8; 32],
    ) -> SpelResult {
        Ok(SpelOutput::execute(vec![feed_price], vec![]))
    }
    */
}