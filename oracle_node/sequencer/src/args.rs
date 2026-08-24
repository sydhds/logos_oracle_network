use clap::Parser;

#[derive(Parser, Debug)]
#[command(about = "LON zone sequencer")]
pub struct SequencerArgs {
    /// Logos blockchain node HTTP endpoint
    #[arg(
        long,
        default_value = "http://localhost:8080",
        env = "SEQUENCER_NODE_ENDPOINT"
    )]
    pub node_url: String,

    /// Logos blockchain node REST API endpoint
    #[arg(
        long,
        default_value = "http://localhost:18080",
        env = "SEQUENCER_REST_ENDPOINT"
    )]
    pub node_rest_url: String,

    /// Path to the signing key file (created if it doesn't exist)
    #[arg(
        long,
        default_value = "./data/sequencer.key",
        env = "SEQUENCER_SIGNING_KEY_PATH"
    )]
    pub key_path: String,

    /// Path to the signing key file (created if it doesn't exist)
    #[arg(
        long,
        default_value = "./data/oracle.key",
        env = "SEQUENCER_ORACLE_SIGNING_KEY_PATH"
    )]
    pub oracle_key_path: String,

    /// Basic auth username for node endpoint
    #[arg(long, env = "SEQUENCER_NODE_AUTH_USERNAME")]
    pub node_auth_username: Option<String>,

    /// Basic auth password for node endpoint
    #[arg(long, env = "SEQUENCER_NODE_AUTH_PASSWORD")]
    pub node_auth_password: Option<String>,

    /// Path to the checkpoint file for crash recovery
    #[arg(
        long,
        default_value = "./data/sequencer.checkpoint",
        env = "CHECKPOINT_PATH"
    )]
    pub checkpoint_path: String,

    /// Path to the channel ID file
    #[arg(long, default_value = "./data/channel.txt", env = "CHANNEL_PATH")]
    pub channel_path: String,
}