#![no_main]
extern crate core;

use spel_framework::prelude::*;
use oracle_register_core::imt::{OracleMerkleTree, TREE_CAPACITY};

risc0_zkvm::guest::entry!(main);

/// The counter state stored on-chain.
///
/// `#[account_type]` registers this in the IDL so `spel inspect <PDA> --type CounterState`
/// can decode raw account bytes into readable JSON.
#[account_type]
#[derive(Debug, Clone, BorshSerialize, BorshDeserialize)]
pub struct RegisterState {
    // /// The current count value.
    // pub count: u64,
    /// The owner
    pub owner: [u8; 32],
    pub mtree: OracleMerkleTree,
    // Note: with tree depth of 10, this is 32 * 1024 -> 32Kb so ok
    pub registered: [[u8; 32]; TREE_CAPACITY],
}

impl Default for RegisterState {
    fn default() -> Self {
        Self {
            owner: [0; 32],
            mtree: Default::default(),
            registered: [[0u8; 32]; TREE_CAPACITY],
        }
    }
}

#[lez_program]
mod my_counter {
    use core::panic::PanicMessage;
    #[allow(unused_imports)]
    use super::*;

    /// Initialize the register contract with an owner.
    ///
    /// Creates a new PDA account derived from the literal seed "register".
    #[instruction]
    pub fn initialize(
        #[account(init, pda = literal("register"))]
        mut counter: AccountWithMetadata,
        #[account(signer)]
        owner: AccountWithMetadata,
    ) -> SpelResult {
        let state = RegisterState {
            owner: *owner.account_id.value(),
            mtree: OracleMerkleTree::new(),
            registered: [[0u8; 32]; TREE_CAPACITY],
        };
        let bytes = borsh::to_vec(&state).map_err(|e| SpelError::SerializationError {
            message: e.to_string(),
        })?;
        counter.account.data = bytes.try_into().unwrap();

        Ok(SpelOutput::execute(vec![counter, owner], vec![]))
    }

    #[instruction]
    pub fn register(
        #[account(mut, pda = literal("register"))]
        mut counter: AccountWithMetadata,
        #[account(signer)]
        owner: AccountWithMetadata,
    ) -> SpelResult {

        println!("[print] register");
        eprintln!("[eprint] register");

        let data: Vec<u8> = counter.account.data.clone().into();
        let mut state: RegisterState = borsh::from_slice(&data).map_err(|e| {
            SpelError::DeserializationError {
                account_index: 0,
                message: e.to_string(),
            }
        })?;

        // Note: let everyone to register (for now, staking will be added later)
        /*
        if *owner.account_id.value() != state.owner {
            return Err(SpelError::Unauthorized {
                message: "Only the owner can increment".to_string(),
            });
        }
        */

        /*
        state.count = state.count.checked_add(amount).ok_or(SpelError::Overflow {
            operation: "counter increment".to_string(),
        })?;
        */

        let pk = [46u8; 32];
        state.mtree.insert_oracle(pk).map_err(|e| SpelError::Custom { code: 0, message: e.to_string() })?;
        let registered_idx = state.mtree.next_index.saturating_sub(1) as usize;
        println!("registered_idx: {}", registered_idx);
        state.registered[registered_idx] = pk;

        println!("state registered len: {}", state.registered.len());
        println!("state registered 0: {:?}", state.registered[0]);

        let bytes = borsh::to_vec(&state).map_err(|e| SpelError::SerializationError {
            message: e.to_string(),
        })?;
        println!("bytes len: {}", bytes.len());
        counter.account.data = bytes.try_into().unwrap();

        Ok(SpelOutput::execute(vec![counter, owner], vec![]))
    }

    /*
    /// Increment the counter by a given amount. Only the owner can increment.
    #[instruction]
    pub fn increment(
        #[account(mut, pda = literal("register"))]
        mut counter: AccountWithMetadata,
        #[account(signer)]
        owner: AccountWithMetadata,
        amount: u64,
    ) -> SpelResult {
        let data: Vec<u8> = counter.account.data.clone().into();
        let mut state: RegisterState = borsh::from_slice(&data).map_err(|e| {
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

    /// Get the current count value (read-only).
    ///
    /// The caller inspects the counter account after the transaction to read the count —
    /// see Step 6 for the `spel inspect … --type CounterState` flow.
    #[instruction]
    pub fn get_register_state(
        #[account(pda = literal("register"))]
        counter: AccountWithMetadata,
    ) -> SpelResult {
        println!("[print] get_register_state");
        eprintln!("[eprint] get_register_state");
        // info!("[eprint] get_register_state");
        Ok(SpelOutput::execute(vec![counter], vec![]))
    }
}