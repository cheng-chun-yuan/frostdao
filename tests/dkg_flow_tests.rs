//! Integration tests for full DKG flow using CLI commands

mod common;

use common::{cleanup_state_prefix, extract_json, frostdao_bin};
use std::fs;
use std::process::Command;
use std::sync::atomic::{AtomicU32, Ordering};

// Atomic counter for unique test IDs
static TEST_COUNTER: AtomicU32 = AtomicU32::new(0);

fn get_unique_prefix() -> String {
    let id = TEST_COUNTER.fetch_add(1, Ordering::SeqCst);
    let time = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis();
    format!("test_{}_{}", time, id)
}

fn cleanup_wallet(prefix: &str) {
    cleanup_state_prefix(prefix);
}

/// Test complete 2-of-3 DKG flow
#[test]
fn test_full_2_of_3_dkg_flow() {
    let prefix = get_unique_prefix();
    let wallet1 = format!("{}_p1", prefix);
    let wallet2 = format!("{}_p2", prefix);
    let wallet3 = format!("{}_p3", prefix);

    // Round 1: All parties generate commitments
    let r1_p1 = Command::new(frostdao_bin())
        .args([
            "keygen-round1",
            "--name",
            &wallet1,
            "--threshold",
            "2",
            "--n-parties",
            "3",
            "--my-index",
            "1",
        ])
        .output()
        .expect("Failed to run keygen-round1 for party 1");
    assert!(
        r1_p1.status.success(),
        "Party 1 round1 failed: {}",
        String::from_utf8_lossy(&r1_p1.stderr)
    );
    let commit1 = extract_json(&String::from_utf8_lossy(&r1_p1.stdout)).unwrap_or_else(|| {
        panic!(
            "No JSON from party 1. Output: {}",
            String::from_utf8_lossy(&r1_p1.stdout)
        )
    });

    let r1_p2 = Command::new(frostdao_bin())
        .args([
            "keygen-round1",
            "--name",
            &wallet2,
            "--threshold",
            "2",
            "--n-parties",
            "3",
            "--my-index",
            "2",
        ])
        .output()
        .expect("Failed to run keygen-round1 for party 2");
    assert!(r1_p2.status.success(), "Party 2 round1 failed");
    let commit2 =
        extract_json(&String::from_utf8_lossy(&r1_p2.stdout)).expect("No JSON from party 2");

    let r1_p3 = Command::new(frostdao_bin())
        .args([
            "keygen-round1",
            "--name",
            &wallet3,
            "--threshold",
            "2",
            "--n-parties",
            "3",
            "--my-index",
            "3",
        ])
        .output()
        .expect("Failed to run keygen-round1 for party 3");
    assert!(r1_p3.status.success(), "Party 3 round1 failed");
    let commit3 =
        extract_json(&String::from_utf8_lossy(&r1_p3.stdout)).expect("No JSON from party 3");

    let all_commits = format!("{} {} {}", commit1, commit2, commit3);

    // Round 2: Exchange shares
    let r2_p1 = Command::new(frostdao_bin())
        .args(["keygen-round2", "--name", &wallet1, "--data", &all_commits])
        .output()
        .expect("Failed to run keygen-round2 for party 1");
    assert!(
        r2_p1.status.success(),
        "Party 1 round2 failed: {}",
        String::from_utf8_lossy(&r2_p1.stderr)
    );
    let shares1 =
        extract_json(&String::from_utf8_lossy(&r2_p1.stdout)).expect("No shares from party 1");

    let r2_p2 = Command::new(frostdao_bin())
        .args(["keygen-round2", "--name", &wallet2, "--data", &all_commits])
        .output()
        .expect("Failed to run keygen-round2 for party 2");
    assert!(
        r2_p2.status.success(),
        "Party 2 round2 failed: {}",
        String::from_utf8_lossy(&r2_p2.stderr)
    );
    let shares2 =
        extract_json(&String::from_utf8_lossy(&r2_p2.stdout)).expect("No shares from party 2");

    let r2_p3 = Command::new(frostdao_bin())
        .args(["keygen-round2", "--name", &wallet3, "--data", &all_commits])
        .output()
        .expect("Failed to run keygen-round2 for party 3");
    assert!(r2_p3.status.success(), "Party 3 round2 failed");
    let shares3 =
        extract_json(&String::from_utf8_lossy(&r2_p3.stdout)).expect("No shares from party 3");

    let all_shares = format!("{} {} {}", shares1, shares2, shares3);

    // Finalize: All parties compute final keys
    let fin_p1 = Command::new(frostdao_bin())
        .args(["keygen-finalize", "--name", &wallet1, "--data", &all_shares])
        .output()
        .expect("Failed to run keygen-finalize for party 1");
    assert!(
        fin_p1.status.success(),
        "Party 1 finalize failed: {}",
        String::from_utf8_lossy(&fin_p1.stderr)
    );

    let fin_p2 = Command::new(frostdao_bin())
        .args(["keygen-finalize", "--name", &wallet2, "--data", &all_shares])
        .output()
        .expect("Failed to run keygen-finalize for party 2");
    assert!(fin_p2.status.success(), "Party 2 finalize failed");

    let fin_p3 = Command::new(frostdao_bin())
        .args(["keygen-finalize", "--name", &wallet3, "--data", &all_shares])
        .output()
        .expect("Failed to run keygen-finalize for party 3");
    assert!(fin_p3.status.success(), "Party 3 finalize failed");

    // Verify all parties have the same group public key
    let addr1 = Command::new(frostdao_bin())
        .args(["dkg-address", "--name", &wallet1])
        .output()
        .expect("Failed to get address for party 1");
    let addr1_json =
        extract_json(&String::from_utf8_lossy(&addr1.stdout)).expect("No address JSON from p1");

    let addr2 = Command::new(frostdao_bin())
        .args(["dkg-address", "--name", &wallet2])
        .output()
        .expect("Failed to get address for party 2");
    let addr2_json =
        extract_json(&String::from_utf8_lossy(&addr2.stdout)).expect("No address JSON from p2");

    let addr3 = Command::new(frostdao_bin())
        .args(["dkg-address", "--name", &wallet3])
        .output()
        .expect("Failed to get address for party 3");
    let addr3_json =
        extract_json(&String::from_utf8_lossy(&addr3.stdout)).expect("No address JSON from p3");

    // Parse addresses and compare
    let pk1: serde_json::Value = serde_json::from_str(&addr1_json).unwrap();
    let pk2: serde_json::Value = serde_json::from_str(&addr2_json).unwrap();
    let pk3: serde_json::Value = serde_json::from_str(&addr3_json).unwrap();

    assert_eq!(
        pk1["public_key"], pk2["public_key"],
        "Party 1 and 2 have different public keys"
    );
    assert_eq!(
        pk2["public_key"], pk3["public_key"],
        "Party 2 and 3 have different public keys"
    );
    assert_eq!(
        pk1["address"], pk2["address"],
        "Party 1 and 2 have different addresses"
    );

    cleanup_wallet(&prefix);
}

