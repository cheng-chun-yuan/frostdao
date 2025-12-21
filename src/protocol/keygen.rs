use crate::storage::{FileStorage, Storage};
use crate::CommandResult;
use anyhow::{Context, Result};
use schnorr_fun::frost::{
    self,
    chilldkg::simplepedpop::{self, *},
};
use secp256kfun::prelude::*;
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use std::collections::{BTreeMap, BTreeSet};

/// Parse space-separated JSON objects into a Vec
/// Handles compact JSON where objects are separated by spaces
pub fn parse_space_separated_json<T>(data: &str) -> Result<Vec<T>>
where
    T: for<'de> Deserialize<'de>,
{
    let mut objects = Vec::new();
    let mut current_obj = String::new();
    let mut brace_depth = 0;
    let mut in_string = false;
    let mut escape_next = false;

    for ch in data.chars() {
        if escape_next {
            current_obj.push(ch);
            escape_next = false;
            continue;
        }

        match ch {
            '\\' if in_string => {
                escape_next = true;
                current_obj.push(ch);
            }
            '"' => {
                in_string = !in_string;
                current_obj.push(ch);
            }
            '{' if !in_string => {
                brace_depth += 1;
                current_obj.push(ch);
            }
            '}' if !in_string => {
                brace_depth -= 1;
                current_obj.push(ch);

                // Complete object found
                if brace_depth == 0 && !current_obj.trim().is_empty() {
                    let obj: T = serde_json::from_str(current_obj.trim()).context(format!(
                        "Failed to parse JSON object: {}",
                        current_obj.trim()
                    ))?;
                    objects.push(obj);
                    current_obj.clear();
                }
            }
            ' ' | '\t' | '\n' | '\r' if !in_string && brace_depth == 0 => {
                // Skip whitespace between objects
                continue;
            }
            _ => {
                current_obj.push(ch);
            }
        }
    }

    if brace_depth != 0 {
        anyhow::bail!("Unbalanced braces in JSON input");
    }

    if !current_obj.trim().is_empty() {
        anyhow::bail!("Incomplete JSON object at end of input");
    }

    Ok(objects)
}

// JSON structures for copy-paste interface

