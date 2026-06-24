use anyhow::{Context, Result};
use clap::{Parser, Subcommand};

// Use library crate for core functionality
use frostdao::btc::{schnorr as bitcoin_schnorr, transaction as bitcoin_tx};
use frostdao::protocol::{dkg_tx, keygen, recovery, reshare, signing};
use frostdao::storage::Storage; // For HD commands

// TUI is CLI-only, not part of lib
mod tui;

const MAINNET_BITCOIN_ENV: &str = "FROSTDAO_ENABLE_MAINNET_BITCOIN";

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

        /// JSON with all commitments from round 1
        #[arg(long)]
        data: String,

        /// Enable NIP-44 E2E encryption for shares
        #[arg(long, default_value = "false")]
        encrypt: bool,
    },

    /// Finalize keygen: Validate and combine shares
    KeygenFinalize {
        /// Wallet/session name (must match round1)
        #[arg(long)]
        name: String,

        /// JSON with all shares sent to you
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

        /// JSON with nonces and group key
        #[arg(long)]
        data: String,
    },

    /// Combine signature shares into final signature
    Combine {
        /// JSON with all signature shares, including message
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

    /// Compile and inspect a Taproot Miniscript policy
    #[cfg(feature = "miniscript-policy")]
    PolicyCompile {
        /// Miniscript policy string, for example: thresh(2,pk(A),pk(B),pk(C))
        #[arg(long)]
        policy: String,

        /// Internal key label used for descriptor compilation
        #[arg(long)]
        internal_key: Option<String>,
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
    // HD Key Derivation (BIP-32/BIP-86) Commands (Taproot)
    // ========================================================================
    /// Derive address at BIP-86 path (m/86'/0'/0'/change/index)
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

        /// Network (testnet, testnet4, signet, regtest, mainnet)
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

        /// Network (testnet, testnet4, signet, regtest, mainnet)
        #[arg(long, default_value = "testnet")]
        network: String,
    },

    /// Generate BIP-39 mnemonic backup for share
    DkgGenerateMnemonic {
        /// Wallet name
        #[arg(long)]
        name: String,
    },

    /// Verify a BIP-39 share mnemonic against the local wallet backup manifest
    DkgVerifyMnemonic {
        /// Wallet name
        #[arg(long)]
        name: String,

        /// 24-word mnemonic in quotes
        #[arg(long)]
        words: String,
    },

    /// Restore local wallet files from a verified share mnemonic and public backup manifest
    DkgRestoreMnemonic {
        /// Wallet name; must match the manifest wallet name
        #[arg(long)]
        name: String,

        /// 24-word mnemonic in quotes
        #[arg(long)]
        words: String,

        /// Path to the public backup manifest JSON
        #[arg(long)]
        manifest: String,
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

    /// Local reshare: refresh all shares at once (when you have t parties locally)
    ReshareLocal {
        /// Source wallet name
        #[arg(long)]
        source: String,

        /// Target wallet name (new wallet to create)
        #[arg(long)]
        target: String,

        /// New threshold (optional, defaults to current)
        #[arg(long)]
        new_threshold: Option<u32>,

        /// New total parties (optional, defaults to current)
        #[arg(long)]
        new_n_parties: Option<u32>,

        /// Enable hierarchical mode
        #[arg(long, default_value = "false")]
        hierarchical: bool,
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

        /// Network (testnet, testnet4, signet, regtest, mainnet)
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

        /// Network (testnet, testnet4, signet, regtest, mainnet)
        #[arg(long, default_value = "testnet")]
        network: String,
    },
}

fn load_backup_inputs(
    name: &str,
) -> Result<(
    frostdao::crypto::backup::BackupWalletMetadata,
    [u8; 32],
    frostdao::protocol::keygen::GroupInfo,
)> {
    use frostdao::storage::FileStorage;

    let state_dir = keygen::get_state_dir(name);
    let storage = FileStorage::new(&state_dir)?;

    let paired_share_bytes = storage
        .read("paired_secret_share.bin")
        .with_context(|| format!("wallet '{}' has no local secret share", name))?;
    let paired_share: schnorr_fun::frost::PairedSecretShare<secp256kfun::marker::EvenY> =
        bincode::deserialize(&paired_share_bytes)?;
    let share_bytes: [u8; 32] = paired_share.secret_share().share.to_bytes();

    let htss_json = String::from_utf8(storage.read("htss_metadata.json")?)
        .context("htss_metadata.json is not valid UTF-8")?;
    let htss: frostdao::protocol::keygen::HtssMetadata =
        serde_json::from_str(&htss_json).context("failed to parse htss_metadata.json")?;

    let shared_key_bytes = storage
        .read("shared_key.bin")
        .with_context(|| format!("wallet '{}' has no shared_key.bin", name))?;
    let shared_key: schnorr_fun::frost::SharedKey<secp256kfun::marker::EvenY> =
        bincode::deserialize(&shared_key_bytes).context("failed to parse shared_key.bin")?;

    let group_info_json =
        String::from_utf8(storage.read("group_info.json")?).with_context(|| {
            format!(
                "wallet '{}' has no group_info.json; run `frostdao dkg-info --name {}` first",
                name, name
            )
        })?;
    let group_info: frostdao::protocol::keygen::GroupInfo =
        serde_json::from_str(&group_info_json).context("failed to parse group_info.json")?;

    let metadata = frostdao::crypto::backup::BackupWalletMetadata {
        wallet_name: group_info.name.clone(),
        party_index: htss.my_index,
        rank: htss.my_rank,
        threshold: htss.threshold,
        total_parties: group_info.total_parties,
        hierarchical: htss.hierarchical,
        party_ranks: htss.party_ranks.clone(),
        group_public_key: group_info.group_public_key.clone(),
        shared_key_polynomial: hex::encode(shared_key.to_bytes()),
        taproot_address_testnet: group_info.taproot_address_testnet.clone(),
        taproot_address_mainnet: group_info.taproot_address_mainnet.clone(),
    };

    Ok((metadata, share_bytes, group_info))
}

fn restore_wallet_from_mnemonic_manifest(
    name: &str,
    words: &str,
    manifest_path: &str,
) -> Result<()> {
    use bitcoin::{Address, Network, XOnlyPublicKey};
    use frostdao::crypto::{backup, mnemonic};
    use frostdao::protocol::keygen::{GroupInfo, HdMetadata, HtssMetadata, PartyInfo};
    use frostdao::storage::FileStorage;
    use schnorr_fun::frost::{SecretShare, SharedKey};
    use secp256kfun::prelude::*;

    let manifest_json = std::fs::read_to_string(manifest_path)
        .with_context(|| format!("failed to read backup manifest '{}'", manifest_path))?;
    let manifest: backup::BackupManifest =
        serde_json::from_str(&manifest_json).context("failed to parse backup manifest JSON")?;
    manifest.validate()?;

    if manifest.wallet_name != name {
        anyhow::bail!(
            "manifest wallet name '{}' does not match requested wallet '{}'",
            manifest.wallet_name,
            name
        );
    }

    let parsed = mnemonic::parse_mnemonic(words)?;
    let share_bytes = mnemonic::mnemonic_to_share(&parsed)?;
    backup::verify_share_against_manifest(&share_bytes, &manifest)?;

    let state_dir = keygen::get_state_dir(name);
    let state_path = std::path::Path::new(&state_dir);
    if state_path.exists() {
        anyhow::bail!(
            "wallet '{}' already exists at {}; refusing to overwrite",
            name,
            state_dir
        );
    }

    let shared_key_polynomial = hex::decode(&manifest.shared_key_polynomial)
        .context("shared key polynomial in manifest is not valid hex")?;
    let shared_key: SharedKey<EvenY> = SharedKey::from_slice(&shared_key_polynomial)
        .ok_or_else(|| anyhow::anyhow!("manifest shared key polynomial is invalid"))?;

    let share_scalar = Scalar::<Secret, Zero>::from_bytes(share_bytes)
        .ok_or_else(|| anyhow::anyhow!("mnemonic share is not a valid secp256k1 scalar"))?;
    let share_index = Scalar::<Secret, Zero>::from(manifest.party_index)
        .public()
        .non_zero()
        .ok_or_else(|| anyhow::anyhow!("manifest party index cannot be zero"))?;
    let secret_share = SecretShare {
        index: share_index,
        share: share_scalar,
    };
    let paired_share = shared_key
        .pair_secret_share(secret_share)
        .ok_or_else(|| anyhow::anyhow!("mnemonic share does not match public FROST polynomial"))?;

    let pubkey_bytes = hex::decode(&manifest.group_public_key)
        .context("group public key in manifest is not valid hex")?;
    let xonly_pk = XOnlyPublicKey::from_slice(&pubkey_bytes)
        .context("group public key in manifest is not a valid x-only key")?;
    let secp = bitcoin::secp256k1::Secp256k1::new();
    let address_testnet = Address::p2tr(&secp, xonly_pk, None, Network::Testnet).to_string();
    let address_mainnet = Address::p2tr(&secp, xonly_pk, None, Network::Bitcoin).to_string();
    if address_testnet != manifest.taproot_address_testnet {
        anyhow::bail!("manifest testnet address does not match group public key");
    }
    if address_mainnet != manifest.taproot_address_mainnet {
        anyhow::bail!("manifest mainnet address does not match group public key");
    }

    let storage = FileStorage::new(&state_dir)?;
    storage.write(
        "paired_secret_share.bin",
        &bincode::serialize(&paired_share)?,
    )?;
    storage.write("shared_key.bin", &bincode::serialize(&shared_key)?)?;

    let htss = HtssMetadata {
        my_index: manifest.party_index,
        my_rank: manifest.rank,
        threshold: manifest.threshold,
        hierarchical: manifest.hierarchical,
        party_ranks: manifest.party_ranks.clone(),
        signing_requirement: None,
    };
    storage.write(
        "htss_metadata.json",
        serde_json::to_string_pretty(&htss)?.as_bytes(),
    )?;

    let chain_code =
        frostdao::crypto::helpers::tagged_hash("FrostDAO/ChainCode", &xonly_pk.serialize());
    let hd_metadata = HdMetadata {
        chain_code: hex::encode(chain_code),
        hd_enabled: true,
        mnemonic_hint: None,
        derived_count: 10,
    };
    storage.write(
        "hd_metadata.json",
        serde_json::to_string_pretty(&hd_metadata)?.as_bytes(),
    )?;

    let parties = manifest
        .party_ranks
        .iter()
        .map(|(index, rank)| PartyInfo {
            index: *index,
            rank: *rank,
            verification_share: "unavailable".to_string(),
        })
        .collect();
    let group_info = GroupInfo {
        name: name.to_string(),
        group_public_key: manifest.group_public_key.clone(),
        taproot_address_testnet: manifest.taproot_address_testnet.clone(),
        taproot_address_mainnet: manifest.taproot_address_mainnet.clone(),
        threshold: manifest.threshold,
        total_parties: manifest.total_parties,
        hierarchical: manifest.hierarchical,
        parties,
    };
    storage.write(
        "group_info.json",
        serde_json::to_string_pretty(&group_info)?.as_bytes(),
    )?;

    println!("Restored wallet '{}' from mnemonic backup.", name);
    println!("Backup ID: {}", manifest.backup_id);
    println!("Party: {}", manifest.party_index);
    println!("Rank: {}", manifest.rank);
    println!(
        "Threshold: {} of {}",
        manifest.threshold, manifest.total_parties
    );
    println!("Testnet address: {}", manifest.taproot_address_testnet);
    println!("Mainnet address: {}", manifest.taproot_address_mainnet);
    println!("Restored files at: {}", state_dir);

    Ok(())
}

fn parse_dkg_network(network: &str) -> Result<bitcoin::Network> {
    match network {
        "testnet" | "testnet3" => Ok(bitcoin::Network::Testnet),
        "testnet4" | "test4" => Ok(bitcoin::Network::Testnet4),
        "signet" => Ok(bitcoin::Network::Signet),
        "regtest" | "local" => Ok(bitcoin::Network::Regtest),
        "mainnet" | "bitcoin" => Ok(bitcoin::Network::Bitcoin),
        other => anyhow::bail!(
            "unknown network '{}'; use testnet, testnet4, signet, regtest, or mainnet",
            other
        ),
    }
}

fn mainnet_bitcoin_enabled_value(value: Option<&str>) -> bool {
    matches!(value, Some("1" | "true" | "TRUE" | "yes" | "YES"))
}

fn ensure_mainnet_bitcoin_enabled(network: bitcoin::Network) -> Result<()> {
    if network == bitcoin::Network::Bitcoin
        && !mainnet_bitcoin_enabled_value(std::env::var(MAINNET_BITCOIN_ENV).ok().as_deref())
    {
        anyhow::bail!(
            "mainnet DKG transaction commands require {}=1; use testnet or signet by default",
            MAINNET_BITCOIN_ENV
        );
    }
    Ok(())
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
        Commands::KeygenRound2 {
            name,
            data,
            encrypt,
        } => {
            keygen::round2(&name, &data, encrypt)?;
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
        #[cfg(feature = "miniscript-policy")]
        Commands::PolicyCompile {
            policy,
            internal_key,
        } => {
            let result = frostdao::btc::miniscript_policy::compile_taproot_policy(
                &policy,
                internal_key.as_deref(),
            )?;
            println!("{}", serde_json::to_string_pretty(&result)?);
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
            use frostdao::crypto::{backup, mnemonic};

            let (metadata, share_bytes, _) = load_backup_inputs(&name)?;
            let mnemonic_result = mnemonic::share_to_mnemonic(&share_bytes)?;
            let manifest = backup::build_backup_manifest(metadata, &share_bytes)?;

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
            println!("\nBackup manifest (public metadata):");
            println!("{}", serde_json::to_string_pretty(&manifest)?);
            println!("\nWrite down these 24 words and store them securely!");
            println!("Never share them with anyone.");
            println!(
                "Verify later with: frostdao dkg-verify-mnemonic --name {} --words '<24 words>'",
                name
            );
        }
        Commands::DkgVerifyMnemonic { name, words } => {
            use frostdao::crypto::{backup, mnemonic};

            let (metadata, local_share_bytes, _) = load_backup_inputs(&name)?;
            let manifest = backup::build_backup_manifest(metadata, &local_share_bytes)?;
            let parsed = mnemonic::parse_mnemonic(&words)?;
            let mnemonic_share = mnemonic::mnemonic_to_share(&parsed)?;
            backup::verify_share_against_manifest(&mnemonic_share, &manifest)?;

            println!("Backup mnemonic verified for wallet '{}'.", name);
            println!("Backup ID: {}", manifest.backup_id);
            println!("Party: {}", manifest.party_index);
            println!("Rank: {}", manifest.rank);
            println!(
                "Threshold: {} of {}",
                manifest.threshold, manifest.total_parties
            );
        }
        Commands::DkgRestoreMnemonic {
            name,
            words,
            manifest,
        } => {
            restore_wallet_from_mnemonic_manifest(&name, &words, &manifest)?;
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
        Commands::ReshareLocal {
            source,
            target,
            new_threshold,
            new_n_parties,
            hierarchical,
        } => {
            reshare::reshare_local(&source, &target, new_threshold, new_n_parties, hierarchical)?;
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
            let net = parse_dkg_network(&network)?;
            ensure_mainnet_bitcoin_enabled(net)?;
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
            let net = parse_dkg_network(&network)?;
            ensure_mainnet_bitcoin_enabled(net)?;
            dkg_tx::dkg_broadcast(&name, &session, &unsigned_tx, &data, net)?;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dkg_network_parser_is_explicit() {
        assert_eq!(
            parse_dkg_network("testnet").unwrap(),
            bitcoin::Network::Testnet
        );
        assert_eq!(
            parse_dkg_network("testnet3").unwrap(),
            bitcoin::Network::Testnet
        );
        assert_eq!(
            parse_dkg_network("testnet4").unwrap(),
            bitcoin::Network::Testnet4
        );
        assert_eq!(
            parse_dkg_network("test4").unwrap(),
            bitcoin::Network::Testnet4
        );
        assert_eq!(
            parse_dkg_network("signet").unwrap(),
            bitcoin::Network::Signet
        );
        assert_eq!(
            parse_dkg_network("regtest").unwrap(),
            bitcoin::Network::Regtest
        );
        assert_eq!(
            parse_dkg_network("local").unwrap(),
            bitcoin::Network::Regtest
        );
        assert_eq!(
            parse_dkg_network("mainnet").unwrap(),
            bitcoin::Network::Bitcoin
        );
        assert!(parse_dkg_network("typo-net").is_err());
    }

    #[test]
    fn mainnet_bitcoin_opt_in_is_deliberate() {
        assert!(!mainnet_bitcoin_enabled_value(None));
        assert!(!mainnet_bitcoin_enabled_value(Some("0")));
        assert!(!mainnet_bitcoin_enabled_value(Some("false")));
        assert!(mainnet_bitcoin_enabled_value(Some("1")));
        assert!(mainnet_bitcoin_enabled_value(Some("true")));
        assert!(mainnet_bitcoin_enabled_value(Some("yes")));
    }
}