/// Test resharing preserves address
#[test]
fn test_resharing_preserves_address() {
    let prefix = get_unique_prefix();
    let wallet1 = format!("{}_p1", prefix);
    let wallet2 = format!("{}_p2", prefix);
    let new_wallet = format!("{}_new", prefix);

    // Create 2-of-2 wallet (simpler for testing)
    let r1_p1 = Command::new(frostdao_bin())
        .args([
            "keygen-round1",
            "--name",
            &wallet1,
            "--threshold",
            "2",
            "--n-parties",
            "2",
            "--my-index",
            "1",
        ])
        .output()
        .expect("keygen-round1 failed");
    assert!(
        r1_p1.status.success(),
        "p1 r1 failed: {}",
        String::from_utf8_lossy(&r1_p1.stderr)
    );
    let commit1 = extract_json(&String::from_utf8_lossy(&r1_p1.stdout)).unwrap_or_else(|| {
        panic!(
            "No JSON. Output: {}",
            String::from_utf8_lossy(&r1_p1.stdout)
        )
    });

    let r1_p2 = Command::new(frostdao_bin())
        .args([
            "keygen-round1",
            "--name",
            &wallet2,
            "--threshold",
            "2",
            "--n-parties",
            "2",
            "--my-index",
            "2",
        ])
        .output()
        .expect("keygen-round1 failed");
    assert!(r1_p2.status.success());
    let commit2 = extract_json(&String::from_utf8_lossy(&r1_p2.stdout)).unwrap();

    let commits = format!("{} {}", commit1, commit2);

    let r2_p1 = Command::new(frostdao_bin())
        .args(["keygen-round2", "--name", &wallet1, "--data", &commits])
        .output()
        .expect("keygen-round2 failed");
    assert!(r2_p1.status.success());
    let shares1 = extract_json(&String::from_utf8_lossy(&r2_p1.stdout)).unwrap();

    let r2_p2 = Command::new(frostdao_bin())
        .args(["keygen-round2", "--name", &wallet2, "--data", &commits])
        .output()
        .expect("keygen-round2 failed");
    assert!(r2_p2.status.success());
    let shares2 = extract_json(&String::from_utf8_lossy(&r2_p2.stdout)).unwrap();

    let shares = format!("{} {}", shares1, shares2);

    let fin1 = Command::new(frostdao_bin())
        .args(["keygen-finalize", "--name", &wallet1, "--data", &shares])
        .output()
        .expect("keygen-finalize failed");
    assert!(fin1.status.success());

    let fin2 = Command::new(frostdao_bin())
        .args(["keygen-finalize", "--name", &wallet2, "--data", &shares])
        .output()
        .expect("keygen-finalize failed");
    assert!(fin2.status.success());

    // Get original address
    let orig_addr = Command::new(frostdao_bin())
        .args(["dkg-address", "--name", &wallet1])
        .output()
        .expect("dkg-address failed");
    let orig_json = extract_json(&String::from_utf8_lossy(&orig_addr.stdout)).unwrap();
    let orig: serde_json::Value = serde_json::from_str(&orig_json).unwrap();

    // Reshare
    let reshare1 = Command::new(frostdao_bin())
        .args([
            "reshare-round1",
            "--source",
            &wallet1,
            "--new-threshold",
            "2",
            "--new-n-parties",
            "2",
            "--my-index",
            "1",
        ])
        .output()
        .expect("reshare-round1 failed");
    assert!(
        reshare1.status.success(),
        "reshare1 failed: {}",
        String::from_utf8_lossy(&reshare1.stderr)
    );
    let sub1 = extract_json(&String::from_utf8_lossy(&reshare1.stdout)).unwrap();

    let reshare2 = Command::new(frostdao_bin())
        .args([
            "reshare-round1",
            "--source",
            &wallet2,
            "--new-threshold",
            "2",
            "--new-n-parties",
            "2",
            "--my-index",
            "2",
        ])
        .output()
        .expect("reshare-round1 failed");
    assert!(reshare2.status.success());
    let sub2 = extract_json(&String::from_utf8_lossy(&reshare2.stdout)).unwrap();

    let reshare_data = format!("{} {}", sub1, sub2);

    // Finalize resharing - use echo to provide 'y' input
    let finalize =
        Command::new("sh")
            .args([
                "-c",
                &format!(
                "echo 'y' | {} reshare-finalize --source {} --target {} --my-index 1 --data '{}'",
                frostdao_bin(), wallet1, new_wallet, reshare_data
            ),
            ])
            .output()
            .expect("reshare-finalize failed");

    assert!(
        finalize.status.success(),
        "reshare-finalize failed: {}",
        String::from_utf8_lossy(&finalize.stderr)
    );

    // Get new address
    let new_addr = Command::new(frostdao_bin())
        .args(["dkg-address", "--name", &new_wallet])
        .output()
        .expect("dkg-address failed");
    let new_json = extract_json(&String::from_utf8_lossy(&new_addr.stdout)).unwrap_or_else(|| {
        panic!(
            "No JSON from new wallet. Output: {}",
            String::from_utf8_lossy(&new_addr.stdout)
        )
    });
    let new: serde_json::Value = serde_json::from_str(&new_json).unwrap();

    assert_eq!(
        orig["address"], new["address"],
        "Reshared wallet has different address! Original: {}, New: {}",
        orig["address"], new["address"]
    );
    assert_eq!(
        orig["public_key"], new["public_key"],
        "Reshared wallet has different public key!"
    );

    cleanup_wallet(&prefix);
}

