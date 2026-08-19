use spel_framework::prelude::*;
use sha2::{Sha256, Digest};

const ORACLE_REGISER_LITERAL: &str = "oracle_register__";
const CREATE_KEY: [u8; 32] = [
    0, 0, 0, 0,
    0, 0, 0, 0,
    0, 0, 0, 0,
    0, 0, 0, 0,
    0, 0, 0, 0,
    0, 0, 0, 0,
    0, 0, 0, 0,
    0, 0, 0, 1,
];

/// Raw seed bytes for the vault PDA, for inclusion in a `Propose` instruction's `pda_seeds`.
/// Vault seed: [literal("multisig_vault__"), arg("create_key")] — two-seed multi-hash.
pub fn vault_pda_seed_bytes(create_key: &[u8; 32]) -> [u8; 32] {
    use sha2::{Sha256, Digest};
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

use risc0_zkvm::compute_image_id;
use std::fs;

fn get_program_id_from_path(path: &str) -> [u32; 8] {
    // 1. Read the compiled ELF binary
    let elf_bytes = fs::read(path).expect("Failed to read ELF file");

    // 2. Cryptographically hash it into a RISC Zero Digest
    let digest = compute_image_id(&elf_bytes).expect("Failed to compute Image ID");

    // 3. Convert the Digest into the [u32; 8] ProgramId
    digest.into()
}

fn main() {
    println!("Hello, world!");

    let program_id_path = "../methods/guest/target/riscv32im-risc0-zkvm-elf/docker/oracle_register.bin";
    let program_id = get_program_id_from_path(program_id_path);

    println!("oracle register program id: {:?}", program_id);

    // let program_id = [2874761583, 327026999, 624706346, 1458333779, 165510429, 1411387713, 1999370741, 2083561331];

    let vault_pda_seed_bytes_ = vault_pda_seed_bytes(&CREATE_KEY);
    println!("vault pda seed bytes: {:?}", vault_pda_seed_bytes_);

    // let hex_string = hex::encode(seed_bytes);
    println!("vault pda seed bytes hex: {:?}", hex::encode(vault_pda_seed_bytes_));

    println!("compute vault pda: {:?}", compute_vault_pda(&program_id, &CREATE_KEY));
}
