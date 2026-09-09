#![no_main]

use spel_framework::prelude::*;
use token_core::{Instruction as TokenInstruction, TokenHolding};
use risc0_zkvm::serde::to_vec;

risc0_zkvm::guest::entry!(main);

#[account_type]
#[derive(Debug, Clone, BorshSerialize, BorshDeserialize)]
pub struct SwapDemoState {
    pub token_program_id: [u32; 8],
}

impl Default for SwapDemoState {
    fn default() -> Self {
        Self {
            token_program_id: [0; 8],
        }
    }
}

#[lez_program]
mod swap_demo {
    use nssa_core::program::Claim::Pda;
    #[allow(unused_imports)]
    use super::*;

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

    /// Initialize the contract
    #[instruction]
    pub fn initialize(
        #[account(init, pda = literal("swap_demo"))]
        mut swap_state: AccountWithMetadata,
        token_program_id: [u32; 8],
    ) -> SpelResult {

        println!("initialize with token program id: {:?}", token_program_id);

        let state = {
            let mut state = SwapDemoState::default();
            state.token_program_id = token_program_id;
            state
        };
        let bytes = borsh::to_vec(&state).map_err(|e| SpelError::SerializationError {
            message: e.to_string(),
        })?;
        swap_state.account.data = bytes.try_into().unwrap();

        Ok(SpelOutput::execute(vec![swap_state], vec![]))
    }

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
    pub fn initialize_pool(
        #[account(mut, pda = literal("swap_demo"))]
        mut swap_state: AccountWithMetadata,
        #[account()]
        token_definition_account: AccountWithMetadata,
        // #[account(init, mut, pda = [literal("swap_demo_pool"), account("token_definition_account")])]
        #[account(mut)]
        mut pool_account: AccountWithMetadata,
        pool_pda_seed: [u8; 32],
    ) -> SpelResult {

        // Note: if we pass #[account(init, mut, pda = ....] we got an "InconsistentAccountPreState"
        //       we need to just use #[account(mut)] here
        // TODO / FIXME: can we just avoid the init and use only put, pda = ... ?

        println!("init - AA");
        println!("pool_account: {:?}", pool_account);

        let data: Vec<u8> = swap_state.account.data.clone().into();
        let mut state: SwapDemoState = borsh::from_slice(&data).map_err(|e| {
            SpelError::DeserializationError {
                account_index: 0,
                message: e.to_string(),
            }
        })?;
        println!("[init] swap state: {:?}", state);
        let token_pg_id = ProgramId::from(state.token_program_id);
        println!("[init] token program id: {:?}", token_pg_id);

        let instruction_init = TokenInstruction::InitializeAccount;
        let instruction_data_init = to_vec(&instruction_init).unwrap();
        println!("[swap demo] instruction_data init: {:?}", instruction_data_init);

        // TODO / FIXME: need this?
        let pool_account_authorized = {
            let mut pool_acc = pool_account.clone();
            pool_acc.is_authorized = true;
            pool_acc
        };

        let chained_call_init = ChainedCall {
            program_id: token_pg_id,
            pre_states: vec![
                token_definition_account.clone(),
                pool_account_authorized
            ],
            instruction_data: instruction_data_init,
            pda_seeds: vec![PdaSeed::new(pool_pda_seed)],
        };

        Ok(SpelOutput::execute(vec![swap_state, token_definition_account, pool_account],
                               vec![
                                   chained_call_init
                               ]))

    }

    /// Swap token A ("from") -> token B ("to")
    #[instruction]
    pub fn swap(
        #[account(mut, pda = literal("swap_demo"))]
        mut swap_state: AccountWithMetadata,
        #[account()]
        price_feed: AccountWithMetadata,
        #[account(signer)]
        mut from: AccountWithMetadata,
        #[account(mut)]
        mut to: AccountWithMetadata,
        #[account(mut)]
        mut pool_a: AccountWithMetadata,
        #[account(mut)]
        mut pool_b: AccountWithMetadata,
        amount: u64,
        pool_b_pda_seed: [u8; 32],
    ) -> SpelResult {

        println!("swap - AC");

        let data: Vec<u8> = swap_state.account.data.clone().into();
        let mut state: SwapDemoState = borsh::from_slice(&data).map_err(|e| {
            SpelError::DeserializationError {
                account_index: 0,
                message: e.to_string(),
            }
        })?;
        println!("swap state: {:?}", state);
        let token_pg_id = ProgramId::from(state.token_program_id);
        println!("token program id: {:?}", token_pg_id);

        // Steps:
        // 1- read price of ETH/USDC in price_feed
        // 2- sub amount to "from" account
        // 3- add amount to "to" account
        // Note: requires some PDA seeds?

        // Step 1: TODO read the price_feed A/B (e.g. ETH/USDC)
        let amount_eth = 1;
        let amount_usdt = 10;

        // Step 2: transfer tokens A: "from" -> "pool A"
        // Note: No pda seed are required here ("from" account is the signer)
        //       while "pool A" is owned by this contract

        let instruction_transfer = TokenInstruction::Transfer { amount_to_transfer: amount_eth };
        let instruction_data_transfer = to_vec(&instruction_transfer).unwrap();
        println!("[swap demo] instruction_data transfer 0: {:?}", instruction_data_transfer);

        let chained_call_transfer_1 = ChainedCall {
            program_id: token_pg_id,
            pre_states: vec![
                // Sender
                from.clone(),
                // Recipient
                pool_a.clone(),
            ],
            instruction_data: instruction_data_transfer,
            pda_seeds: vec![],
        };


        // Step 3: transfer tokens B: "pool B" -> "to"
        // Note: a pda seeds is required for "pool B" (bc "pool B" account is owned by this contract
        //       and it must "sign" the transfer)

        let instruction_transfer = TokenInstruction::Transfer { amount_to_transfer: amount_usdt };
        let instruction_data_transfer = to_vec(&instruction_transfer).unwrap();
        println!("[swap demo] instruction_data transfer: {:?}", instruction_data_transfer);

        // TODO / FIXME: need this - check oracle_register contract?
        let to_authorized = {
            let mut to = to.clone();
            to.is_authorized = true;
            to
        };

        let chained_call_transfer_2 = ChainedCall {
            program_id: token_pg_id,
            pre_states: vec![
                // Sender
                pool_b.clone(),
                // Recipient
                to_authorized,
            ],
            instruction_data: instruction_data_transfer,
            pda_seeds: vec![PdaSeed::new(pool_b_pda_seed)],
        };

        Ok(SpelOutput::execute(vec![swap_state, from, to, pool_a, pool_b],
                               vec![
                                   chained_call_transfer_1,
                                   // FIXME
                                   // chained_call_transfer_2
                               ]))
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
}