#[test]
fn test_reshare_preserves_hd_address_control() {
    let prefix = get_unique_prefix();
    let wallet1 = format!("{}_p1", prefix);
    let wallet2 = format!("{}_p2", prefix);
    let new_wallet = format!("{}_new", prefix);

    let parse_hd_address = |output: &str| -> String {
        output
            .lines()
            .find_map(|line| {
                let line = line.trim();
                line.strip_prefix("Address: ")
                    .map(|value| value.to_string())
            })
            .unwrap_or_else(|| panic!("Missing Address line in output: {output}"))
    };

    // Create 2-of-2 wallet
    let r1_p1 = Command::new(frostdao_bin())
        .args([
            "keygen-round1",
            "--name",
            &wallet1,
            "--threshold",
            "2",
            "--n-parties",
            "2",
            "--my-index",
            "1",
        ])
        .output()
        .expect("keygen-round1 failed");
    assert!(r1_p1.status.success(), "p1 r1 failed");
    let commit1 = extract_json(&String::from_utf8_lossy(&r1_p1.stdout)).unwrap();

    let r1_p2 = Command::new(frostdao_bin())
        .args([
            "keygen-round1",
            "--name",
            &wallet2,
            "--threshold",
            "2",
            "--n-parties",
            "2",
            "--my-index",
            "2",
        ])
        .output()
        .expect("keygen-round1 failed");
    assert!(r1_p2.status.success(), "p2 r1 failed");
    let commit2 = extract_json(&String::from_utf8_lossy(&r1_p2.stdout)).unwrap();

    let commits = format!("{} {}", commit1, commit2);

    let r2_p1 = Command::new(frostdao_bin())
        .args(["keygen-round2", "--name", &wallet1, "--data", &commits])
        .output()
        .expect("keygen-round2 failed");
    assert!(r2_p1.status.success(), "p1 r2 failed");
    let shares1 = extract_json(&String::from_utf8_lossy(&r2_p1.stdout)).unwrap();

    let r2_p2 = Command::new(frostdao_bin())
        .args(["keygen-round2", "--name", &wallet2, "--data", &commits])
        .output()
        .expect("keygen-round2 failed");
    assert!(r2_p2.status.success(), "p2 r2 failed");
    let shares2 = extract_json(&String::from_utf8_lossy(&r2_p2.stdout)).unwrap();

    let shares = format!("{} {}", shares1, shares2);

    let fin1 = Command::new(frostdao_bin())
        .args(["keygen-finalize", "--name", &wallet1, "--data", &shares])
        .output()
        .expect("keygen-finalize failed");
    assert!(fin1.status.success(), "p1 finalize failed");

    let fin2 = Command::new(frostdao_bin())
        .args(["keygen-finalize", "--name", &wallet2, "--data", &shares])
        .output()
        .expect("keygen-finalize failed");
    assert!(fin2.status.success(), "p2 finalize failed");

    let source_hd_path = format!(".frost_state/{}/hd_metadata.json", wallet1);
    let source_hd_content =
        fs::read_to_string(&source_hd_path).expect("source hd_metadata.json should exist");
    let source_hd: serde_json::Value = serde_json::from_str(&source_hd_content).unwrap();
    assert!(source_hd["hd_enabled"].as_bool().unwrap_or(false));

    let source_hd_address = Command::new(frostdao_bin())
        .args([
            "dkg-derive-address",
            "--name",
            &wallet1,
            "--change",
            "0",
            "--index",
            "4",
            "--network",
            "testnet",
        ])
        .output()
        .expect("dkg-derive-address failed");
    assert!(
        source_hd_address.status.success(),
        "source derive address failed"
    );
    let source_hd_output = String::from_utf8_lossy(&source_hd_address.stdout);
    let source_hd_addr = parse_hd_address(&source_hd_output);

    let reshare1 = Command::new(frostdao_bin())
        .args([
            "reshare-round1",
            "--source",
            &wallet1,
            "--new-threshold",
            "2",
            "--new-n-parties",
            "2",
            "--my-index",
            "1",
        ])
        .output()
        .expect("reshare-round1 failed");
    assert!(reshare1.status.success(), "reshare1 failed");
    let sub1 = extract_json(&String::from_utf8_lossy(&reshare1.stdout)).unwrap();

    let reshare2 = Command::new(frostdao_bin())
        .args([
            "reshare-round1",
            "--source",
            &wallet2,
            "--new-threshold",
            "2",
            "--new-n-parties",
            "2",
            "--my-index",
            "2",
        ])
        .output()
        .expect("reshare-round1 failed");
    assert!(reshare2.status.success(), "reshare2 failed");
    let sub2 = extract_json(&String::from_utf8_lossy(&reshare2.stdout)).unwrap();

    let reshare_data = format!("{} {}", sub1, sub2);

    let finalize = Command::new("sh")
        .args([
            "-c",
            &format!(
                "echo 'y' | {} reshare-finalize --source {} --target {} --my-index 1 --data '{}'",
                frostdao_bin(),
                wallet1,
                new_wallet,
                reshare_data
            ),
        ])
        .output()
        .expect("reshare-finalize failed");
    assert!(finalize.status.success(), "reshare-finalize failed");

    let target_hd_path = format!(".frost_state/{}/hd_metadata.json", new_wallet);
    let target_hd_content =
        fs::read_to_string(&target_hd_path).expect("target hd_metadata.json should exist");
    let target_hd: serde_json::Value = serde_json::from_str(&target_hd_content).unwrap();

    assert_eq!(source_hd["chain_code"], target_hd["chain_code"]);
    assert_eq!(source_hd["hd_enabled"], target_hd["hd_enabled"]);

    let target_hd_address = Command::new(frostdao_bin())
        .args([
            "dkg-derive-address",
            "--name",
            &new_wallet,
            "--change",
            "0",
            "--index",
            "4",
            "--network",
            "testnet",
        ])
        .output()
        .expect("dkg-derive-address failed");
    assert!(
        target_hd_address.status.success(),
        "target derive address failed"
    );
    let target_hd_output = String::from_utf8_lossy(&target_hd_address.stdout);
    let target_hd_addr = parse_hd_address(&target_hd_output);

    assert_eq!(
        source_hd_addr, target_hd_addr,
        "Derived HD address should stay exactly the same after reshare."
    );

    cleanup_wallet(&prefix);
}