#[derive(Serialize, Deserialize, Debug)]
pub struct Round1Output {
    pub party_index: u32,
    #[serde(default)]
    pub rank: u32, // HTSS rank (0 = highest authority)
    pub keygen_input: String, // Bincode hex
    #[serde(default)]
    pub hierarchical: bool, // Whether HTSS mode is enabled
    #[serde(rename = "type")]
    pub event_type: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Round1Input {
    pub commitments: Vec<CommitmentData>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct CommitmentData {
    pub index: u32,
    pub data: String, // Bincode hex of KeygenInput
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Round2Output {
    pub party_index: u32,
    pub shares: Vec<ShareData>,
    #[serde(rename = "type")]
    pub event_type: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ShareData {
    pub to_index: u32,
    pub share: String, // Bincode hex of secret scalar
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Round2Input {
    pub shares_for_me: Vec<IncomingShare>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct IncomingShare {
    pub from_index: u32,
    pub share: String,
}

// Internal state
#[derive(Serialize, Deserialize)]
struct Round1State {
    my_index: u32,
    my_rank: u32, // HTSS rank (0 = highest authority)
    threshold: u32,
    n_parties: u32,
    hierarchical: bool, // Whether HTSS mode is enabled
    contributor: Contributor,
    share_indices: Vec<String>, // Hex encoded ShareIndex scalars
}

/// HTSS metadata stored after keygen finalize
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct HtssMetadata {
    pub my_index: u32,
    pub my_rank: u32,
    pub threshold: u32,
    pub hierarchical: bool,
    /// Map of party_index -> rank for all participants
    pub party_ranks: std::collections::BTreeMap<u32, u32>,
}

/// Party info for group_info.json
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PartyInfo {
    pub index: u32,
    pub rank: u32,
    pub verification_share: String,
}

/// Group info stored after DKG finalize (shareable public info)
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct GroupInfo {
    pub name: String,
    pub group_public_key: String,
    pub taproot_address_testnet: String,
    pub taproot_address_mainnet: String,
    pub threshold: u32,
    pub total_parties: u32,
    pub hierarchical: bool,
    /// Parties sorted by rank (ascending)
    pub parties: Vec<PartyInfo>,
}

/// Helper to get the state directory path for a given wallet name
pub fn get_state_dir(name: &str) -> String {
    format!(".frost_state/{}", name)
}

/// List all available DKG wallets
pub fn list_wallets() -> Result<Vec<WalletSummary>> {
    let base_dir = std::path::Path::new(".frost_state");

    if !base_dir.exists() {
        return Ok(Vec::new());
    }

    let mut wallets = Vec::new();

    for entry in std::fs::read_dir(base_dir)? {
        let entry = entry?;
        let path = entry.path();

        // Skip if not a directory
        if !path.is_dir() {
            continue;
        }

        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string();

        // Check if it's a valid wallet (has shared_key.bin)
        let shared_key_path = path.join("shared_key.bin");
        if !shared_key_path.exists() {
            continue;
        }

        // Try to load group_info.json for more details
        let group_info_path = path.join("group_info.json");
        let (threshold, total_parties, hierarchical, address) = if group_info_path.exists() {
            if let Ok(content) = std::fs::read_to_string(&group_info_path) {
                if let Ok(info) = serde_json::from_str::<GroupInfo>(&content) {
                    (
                        Some(info.threshold),
                        Some(info.total_parties),
                        Some(info.hierarchical),
                        Some(info.taproot_address_testnet),
                    )
                } else {
                    (None, None, None, None)
                }
            } else {
                (None, None, None, None)
            }
        } else {
            // Try to load from htss_metadata.json
            let htss_path = path.join("htss_metadata.json");
            if htss_path.exists() {
                if let Ok(content) = std::fs::read_to_string(&htss_path) {
                    if let Ok(htss) = serde_json::from_str::<HtssMetadata>(&content) {
                        (
                            Some(htss.threshold),
                            Some(htss.party_ranks.len() as u32),
                            Some(htss.hierarchical),
                            None,
                        )
                    } else {
                        (None, None, None, None)
                    }
                } else {
                    (None, None, None, None)
                }
            } else {
                (None, None, None, None)
            }
        };

        wallets.push(WalletSummary {
            name,
            threshold,
            total_parties,
            hierarchical,
            address,
        });
    }

    // Sort by name
    wallets.sort_by(|a, b| a.name.cmp(&b.name));

    Ok(wallets)
}

/// Summary info for a wallet
#[derive(Debug, Clone)]
pub struct WalletSummary {
    pub name: String,
    pub threshold: Option<u32>,
    pub total_parties: Option<u32>,
    pub hierarchical: Option<bool>,
    pub address: Option<String>,
}

/// Print wallet list to console
pub fn print_wallet_list() -> Result<()> {
    let wallets = list_wallets()?;

    if wallets.is_empty() {
        println!("No DKG wallets found.\n");
        println!("Create one with:");
        println!("  frostdao keygen-round1 --name <wallet_name> --threshold <t> --n-parties <n> --my-index <i>");
        return Ok(());
    }

    println!("DKG Wallets\n");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");

    for wallet in &wallets {
        let mode = match wallet.hierarchical {
            Some(true) => "HTSS",
            Some(false) => "TSS",
            None => "?",
        };

        let threshold_str = match (wallet.threshold, wallet.total_parties) {
            (Some(t), Some(n)) => format!("{}-of-{}", t, n),
            _ => "?".to_string(),
        };

        println!("  {} ({} {})", wallet.name, threshold_str, mode);

        if let Some(addr) = &wallet.address {
            let short_addr = if addr.len() > 20 {
                format!("{}...{}", &addr[..10], &addr[addr.len() - 8..])
            } else {
                addr.clone()
            };
            println!("    Address: {}", short_addr);
        }
    }

    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("\nUse --name <wallet_name> to select a wallet:");
    println!(
        "  frostdao dkg-address --name {}",
        wallets.first().map(|w| w.name.as_str()).unwrap_or("<name>")
    );
    println!(
        "  frostdao dkg-balance --name {}",
        wallets.first().map(|w| w.name.as_str()).unwrap_or("<name>")
    );

    Ok(())
}

pub fn round1_core(
    threshold: u32,
    n_parties: u32,
    my_index: u32,
    my_rank: u32,       // HTSS rank (0 = highest authority)
    hierarchical: bool, // Whether HTSS mode is enabled
    storage: &dyn Storage,
) -> Result<CommandResult> {
    let mut out = String::new();

    let mode_name = if hierarchical { "HTSS" } else { "TSS" };
    out.push_str(&format!("FROST Keygen ({}) - Round 1\n\n", mode_name));
    out.push_str("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");
    out.push_str("Configuration:\n");
    out.push_str(&format!(
        "  Threshold: {} (need {} parties to sign)\n",
        threshold, threshold
    ));
    out.push_str(&format!("  Total parties: {}\n", n_parties));
    out.push_str(&format!("  Your index: {}\n", my_index));
    if hierarchical {
        out.push_str(&format!("  Your rank: {} (HTSS mode)\n", my_rank));
        out.push_str("  Note: Rank 0 = highest authority, higher ranks = lower authority\n");
    }
    out.push_str("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n\n");

    if threshold > n_parties {
        anyhow::bail!("Threshold cannot exceed number of parties");
    }
    if my_index == 0 || my_index > n_parties {
        anyhow::bail!("Party index must be between 1 and {}", n_parties);
    }

    // Create the FROST instance
    let frost = frost::new_with_deterministic_nonces::<Sha256>();

    // Create share indices for all parties (1-based indices)
    let share_indices: BTreeSet<_> = (1..=n_parties)
        .map(|i| Scalar::from(i).non_zero().expect("nonzero"))
        .collect();

    out.push_str("⚙️  Using schnorr_fun's FROST implementation\n");
    out.push_str("   Calling: Contributor::gen_keygen_input()\n\n");

    out.push_str("⚙️  Generating random polynomial...\n");
    out.push_str(&format!(
        "   Degree: t-1 = {} (for threshold {})\n",
        threshold - 1,
        threshold
    ));
    out.push_str("   The polynomial f(x) = a0 + a1*x + a2*x² + ...\n");
    out.push_str("   where a0 is your secret contribution\n\n");

    // Generate keygen input as a contributor
    let mut rng = rand::thread_rng();
    let (contributor, keygen_input, secret_shares) = Contributor::gen_keygen_input(
        &frost.schnorr,
        threshold,
        &share_indices,
        my_index - 1, // Contributor uses 0-based indexing
        &mut rng,
    );

    out.push_str("❄️  Generated:\n");
    out.push_str(&format!(
        "   - {} polynomial commitments (public points)\n",
        threshold
    ));
    out.push_str("   - Proof of Possession (PoP) signature\n");
    out.push_str(&format!(
        "   - {} secret shares (one for each party)\n\n",
        n_parties
    ));

    out.push_str("🧠 What just happened:\n");
    out.push_str(&format!(
        "   1. Generated {} random polynomial coefficients [a₀, a₁, ..., a_{}]\n",
        threshold,
        threshold - 1
    ));
    out.push_str("      • a₀ is your SECRET contribution to the group key\n");
    out.push_str("      • a₁, a₂, ... are random coefficients\n\n");
    out.push_str(&format!(
        "   2. Created {} commitments: [a₀*G, a₁*G, ..., a_{}*G]\n",
        threshold,
        threshold - 1
    ));
    out.push_str("      • These prove the polynomial without revealing it (safe to share!)\n");
    out.push_str("      • Everyone combines a₀*G values to get the shared public key\n\n");
    out.push_str(&format!(
        "   3. Evaluated polynomial at {} indices to create secret shares\n",
        n_parties
    ));
    out.push_str("      • Party i receives: f(i) = a₀ + a₁*i + a₂*i² + ...\n");
    out.push_str("      • Each share is a point on your polynomial\n\n");
    out.push_str("   4. Created Proof-of-Possession (PoP) signature\n");
    out.push_str("      • This proves you know a₀ (your secret contribution)\n");
    out.push_str("      • Prevents rogue-key and key-cancellation attacks\n\n");
    out.push_str("❓ Think about it:\n");
    out.push_str("   Why is it important to verify Proofs-of-Possession?\n");
    out.push_str("   What could an attacker do if they could contribute a₀*G\n");
    out.push_str("   without proving they know a₀?\n\n");

    // Serialize for output
    let keygen_input_bytes = bincode::serialize(&keygen_input)?;
    let keygen_input_hex = hex::encode(&keygen_input_bytes);

    // Save state for round 2
    let state = Round1State {
        my_index,
        my_rank,
        threshold,
        n_parties,
        hierarchical,
        contributor,
        share_indices: share_indices
            .iter()
            .map(|s| hex::encode(s.to_bytes()))
            .collect(),
    };
    storage.write(
        "round1_state.json",
        serde_json::to_string_pretty(&state)?.as_bytes(),
    )?;

    // Save keygen shares for round 2
    let shares_map: BTreeMap<String, String> = secret_shares
        .into_iter()
        .map(|(idx, share)| (hex::encode(idx.to_bytes()), hex::encode(share.to_bytes())))
        .collect();
    storage.write(
        "my_secret_shares.json",
        serde_json::to_string_pretty(&shares_map)?.as_bytes(),
    )?;

    out.push_str("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");
    out.push_str("✉️  Your commitment generated!\n\n");

    out.push_str("➜ Paste the result JSON into the webpage\n");
    out.push_str(&format!(
        "➜ Wait for all {} parties to post their commitments\n",
        n_parties
    ));
    out.push_str("➜ Copy the \"all commitments\" JSON from webpage\n");
    out.push_str("➜ Run: yushan keygen-round2 --data '<JSON>'\n");

    // Create JSON result for copy-pasting
    let output = Round1Output {
        party_index: my_index,
        rank: my_rank,
        keygen_input: keygen_input_hex,
        hierarchical,
        event_type: "keygen_round1".to_string(),
    };
    let result = serde_json::to_string(&output)?;

    Ok(CommandResult {
        output: out,
        result,
    })
}

pub fn round1(
    name: &str,
    threshold: u32,
    n_parties: u32,
    my_index: u32,
    my_rank: u32,
    hierarchical: bool,
) -> Result<()> {
    let state_dir = get_state_dir(name);
    let path = std::path::Path::new(&state_dir);

    // Check if folder exists and prompt for confirmation
    if path.exists() {
        println!("⚠️  Wallet '{}' already exists at {}", name, state_dir);
        println!("   This will OVERWRITE your existing keys!");
        print!("   Replace? [y/N]: ");
        std::io::Write::flush(&mut std::io::stdout())?;

        let mut input = String::new();
        std::io::stdin().read_line(&mut input)?;
        let input = input.trim().to_lowercase();

        if input != "y" && input != "yes" {
            println!("Aborted. Your existing wallet is safe.");
            return Ok(());
        }

        // Remove existing folder
        std::fs::remove_dir_all(path)?;
        println!("   Removed existing wallet.\n");
    }

    let storage = FileStorage::new(&state_dir)?;
    let cmd_result = round1_core(
        threshold,
        n_parties,
        my_index,
        my_rank,
        hierarchical,
        &storage,
    )?;
    println!("{}", cmd_result.output);
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("📋 Copy this JSON:");
    println!("{}\n", cmd_result.result);
    println!("💾 State saved to: {}/", state_dir);
    Ok(())
}

pub fn round2_core(data: &str, storage: &dyn Storage) -> Result<CommandResult> {
    let mut out = String::new();

    out.push_str("FROST Keygen - Round 2\n\n");

    // Load state
    let state_json = String::from_utf8(storage.read("round1_state.json")?)
        .context("Failed to load round 1 state. Did you run keygen-round1?")?;
    let state: Round1State = serde_json::from_str(&state_json)?;

    // Load my keygen shares (to send to other parties)
    let shares_json = String::from_utf8(storage.read("my_secret_shares.json")?)?;
    let shares_map: BTreeMap<String, String> = serde_json::from_str(&shares_json)?;

    // Parse input - space-separated Round1Output objects
    let round1_outputs: Vec<Round1Output> = parse_space_separated_json(data)?;

    // Convert to expected format
    let commitments: Vec<CommitmentData> = round1_outputs
        .into_iter()
        .map(|output| CommitmentData {
            index: output.party_index,
            data: output.keygen_input,
        })
        .collect();

    let input = Round1Input { commitments };

    out.push_str(&format!(
        " Received {} commitments from other parties\n\n",
        input.commitments.len()
    ));

    out.push_str("⚙️  Using schnorr_fun's FROST coordinator\n");
    out.push_str("   This aggregates all commitments and validates them\n\n");

    // Create FROST instance
    let frost = frost::new_with_deterministic_nonces::<Sha256>();

    // Create coordinator to aggregate inputs
    let mut coordinator = Coordinator::new(state.threshold, state.n_parties);

    out.push_str("⚙️  Adding inputs to coordinator...\n");
    for commit_data in &input.commitments {
        let keygen_input_bytes = hex::decode(&commit_data.data)?;
        let keygen_input: KeygenInput = bincode::deserialize(&keygen_input_bytes)?;

        coordinator
            .add_input(
                &frost.schnorr,
                commit_data.index - 1, // Coordinator uses 0-based indexing
                keygen_input,
            )
            .map_err(|e| anyhow::anyhow!("Failed to add input: {}", e))?;

        out.push_str(&format!(
            "    Party {}: Commitment validated\n",
            commit_data.index
        ));
    }

    out.push_str("\n❄️  All commitments valid!\n\n");

    out.push_str("✉️  Your keygen shares to send:\n");
    out.push_str("🧠 Why send keygen shares?\n");
    out.push_str(&format!(
        "   Each party evaluates their polynomial at ALL {} party indices\n",
        state.n_parties
    ));
    out.push_str("   Party i sends f_i(j) to party j\n");
    out.push_str("   These keygen shares will be combined to create each party's\n");
    out.push_str("   final secret share (without anyone knowing the full key!)\n\n");
    out.push_str("❓ Think about it:\n");
    out.push_str("   By broadcasting these keygen shares publicly on Nostr, we're\n");
    out.push_str("   making a critical security mistake! Anyone can reconstruct\n");
    out.push_str("   the full private key. What should be done instead?\n\n");

    // Create output with shares
    let mut shares = Vec::new();
    for (idx_hex, share_hex) in shares_map {
        let idx_bytes = hex::decode(&idx_hex)?;
        let idx_scalar: Scalar<Public, NonZero> = Scalar::<NonZero>::from_slice(&idx_bytes[..32])
            .expect("share index cant be zero!")
            .public();
        // Extract index value - scalars are big-endian, so small values are in last byte
        let to_index = idx_scalar.to_bytes()[31] as u32;

        out.push_str(&format!("   Share for Party {}: {}\n", to_index, share_hex));

        shares.push(ShareData {
            to_index,
            share: share_hex,
        });
    }

    out.push_str("\n━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");
    out.push_str("✉️  Your shares generated!\n\n");

    out.push_str("➜ Paste the result JSON into the webpage\n");
    out.push_str("➜ Wait for all parties to post their shares\n");
    out.push_str(&format!(
        "➜ Copy \"shares for Party {}\" JSON from webpage\n",
        state.my_index
    ));
    out.push_str("➜ Run: yushan keygen-finalize --data '<JSON>'\n");

    // Save all commitments for validation
    storage.write("all_commitments.json", data.as_bytes())?;

    // Create JSON result for copy-pasting
    let output = Round2Output {
        party_index: state.my_index,
        shares,
        event_type: "keygen_round2".to_string(),
    };
    let result = serde_json::to_string(&output)?;

    Ok(CommandResult {
        output: out,
        result,
    })
}

pub fn round2(name: &str, data: &str) -> Result<()> {
    let state_dir = get_state_dir(name);
    let path = std::path::Path::new(&state_dir);

    if !path.exists() {
        anyhow::bail!(
            "Wallet '{}' not found at {}. Did you run keygen-round1 with --name {}?",
            name,
            state_dir,
            name
        );
    }

    let storage = FileStorage::new(&state_dir)?;
    let cmd_result = round2_core(data, &storage)?;
    println!("{}", cmd_result.output);
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("📋 Copy this JSON:");
    println!("{}\n", cmd_result.result);
    println!("💾 State saved to: {}/", state_dir);
    Ok(())
}

pub fn finalize_core(data: &str, storage: &dyn Storage) -> Result<CommandResult> {
    let mut out = String::new();

    // Load state
    let state_json = String::from_utf8(storage.read("round1_state.json")?)?;
    let state: Round1State = serde_json::from_str(&state_json)?;

    let mode_name = if state.hierarchical { "HTSS" } else { "TSS" };
    out.push_str(&format!("FROST Keygen ({}) - Finalize\n\n", mode_name));

    let commitments_json = String::from_utf8(storage.read("all_commitments.json")?)?;
    let round1_outputs: Vec<Round1Output> = parse_space_separated_json(&commitments_json)?;

    // Collect party ranks for HTSS metadata
    let mut party_ranks = std::collections::BTreeMap::new();
    for output in &round1_outputs {
        party_ranks.insert(output.party_index, output.rank);
    }

    let commitments: Vec<CommitmentData> = round1_outputs
        .iter()
        .map(|output| CommitmentData {
            index: output.party_index,
            data: output.keygen_input.clone(),
        })
        .collect();
    let commitments_input = Round1Input { commitments };

    // Parse shares sent to me - space-separated Round2Output objects
    let round2_outputs: Vec<Round2Output> = parse_space_separated_json(data)?;

    // Extract shares sent to my_index
    let mut shares_for_me = Vec::new();
    for output in round2_outputs {
        for share in output.shares {
            if share.to_index == state.my_index {
                shares_for_me.push(IncomingShare {
                    from_index: output.party_index,
                    share: share.share,
                });
            }
        }
    }

    let shares_input = Round2Input { shares_for_me };

    out.push_str(&format!(
        " Received {} keygen shares sent to you\n\n",
        shares_input.shares_for_me.len()
    ));

    out.push_str("⚙️  Computing your final secret share:\n");
    out.push_str("🧠 How it works:\n");
    out.push_str("   Your final secret share = sum of all keygen shares received\n");
    out.push_str(&format!(
        "   secret_share = f₁({}) + f₂({}) + f₃({}) + ...\n",
        state.my_index, state.my_index, state.my_index
    ));
    out.push_str("   \n");
    out.push_str("   This is YOUR piece of the distributed private key!\n");
    out.push_str(&format!(
        "   With {} secret shares, you can reconstruct the full key.\n\n",
        state.threshold
    ));

    // Collect keygen shares into a vector
    let mut secret_share_inputs = Vec::new();
    for incoming in &shares_input.shares_for_me {
        let share_bytes = hex::decode(&incoming.share)?;
        let share: Scalar<Secret, Zero> = bincode::deserialize(&share_bytes)?;
        secret_share_inputs.push(share);
        out.push_str(&format!(
            "   + Party {}'s keygen share\n",
            incoming.from_index
        ));
    }

    out.push_str("\n⚙️  Computing shared public key:\n");
    out.push_str("🧠 How the group public key is created:\n");
    out.push_str("   PublicKey = sum of all parties' a₀*G commitments\n");
    out.push_str("   PK = (a₀)₁*G + (a₀)₂*G + (a₀)₃*G + ...\n");
    out.push_str("   \n");
    out.push_str("   Since PK = (a₀)₁ + (a₀)₂ + ... times G,\n");
    out.push_str("   and the private key = (a₀)₁ + (a₀)₂ + ...,\n");
    out.push_str("   this IS the public key for the distributed private key!\n\n");

    // Reconstruct all KeygenInputs to get the aggregated key
    let frost = frost::new_with_deterministic_nonces::<Sha256>();
    let mut coordinator = Coordinator::new(state.threshold, state.n_parties);

    for commit_data in &commitments_input.commitments {
        let keygen_input_bytes = hex::decode(&commit_data.data)?;
        let keygen_input: KeygenInput = bincode::deserialize(&keygen_input_bytes)?;
        coordinator
            .add_input(&frost.schnorr, commit_data.index - 1, keygen_input)
            .map_err(|e| anyhow::anyhow!("Failed to add input: {}", e))?;
    }

    let agg_input = coordinator.finish().context("Coordinator not finished")?;

    out.push_str("⚙️  Verifying keygen shares against commitments:\n");
    out.push_str("🧠 Critical security check!\n");
    out.push_str("   For each share f_i(j) received from party i:\n");
    out.push_str("   • Verify: f_i(j)*G == C_0 + C_1*j + C_2*j² + ...\n");
    out.push_str("   • Where [C_0, C_1, C_2, ...] are party i's commitments from Round 1\n");
    out.push_str("   • This proves the share is consistent with the polynomial!\n");
    out.push_str("   • Prevents malicious parties from sending bad shares\n\n");

    // Use SimplePedPop utility functions to properly create and pair the secret share
    let my_share_index = Scalar::<Secret, Zero>::from(state.my_index)
        .public()
        .non_zero()
        .expect("participant index cant be zero");

    let secret_share = simplepedpop::collect_secret_inputs(my_share_index, secret_share_inputs);

    out.push_str("⚙️  Calling simplepedpop::receive_secret_share()...\n");
    out.push_str("   This verifies all shares and pairs them with the commitments\n\n");

    let paired_share = simplepedpop::receive_secret_share(&frost.schnorr, &agg_input, secret_share)
        .map_err(|e| anyhow::anyhow!("Share verification failed: {:?}", e))?;

    out.push_str("❄️  All shares verified successfully!\n");
    out.push_str("   Every share is cryptographically valid\n\n");

    let shared_key = agg_input.shared_key();

    // Convert to xonly (EvenY) for BIP340 compatibility
    let xonly_paired_share = paired_share
        .non_zero()
        .context("Paired share is zero")?
        .into_xonly();
    let xonly_shared_key = shared_key
        .non_zero()
        .context("Shared key is zero")?
        .into_xonly();

    // Display clean hex (just the raw bytes, no metadata)
    let final_share_hex = hex::encode(xonly_paired_share.secret_share().share.to_bytes());
    let public_key_hex = hex::encode(xonly_shared_key.public_key().to_bytes());

    // Save bincode format for loading later (includes type info for deserialization)
    let final_share_bytes = bincode::serialize(&xonly_paired_share)?;
    let public_key_bytes = bincode::serialize(&xonly_shared_key)?;
    storage.write("paired_secret_share.bin", &final_share_bytes)?;
    storage.write("shared_key.bin", &public_key_bytes)?;

    // Save HTSS metadata
    let htss_metadata = HtssMetadata {
        my_index: state.my_index,
        my_rank: state.my_rank,
        threshold: state.threshold,
        hierarchical: state.hierarchical,
        party_ranks,
    };
    storage.write(
        "htss_metadata.json",
        serde_json::to_string_pretty(&htss_metadata)?.as_bytes(),
    )?;

    out.push_str("\n━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");
    out.push_str("❄️  Key generation complete!\n");
    out.push_str("   Compare public keys with other tables to verify!\n\n");

    if state.hierarchical {
        out.push_str("🔐 HTSS Configuration:\n");
        out.push_str(&format!("   Your rank: {}\n", state.my_rank));
        out.push_str("   Party ranks: ");
        let ranks_str: Vec<String> = htss_metadata
            .party_ranks
            .iter()
            .map(|(idx, rank)| format!("P{}=r{}", idx, rank))
            .collect();
        out.push_str(&ranks_str.join(", "));
        out.push_str("\n\n");
        out.push_str("🧠 HTSS Signing Rules:\n");
        out.push_str("   To sign, signers' ranks (sorted) must satisfy: rank[i] <= i\n");
        out.push_str("   Example: [0,1,1] valid, [1,1,2] invalid (rank 1 > position 0)\n\n");
    }

    // Create result with the keys
    let result = format!(
        "Secret Share: {}\nPublic Key: {}\nMode: {}",
        final_share_hex, public_key_hex, mode_name
    );

    Ok(CommandResult {
        output: out,
        result,
    })
}

pub fn finalize(name: &str, data: &str) -> Result<()> {
    let state_dir = get_state_dir(name);
    let path = std::path::Path::new(&state_dir);

    if !path.exists() {
        anyhow::bail!(
            "Wallet '{}' not found at {}. Did you run keygen-round1 with --name {}?",
            name,
            state_dir,
            name
        );
    }

    let storage = FileStorage::new(&state_dir)?;
    let cmd_result = finalize_core(data, &storage)?;

    // Generate group_info.json
    generate_group_info(name, &storage)?;

    println!("{}", cmd_result.output);
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("📋 Your keys:");
    println!("{}\n", cmd_result.result);
    println!("💾 Wallet saved to: {}/", state_dir);
    println!("📄 Group info: {}/group_info.json", state_dir);
    Ok(())
}

/// Generate group_info.json with parties ordered by rank
fn generate_group_info(name: &str, storage: &dyn Storage) -> Result<()> {
    // Load HTSS metadata
    let htss_json = String::from_utf8(storage.read("htss_metadata.json")?)?;
    let htss: HtssMetadata = serde_json::from_str(&htss_json)?;

    // Load shared key for public key and addresses
    let shared_key_bytes = storage.read("shared_key.bin")?;
    let xonly_shared_key: schnorr_fun::frost::SharedKey<schnorr_fun::fun::marker::EvenY> =
        bincode::deserialize(&shared_key_bytes)?;

    // Get x-only public key bytes (32 bytes)
    let pubkey_bytes: [u8; 32] = xonly_shared_key.public_key().to_xonly_bytes();
    let public_key_hex = hex::encode(pubkey_bytes);

    // Generate Taproot addresses
    use bitcoin::{Address, Network, XOnlyPublicKey};
    let xonly_pk = XOnlyPublicKey::from_slice(&pubkey_bytes)
        .map_err(|e| anyhow::anyhow!("Invalid public key: {}", e))?;

    let secp = bitcoin::secp256k1::Secp256k1::new();
    let address_testnet = Address::p2tr(&secp, xonly_pk, None, Network::Testnet).to_string();
    let address_mainnet = Address::p2tr(&secp, xonly_pk, None, Network::Bitcoin).to_string();

    // Load commitments to extract verification shares
    let commitments_json = String::from_utf8(storage.read("all_commitments.json")?)?;
    let round1_outputs: Vec<Round1Output> = parse_space_separated_json(&commitments_json)?;

    // Build party info with verification shares
    let mut parties: Vec<PartyInfo> = Vec::new();
    for output in &round1_outputs {
        // Try to extract verification share from keygen_input (first commitment = a0*G)
        let verification_share = match hex::decode(&output.keygen_input) {
            Ok(keygen_input_bytes) => {
                match bincode::deserialize::<schnorr_fun::frost::chilldkg::simplepedpop::KeygenInput>(
                    &keygen_input_bytes,
                ) {
                    Ok(keygen_input) => {
                        // The first coefficient commitment is the verification share
                        if !keygen_input.com.is_empty() {
                            hex::encode(keygen_input.com[0].to_bytes())
                        } else {
                            "unavailable".to_string()
                        }
                    }
                    Err(_) => "unavailable".to_string(),
                }
            }
            Err(_) => "unavailable".to_string(),
        };

        parties.push(PartyInfo {
            index: output.party_index,
            rank: output.rank,
            verification_share,
        });
    }

    // Sort parties by rank (ascending), then by index
    parties.sort_by(|a, b| a.rank.cmp(&b.rank).then(a.index.cmp(&b.index)));

    let group_info = GroupInfo {
        name: name.to_string(),
        group_public_key: public_key_hex,
        taproot_address_testnet: address_testnet,
        taproot_address_mainnet: address_mainnet,
        threshold: htss.threshold,
        total_parties: parties.len() as u32,
        hierarchical: htss.hierarchical,
        parties,
    };

    storage.write(
        "group_info.json",
        serde_json::to_string_pretty(&group_info)?.as_bytes(),
    )?;

    Ok(())
}

/// Regenerate group_info.json for an existing wallet
pub fn regenerate_group_info(name: &str) -> Result<()> {
    let state_dir = get_state_dir(name);
    let path = std::path::Path::new(&state_dir);

    if !path.exists() {
        anyhow::bail!("Wallet '{}' not found at {}.", name, state_dir);
    }

    let storage = FileStorage::new(&state_dir)?;
    generate_group_info(name, &storage)?;

    // Read and display the generated info
    let info_path = path.join("group_info.json");
    let content = std::fs::read_to_string(&info_path)?;
    let info: GroupInfo = serde_json::from_str(&content)?;

    println!("Group Info for '{}'\n", name);
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("Public Key: {}", info.group_public_key);
    println!("Threshold:  {}-of-{}", info.threshold, info.total_parties);
    println!(
        "Mode:       {}",
        if info.hierarchical { "HTSS" } else { "TSS" }
    );
    println!();
    println!("Addresses:");
    println!("  Testnet: {}", info.taproot_address_testnet);
    println!("  Mainnet: {}", info.taproot_address_mainnet);
    println!();
    println!("Parties (sorted by rank):");
    for party in &info.parties {
        let share_display = if party.verification_share.len() > 16 {
            format!("{}...", &party.verification_share[..16])
        } else {
            party.verification_share.clone()
        };
        println!(
            "  Party {} (rank {}): {}",
            party.index, party.rank, share_display
        );
    }
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("\nSaved to: {}/group_info.json", state_dir);

    Ok(())
}
