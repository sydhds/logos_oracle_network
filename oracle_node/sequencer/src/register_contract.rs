use std::collections::HashMap;
use std::path::Path;
use anyhow::{anyhow, Context};
use spel::tx::execute_instruction;
use spel_framework::idl::SpelIdl;
use tracing::info;

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