/// Test wallet listing
#[test]
fn test_wallet_listing() {
    let prefix = get_unique_prefix();
    let wallet = format!("{}_list", prefix);

    // Create a 1-of-1 wallet (simplest case)
    let r1 = Command::new(frostdao_bin())
        .args([
            "keygen-round1",
            "--name",
            &wallet,
            "--threshold",
            "1",
            "--n-parties",
            "1",
            "--my-index",
            "1",
        ])
        .output()
        .expect("keygen-round1 failed");
    assert!(r1.status.success());
    let commit = extract_json(&String::from_utf8_lossy(&r1.stdout)).unwrap();

    let r2 = Command::new(frostdao_bin())
        .args(["keygen-round2", "--name", &wallet, "--data", &commit])
        .output()
        .expect("keygen-round2 failed");
    assert!(r2.status.success());
    let shares = extract_json(&String::from_utf8_lossy(&r2.stdout)).unwrap();

    let fin = Command::new(frostdao_bin())
        .args(["keygen-finalize", "--name", &wallet, "--data", &shares])
        .output()
        .expect("keygen-finalize failed");
    assert!(fin.status.success());

    // List wallets
    let list = Command::new(frostdao_bin())
        .args(["dkg-list"])
        .output()
        .expect("dkg-list failed");

    let output = String::from_utf8_lossy(&list.stdout);
    assert!(
        output.contains(&wallet),
        "Wallet list should contain created wallet. Output: {}",
        output
    );

    cleanup_wallet(&prefix);
}

