#![no_main]

use spel_framework::prelude::*;
use nssa_core::account::Data;

risc0_zkvm::guest::entry!(main);

#[lez_program]
mod oracle_register {
    #[allow(unused_imports)]
    use super::*;

    /// Program state stored in a PDA account.
    #[derive(BorshSerialize, BorshDeserialize)]
    #[account_type]
    pub struct ProgramState {
        pub initialized: bool,
        pub owner: [u8; 32],
    }

    /// Initialize the program state.
    #[instruction]
    pub fn initialize(
        _ctx: ProgramContext,
        #[account(init, pda = literal("state"))]
        mut state: AccountWithMetadata,
        #[account(signer)]
        owner: AccountWithMetadata,
    ) -> SpelResult {
        let ps = ProgramState {
            initialized: true,
            owner: *owner.account_id.value(),
        };
        let bytes = borsh::to_vec(&ps).map_err(|e| SpelError::custom(999, format!("borsh error: {e}")))?;
        state.account.data = Data::try_from(bytes).map_err(|_| SpelError::custom(999, "data too big"))?;
        Ok(SpelOutput::execute(vec![state, owner], vec![]))
    }

    /// Example instruction — replace with your own.
    #[instruction]
    pub fn do_something(
        #[account(mut, pda = literal("state"))]
        state: AccountWithMetadata,
        #[account(signer)]
        owner: AccountWithMetadata,
        _amount: u64,
    ) -> SpelResult {
        // TODO: implement your logic
        Ok(SpelOutput::execute(vec![state, owner], vec![]))
    }
}
