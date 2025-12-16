use anyhow::Result;
use clap::{Parser, Subcommand};

/// Result from a command, separating educational output from copy-paste result
#[derive(Debug, Clone)]
pub struct CommandResult {
    /// Educational output with explanations (🧠, ⚙️, ❄️, etc.)
    pub output: String,
    /// Clean JSON result for copy-pasting
    pub result: String,
}

mod birkhoff;
mod keygen;
mod signing;
mod storage;

#[derive(Parser)]
#[command(name = "yushan")]
#[command(about = "Educational FROST threshold signature workshop", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Round 1 of keygen: Generate polynomial and commitments
    KeygenRound1 {
        /// Threshold (minimum signers needed)
        #[arg(long)]
        threshold: u32,

        /// Total number of parties
        #[arg(long)]
        n_parties: u32,

        /// Your party index (1-based)
        #[arg(long)]
        my_index: u32,

        /// Your HTSS rank (0 = highest authority, higher = lower authority)
        #[arg(long, default_value = "0")]
        rank: u32,

        /// Enable hierarchical threshold secret sharing (HTSS)
        #[arg(long, default_value = "false")]
        hierarchical: bool,
    },

    /// Round 2 of keygen: Exchange shares
    KeygenRound2 {
        /// JSON with all commitments from round 1 (paste from webpage)
        #[arg(long)]
        data: String,
    },

    /// Finalize keygen: Validate and combine shares
    KeygenFinalize {
        /// JSON with all shares sent to you (paste from webpage)
        #[arg(long)]
        data: String,
    },

    /// Generate nonce for signing session
    GenerateNonce {
        /// Signing session ID (must be unique per signature)
        #[arg(long)]
        session: String,
    },

    /// Create signature share
    Sign {
        /// Signing session ID
        #[arg(long)]
        session: String,

        /// Message to sign
        #[arg(long)]
        message: String,

        /// JSON with nonces and group key (paste from webpage)
        #[arg(long)]
        data: String,
    },

    /// Combine signature shares into final signature
    Combine {
        /// JSON with all signature shares (includes message, paste from webpage)
        #[arg(long)]
        data: String,
    },

    /// Verify a Schnorr signature
    Verify {
        /// Signature hex (64 bytes / 128 hex chars)
        #[arg(long)]
        signature: String,

        /// Public key hex
        #[arg(long)]
        public_key: String,

        /// Message that was signed
        #[arg(long)]
        message: String,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::KeygenRound1 {
            threshold,
            n_parties,
            my_index,
            rank,
            hierarchical,
        } => {
            keygen::round1(threshold, n_parties, my_index, rank, hierarchical)?;
        }
        Commands::KeygenRound2 { data } => {
            keygen::round2(&data)?;
        }
        Commands::KeygenFinalize { data } => {
            keygen::finalize(&data)?;
        }
        Commands::GenerateNonce { session } => {
            signing::generate_nonce(&session)?;
        }
        Commands::Sign {
            session,
            message,
            data,
        } => {
            signing::create_signature_share(&session, &message, &data)?;
        }
        Commands::Combine { data } => {
            signing::combine_signatures(&data)?;
        }
        Commands::Verify {
            signature,
            public_key,
            message,
        } => {
            signing::verify_signature(&signature, &public_key, &message)?;
        }
    }

    Ok(())
}