/// Test complete 2-of-3 HTSS (Hierarchical) DKG flow
#[test]
fn test_full_2_of_3_htss_flow() {
    let prefix = get_unique_prefix();
    let wallet1 = format!("{}_htss_p1", prefix);
    let wallet2 = format!("{}_htss_p2", prefix);
    let wallet3 = format!("{}_htss_p3", prefix);

    // Round 1: All parties generate commitments with HTSS enabled
    // Party 1: rank 0 (highest authority)
    let r1_p1 = Command::new(frostdao_bin())
        .args([
            "keygen-round1",
            "--name",
            &wallet1,
            "--threshold",
            "2",
            "--n-parties",
            "3",
            "--my-index",
            "1",
            "--rank",
            "0",
            "--hierarchical",
        ])
        .output()
        .expect("Failed to run keygen-round1 for party 1");
    assert!(
        r1_p1.status.success(),
        "HTSS Party 1 round1 failed: {}",
        String::from_utf8_lossy(&r1_p1.stderr)
    );
    let commit1 =
        extract_json(&String::from_utf8_lossy(&r1_p1.stdout)).expect("No JSON from HTSS party 1");

    // Party 2: rank 1 (lower authority)
    let r1_p2 = Command::new(frostdao_bin())
        .args([
            "keygen-round1",
            "--name",
            &wallet2,
            "--threshold",
            "2",
            "--n-parties",
            "3",
            "--my-index",
            "2",
            "--rank",
            "1",
            "--hierarchical",
        ])
        .output()
        .expect("Failed to run keygen-round1 for party 2");
    assert!(r1_p2.status.success(), "HTSS Party 2 round1 failed");
    let commit2 =
        extract_json(&String::from_utf8_lossy(&r1_p2.stdout)).expect("No JSON from HTSS party 2");

    // Party 3: rank 1 (same as party 2)
    let r1_p3 = Command::new(frostdao_bin())
        .args([
            "keygen-round1",
            "--name",
            &wallet3,
            "--threshold",
            "2",
            "--n-parties",
            "3",
            "--my-index",
            "3",
            "--rank",
            "1",
            "--hierarchical",
        ])
        .output()
        .expect("Failed to run keygen-round1 for party 3");
    assert!(r1_p3.status.success(), "HTSS Party 3 round1 failed");
    let commit3 =
        extract_json(&String::from_utf8_lossy(&r1_p3.stdout)).expect("No JSON from HTSS party 3");

    let all_commits = format!("{} {} {}", commit1, commit2, commit3);

    // Round 2: Exchange shares
    let r2_p1 = Command::new(frostdao_bin())
        .args(["keygen-round2", "--name", &wallet1, "--data", &all_commits])
        .output()
        .expect("Failed to run keygen-round2 for party 1");
    assert!(
        r2_p1.status.success(),
        "HTSS Party 1 round2 failed: {}",
        String::from_utf8_lossy(&r2_p1.stderr)
    );
    let shares1 =
        extract_json(&String::from_utf8_lossy(&r2_p1.stdout)).expect("No shares from HTSS party 1");

    let r2_p2 = Command::new(frostdao_bin())
        .args(["keygen-round2", "--name", &wallet2, "--data", &all_commits])
        .output()
        .expect("Failed to run keygen-round2 for party 2");
    assert!(r2_p2.status.success(), "HTSS Party 2 round2 failed");
    let shares2 =
        extract_json(&String::from_utf8_lossy(&r2_p2.stdout)).expect("No shares from HTSS party 2");

    let r2_p3 = Command::new(frostdao_bin())
        .args(["keygen-round2", "--name", &wallet3, "--data", &all_commits])
        .output()
        .expect("Failed to run keygen-round2 for party 3");
    assert!(r2_p3.status.success(), "HTSS Party 3 round2 failed");
    let shares3 =
        extract_json(&String::from_utf8_lossy(&r2_p3.stdout)).expect("No shares from HTSS party 3");

    let all_shares = format!("{} {} {}", shares1, shares2, shares3);

    // Finalize: All parties compute final keys
    let fin_p1 = Command::new(frostdao_bin())
        .args(["keygen-finalize", "--name", &wallet1, "--data", &all_shares])
        .output()
        .expect("Failed to run keygen-finalize for party 1");
    assert!(
        fin_p1.status.success(),
        "HTSS Party 1 finalize failed: {}",
        String::from_utf8_lossy(&fin_p1.stderr)
    );

    let fin_p2 = Command::new(frostdao_bin())
        .args(["keygen-finalize", "--name", &wallet2, "--data", &all_shares])
        .output()
        .expect("Failed to run keygen-finalize for party 2");
    assert!(fin_p2.status.success(), "HTSS Party 2 finalize failed");

    let fin_p3 = Command::new(frostdao_bin())
        .args(["keygen-finalize", "--name", &wallet3, "--data", &all_shares])
        .output()
        .expect("Failed to run keygen-finalize for party 3");
    assert!(fin_p3.status.success(), "HTSS Party 3 finalize failed");

    // Verify all parties have the same group public key
    let addr1 = Command::new(frostdao_bin())
        .args(["dkg-address", "--name", &wallet1])
        .output()
        .expect("Failed to get address for party 1");
    let addr1_json = extract_json(&String::from_utf8_lossy(&addr1.stdout))
        .expect("No address JSON from HTSS p1");

    let addr2 = Command::new(frostdao_bin())
        .args(["dkg-address", "--name", &wallet2])
        .output()
        .expect("Failed to get address for party 2");
    let addr2_json = extract_json(&String::from_utf8_lossy(&addr2.stdout))
        .expect("No address JSON from HTSS p2");

    let addr3 = Command::new(frostdao_bin())
        .args(["dkg-address", "--name", &wallet3])
        .output()
        .expect("Failed to get address for party 3");
    let addr3_json = extract_json(&String::from_utf8_lossy(&addr3.stdout))
        .expect("No address JSON from HTSS p3");

    // Parse addresses and compare
    let pk1: serde_json::Value = serde_json::from_str(&addr1_json).unwrap();
    let pk2: serde_json::Value = serde_json::from_str(&addr2_json).unwrap();
    let pk3: serde_json::Value = serde_json::from_str(&addr3_json).unwrap();

    assert_eq!(
        pk1["public_key"], pk2["public_key"],
        "HTSS Party 1 and 2 have different public keys"
    );
    assert_eq!(
        pk2["public_key"], pk3["public_key"],
        "HTSS Party 2 and 3 have different public keys"
    );
    assert_eq!(
        pk1["address"], pk2["address"],
        "HTSS Party 1 and 2 have different addresses"
    );

    // Verify HTSS metadata exists and is correct
    let htss_path1 = format!(".frost_state/{}/htss_metadata.json", wallet1);
    assert!(
        std::path::Path::new(&htss_path1).exists(),
        "HTSS metadata should exist for party 1"
    );

    let htss_content = fs::read_to_string(&htss_path1).expect("Failed to read htss_metadata.json");
    let htss: serde_json::Value = serde_json::from_str(&htss_content).unwrap();
    assert_eq!(
        htss["hierarchical"], true,
        "Should be marked as hierarchical"
    );
    assert_eq!(htss["my_rank"], 0, "Party 1 should have rank 0");

    cleanup_wallet(&prefix);
}
