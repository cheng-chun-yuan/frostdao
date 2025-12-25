use anyhow::Result;
use clap::{Parser, Subcommand};

// Use library crate for core functionality
use frostdao::btc::{schnorr as bitcoin_schnorr, transaction as bitcoin_tx};
use frostdao::protocol::{dkg_tx, keygen, recovery, reshare, signing};
use frostdao::storage::Storage; // For HD commands

// TUI is CLI-only, not part of lib
mod tui;

#[derive(Parser)]
#[command(name = "frostdao")]
#[command(about = "FrostDAO - FROST threshold signatures for Bitcoin", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Round 1 of keygen: Generate polynomial and commitments
    KeygenRound1 {
        /// Wallet/session name (creates .frost_state/<name>/ folder)
        #[arg(long)]
        name: String,

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
        /// Wallet/session name (must match round1)
        #[arg(long)]
        name: String,

        /// JSON with all commitments from round 1 (paste from webpage)
        #[arg(long)]
        data: String,
    },

    /// Finalize keygen: Validate and combine shares
    KeygenFinalize {
        /// Wallet/session name (must match round1)
        #[arg(long)]
        name: String,

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

    // ========================================================================
    // Bitcoin Schnorr (BIP340) Commands
    // ========================================================================
    /// Generate a new Bitcoin Schnorr keypair (BIP340)
    BtcKeygen,

    /// Import an existing Bitcoin secret key
    BtcImportKey {
        /// Secret key in hex (32 bytes / 64 hex chars)
        #[arg(long)]
        secret: String,
    },

    /// Get the stored Bitcoin public key
    BtcPubkey,

    /// Sign a message with Bitcoin Schnorr (BIP340)
    BtcSign {
        /// Message to sign (UTF-8 string)
        #[arg(long)]
        message: String,
    },

    /// Sign a hex-encoded message with Bitcoin Schnorr (BIP340)
    BtcSignHex {
        /// Message to sign (hex-encoded)
        #[arg(long)]
        message: String,
    },

    /// Verify a BIP340 Schnorr signature
    BtcVerify {
        /// Signature hex (64 bytes / 128 hex chars)
        #[arg(long)]
        signature: String,

        /// Public key hex (32 bytes / 64 hex chars, x-only)
        #[arg(long)]
        public_key: String,

        /// Message that was signed (UTF-8 string)
        #[arg(long)]
        message: String,
    },

    /// Verify a BIP340 Schnorr signature with hex-encoded message
    BtcVerifyHex {
        /// Signature hex (64 bytes / 128 hex chars)
        #[arg(long)]
        signature: String,

        /// Public key hex (32 bytes / 64 hex chars, x-only)
        #[arg(long)]
        public_key: String,

        /// Message that was signed (hex-encoded)
        #[arg(long)]
        message: String,
    },

    /// Sign a Bitcoin Taproot sighash
    BtcSignTaproot {
        /// Transaction sighash (32 bytes / 64 hex chars)
        #[arg(long)]
        sighash: String,
    },

    /// Get Bitcoin Taproot address (mainnet)
    BtcAddress,

    /// Get Bitcoin Taproot address (testnet)
    BtcAddressTestnet,

    /// Get Bitcoin Taproot address (signet)
    BtcAddressSignet,

    /// Get DKG group Taproot address (testnet). Without --name, lists all wallets.
    DkgAddress {
        /// Wallet/session name (optional - lists wallets if not provided)
        #[arg(long)]
        name: Option<String>,
    },

    /// Check DKG group balance (testnet). Without --name, lists all wallets.
    DkgBalance {
        /// Wallet/session name (optional - lists wallets if not provided)
        #[arg(long)]
        name: Option<String>,
    },

    /// List all DKG wallets
    DkgList,

    /// Regenerate group_info.json for a wallet
    DkgInfo {
        /// Wallet/session name
        #[arg(long)]
        name: String,
    },

    // ========================================================================
    // HD Key Derivation (BIP-32/BIP-44) Commands
    // ========================================================================
    /// Derive address at BIP-44 path (m/44'/0'/0'/change/index)
    DkgDeriveAddress {
        /// Wallet name
        #[arg(long)]
        name: String,

        /// Change level (0=external/receive, 1=internal/change)
        #[arg(long, default_value = "0")]
        change: u32,

        /// Address index
        #[arg(long, default_value = "0")]
        index: u32,

        /// Network (testnet, mainnet, signet)
        #[arg(long, default_value = "testnet")]
        network: String,
    },

    /// List multiple derived addresses
    DkgListAddresses {
        /// Wallet name
        #[arg(long)]
        name: String,

        /// Number of addresses to derive
        #[arg(long, default_value = "10")]
        count: u32,

        /// Network (testnet, mainnet, signet)
        #[arg(long, default_value = "testnet")]
        network: String,
    },

    /// Generate BIP-39 mnemonic backup for share
    DkgGenerateMnemonic {
        /// Wallet name
        #[arg(long)]
        name: String,
    },

    /// Reshare Round 1: Old party generates sub-shares for new parties
    ReshareRound1 {
        /// Source wallet name (existing wallet to reshare from)
        #[arg(long)]
        source: String,

        /// New threshold for reshared wallet
        #[arg(long)]
        new_threshold: u32,

        /// New total number of parties
        #[arg(long)]
        new_n_parties: u32,

        /// Your old party index
        #[arg(long)]
        my_index: u32,
    },

    /// Reshare Finalize: New party combines sub-shares
    ReshareFinalize {
        /// Source wallet name
        #[arg(long)]
        source: String,

        /// Target wallet name (new wallet to create)
        #[arg(long)]
        target: String,

        /// Your new party index
        #[arg(long)]
        my_index: u32,

        /// Your HTSS rank (0 = highest)
        #[arg(long, default_value = "0")]
        rank: u32,

        /// Enable hierarchical mode
        #[arg(long, default_value = "false")]
        hierarchical: bool,

        /// JSON with round1 outputs from old parties
        #[arg(long)]
        data: String,
    },

    /// Recovery Round 1: Helper party generates sub-share for lost party
    RecoverRound1 {
        /// Wallet name
        #[arg(long)]
        name: String,

        /// Index of the party who lost their share
        #[arg(long)]
        lost_index: u32,
    },

    /// Recovery Finalize: Lost party combines sub-shares to recover
    RecoverFinalize {
        /// Source wallet name (the wallet to recover into)
        #[arg(long)]
        source: String,

        /// Target wallet name (new wallet file to create)
        #[arg(long)]
        target: String,

        /// Your party index (the one being recovered)
        #[arg(long)]
        my_index: u32,

        /// Your HTSS rank (0 = highest)
        #[arg(long, default_value = "0")]
        rank: u32,

        /// Enable hierarchical mode
        #[arg(long, default_value = "false")]
        hierarchical: bool,

        /// JSON with round1 outputs from helper parties
        #[arg(long)]
        data: String,

        /// Force overwrite if target wallet exists
        #[arg(long, default_value = "false")]
        force: bool,
    },

    /// Interactive Terminal UI for wallet management
    Tui,

    /// Check Bitcoin balance (testnet)
    BtcBalance,

    /// Send Bitcoin on testnet
    BtcSend {
        /// Recipient address
        #[arg(long)]
        to: String,

        /// Amount in satoshis
        #[arg(long)]
        amount: u64,

        /// Fee rate in sats/vbyte (optional, defaults to recommended)
        #[arg(long)]
        fee_rate: Option<u64>,
    },

    /// Send Bitcoin on signet
    BtcSendSignet {
        /// Recipient address
        #[arg(long)]
        to: String,

        /// Amount in satoshis
        #[arg(long)]
        amount: u64,

        /// Fee rate in sats/vbyte (optional, defaults to recommended)
        #[arg(long)]
        fee_rate: Option<u64>,
    },

    // ========================================================================
    // DKG Threshold Transaction Commands
    // ========================================================================
    /// Build unsigned transaction for DKG threshold signing
    DkgBuildTx {
        /// Wallet name
        #[arg(long)]
        name: String,

        /// Recipient address
        #[arg(long)]
        to: String,

        /// Amount in satoshis
        #[arg(long)]
        amount: u64,

        /// Fee rate in sats/vbyte (optional)
        #[arg(long)]
        fee_rate: Option<u64>,

        /// Network (testnet, signet, mainnet)
        #[arg(long, default_value = "testnet")]
        network: String,
    },

    /// Generate nonce for DKG transaction signing
    DkgNonce {
        /// Wallet name
        #[arg(long)]
        name: String,

        /// Session ID from dkg-build-tx
        #[arg(long)]
        session: String,
    },

    /// Create signature share for DKG transaction
    DkgSign {
        /// Wallet name
        #[arg(long)]
        name: String,

        /// Session ID
        #[arg(long)]
        session: String,

        /// Sighash to sign (32 bytes hex)
        #[arg(long)]
        sighash: String,

        /// JSON with nonces from all signing parties
        #[arg(long)]
        data: String,
    },

    /// Combine signature shares and broadcast transaction
    DkgBroadcast {
        /// Wallet name
        #[arg(long)]
        name: String,

        /// Session ID
        #[arg(long)]
        session: String,

        /// Unsigned transaction hex
        #[arg(long)]
        unsigned_tx: String,

        /// JSON with signature shares from all parties
        #[arg(long)]
        data: String,

        /// Network (testnet, signet, mainnet)
        #[arg(long, default_value = "testnet")]
        network: String,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::KeygenRound1 {
            name,
            threshold,
            n_parties,
            my_index,
            rank,
            hierarchical,
        } => {
            keygen::round1(&name, threshold, n_parties, my_index, rank, hierarchical)?;
        }
        Commands::KeygenRound2 { name, data } => {
            keygen::round2(&name, &data)?;
        }
        Commands::KeygenFinalize { name, data } => {
            keygen::finalize(&name, &data)?;
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

        // Bitcoin Schnorr (BIP340) commands
        Commands::BtcKeygen => {
            bitcoin_schnorr::generate_keypair()?;
        }
        Commands::BtcImportKey { secret } => {
            bitcoin_schnorr::import_key(&secret)?;
        }
        Commands::BtcPubkey => {
            bitcoin_schnorr::get_public_key()?;
        }
        Commands::BtcSign { message } => {
            bitcoin_schnorr::sign_message(&message)?;
        }
        Commands::BtcSignHex { message } => {
            bitcoin_schnorr::sign_message_hex(&message)?;
        }
        Commands::BtcVerify {
            signature,
            public_key,
            message,
        } => {
            bitcoin_schnorr::verify_signature(&signature, &public_key, &message)?;
        }
        Commands::BtcVerifyHex {
            signature,
            public_key,
            message,
        } => {
            bitcoin_schnorr::verify_signature_hex(&signature, &public_key, &message)?;
        }
        Commands::BtcSignTaproot { sighash } => {
            bitcoin_schnorr::sign_taproot_sighash(&sighash)?;
        }
        Commands::BtcAddress => {
            bitcoin_schnorr::get_address_mainnet()?;
        }
        Commands::BtcAddressTestnet => {
            bitcoin_schnorr::get_address_testnet()?;
        }
        Commands::BtcAddressSignet => {
            bitcoin_schnorr::get_address_signet()?;
        }
        Commands::DkgAddress { name } => match name {
            Some(n) => bitcoin_schnorr::get_dkg_address_testnet(&n)?,
            None => keygen::print_wallet_list()?,
        },
        Commands::DkgBalance { name } => match name {
            Some(n) => bitcoin_tx::check_dkg_balance_testnet(&n)?,
            None => keygen::print_wallet_list()?,
        },
        Commands::DkgList => {
            keygen::print_wallet_list()?;
        }
        Commands::DkgInfo { name } => {
            keygen::regenerate_group_info(&name)?;
        }

        // HD Key Derivation commands
        Commands::DkgDeriveAddress {
            name,
            change,
            index,
            network,
        } => {
            use frostdao::btc::hd_address;
            use frostdao::storage::FileStorage;

            let state_dir = keygen::get_state_dir(&name);
            let storage = FileStorage::new(&state_dir)?;
            let result = hd_address::derive_address_core(change, index, &network, &storage)?;
            println!("{}", result.output);
        }
        Commands::DkgListAddresses {
            name,
            count,
            network,
        } => {
            use frostdao::btc::hd_address;
            use frostdao::storage::FileStorage;

            let state_dir = keygen::get_state_dir(&name);
            let storage = FileStorage::new(&state_dir)?;
            let result = hd_address::list_addresses_core(count, &network, &storage)?;
            println!("{}", result.output);
        }
        Commands::DkgGenerateMnemonic { name } => {
            use frostdao::crypto::mnemonic;
            use frostdao::storage::FileStorage;

            let state_dir = keygen::get_state_dir(&name);
            let storage = FileStorage::new(&state_dir)?;

            // Load the secret share
            let paired_share_bytes = storage.read("paired_secret_share.bin")?;
            let paired_share: schnorr_fun::frost::PairedSecretShare<secp256kfun::marker::EvenY> =
                bincode::deserialize(&paired_share_bytes)?;

            // Get share bytes
            let share_bytes: [u8; 32] = paired_share.secret_share().share.to_bytes();

            // Generate mnemonic from share
            let mnemonic_result = mnemonic::share_to_mnemonic(&share_bytes)?;

            println!("BIP-39 Mnemonic Backup for Wallet '{}'\n", name);
            println!(
                "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
            );
            println!("WARNING: This mnemonic backs up YOUR SECRET SHARE only.");
            println!("         Recovery still requires threshold shares from other parties.\n");
            println!("{}\n", mnemonic::format_mnemonic_grid(&mnemonic_result));
            println!(
                "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
            );
            println!("\nWrite down these 24 words and store them securely!");
            println!("Never share them with anyone.");
        }

        Commands::ReshareRound1 {
            source,
            new_threshold,
            new_n_parties,
            my_index,
        } => {
            reshare::reshare_round1(&source, new_threshold, new_n_parties, my_index)?;
        }
        Commands::ReshareFinalize {
            source,
            target,
            my_index,
            rank,
            hierarchical,
            data,
        } => {
            reshare::reshare_finalize(&source, &target, my_index, rank, hierarchical, &data)?;
        }
        Commands::RecoverRound1 { name, lost_index } => {
            recovery::recover_round1(&name, lost_index)?;
        }
        Commands::RecoverFinalize {
            source,
            target,
            my_index,
            rank,
            hierarchical,
            data,
            force,
        } => {
            recovery::recover_finalize(
                &source,
                &target,
                my_index,
                rank,
                hierarchical,
                &data,
                force,
            )?;
        }
        Commands::Tui => {
            tui::run_tui()?;
        }
        Commands::BtcBalance => {
            bitcoin_tx::check_balance_testnet()?;
        }
        Commands::BtcSend {
            to,
            amount,
            fee_rate,
        } => {
            bitcoin_tx::send_testnet(&to, amount, fee_rate)?;
        }
        Commands::BtcSendSignet {
            to,
            amount,
            fee_rate,
        } => {
            bitcoin_tx::send_signet(&to, amount, fee_rate)?;
        }

        // DKG Threshold Transaction commands
        Commands::DkgBuildTx {
            name,
            to,
            amount,
            fee_rate,
            network,
        } => {
            let net = match network.as_str() {
                "mainnet" => bitcoin::Network::Bitcoin,
                "signet" => bitcoin::Network::Signet,
                _ => bitcoin::Network::Testnet,
            };
            dkg_tx::build_unsigned_tx(&name, &to, amount, fee_rate, net)?;
        }
        Commands::DkgNonce { name, session } => {
            dkg_tx::dkg_generate_nonce(&name, &session)?;
        }
        Commands::DkgSign {
            name,
            session,
            sighash,
            data,
        } => {
            dkg_tx::dkg_sign(&name, &session, &sighash, &data)?;
        }
        Commands::DkgBroadcast {
            name,
            session,
            unsigned_tx,
            data,
            network,
        } => {
            let net = match network.as_str() {
                "mainnet" => bitcoin::Network::Bitcoin,
                "signet" => bitcoin::Network::Signet,
                _ => bitcoin::Network::Testnet,
            };
            dkg_tx::dkg_broadcast(&name, &session, &unsigned_tx, &data, net)?;
        }
    }

    Ok(())
}
