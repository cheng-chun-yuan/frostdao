//! Integration tests for signing functionality

mod common;

use common::{extract_json, frostdao_bin};
use serial_test::serial;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU32, Ordering};
const TEST_WALLET_PREFIX: &str = "test_sign";

static TEST_COUNTER: AtomicU32 = AtomicU32::new(0);

struct TestWorkspace {
    path: PathBuf,
}

impl TestWorkspace {
    fn new(label: &str) -> Self {
        let id = TEST_COUNTER.fetch_add(1, Ordering::SeqCst);
        let time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis();
        let path = std::env::temp_dir().join(format!(
            "frostdao-signing-{label}-{}-{time}-{id}",
            std::process::id()
        ));
        fs::create_dir_all(&path).expect("create isolated test workspace");
        Self { path }
    }

    fn command(&self) -> Command {
        let mut command = Command::new(frostdao_bin());
        command.current_dir(&self.path);
        command
    }

    fn state_path(&self, path: impl AsRef<Path>) -> PathBuf {
        self.path.join(".frost_state").join(path)
    }
}

impl Drop for TestWorkspace {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

fn get_unique_prefix() -> String {
    let id = TEST_COUNTER.fetch_add(1, Ordering::SeqCst);
    let time = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis();
    format!(
        "{}_{}_{}_{}",
        TEST_WALLET_PREFIX,
        std::process::id(),
        time,
        id
    )
}

fn create_2_of_3_wallet(workspace: &TestWorkspace, prefix: &str) -> (String, String, String) {
    let wallet1 = format!("{}_p1", prefix);
    let wallet2 = format!("{}_p2", prefix);
    let wallet3 = format!("{}_p3", prefix);

    // Round 1
    let r1_p1 = workspace
        .command()
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
        .expect("keygen-round1 failed");
    let commit1 = extract_json(&String::from_utf8_lossy(&r1_p1.stdout)).unwrap();

    let r1_p2 = workspace
        .command()
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
        .expect("keygen-round1 failed");
    let commit2 = extract_json(&String::from_utf8_lossy(&r1_p2.stdout)).unwrap();

    let r1_p3 = workspace
        .command()
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
        .expect("keygen-round1 failed");
    let commit3 = extract_json(&String::from_utf8_lossy(&r1_p3.stdout)).unwrap();

    let commits = format!("{} {} {}", commit1, commit2, commit3);

    // Round 2
    let r2_p1 = workspace
        .command()
        .args(["keygen-round2", "--name", &wallet1, "--data", &commits])
        .output()
        .expect("keygen-round2 failed");
    let shares1 = extract_json(&String::from_utf8_lossy(&r2_p1.stdout)).unwrap();

    let r2_p2 = workspace
        .command()
        .args(["keygen-round2", "--name", &wallet2, "--data", &commits])
        .output()
        .expect("keygen-round2 failed");
    let shares2 = extract_json(&String::from_utf8_lossy(&r2_p2.stdout)).unwrap();

    let r2_p3 = workspace
        .command()
        .args(["keygen-round2", "--name", &wallet3, "--data", &commits])
        .output()
        .expect("keygen-round2 failed");
    let shares3 = extract_json(&String::from_utf8_lossy(&r2_p3.stdout)).unwrap();

    let shares = format!("{} {} {}", shares1, shares2, shares3);

    // Finalize
    workspace
        .command()
        .args(["keygen-finalize", "--name", &wallet1, "--data", &shares])
        .output()
        .expect("keygen-finalize failed");

    workspace
        .command()
        .args(["keygen-finalize", "--name", &wallet2, "--data", &shares])
        .output()
        .expect("keygen-finalize failed");

    workspace
        .command()
        .args(["keygen-finalize", "--name", &wallet3, "--data", &shares])
        .output()
        .expect("keygen-finalize failed");

    (wallet1, wallet2, wallet3)
}

/// Test Bitcoin Schnorr key generation
#[test]
#[serial]
fn test_btc_keygen() {
    let workspace = TestWorkspace::new("btc-keygen");
    let output = workspace
        .command()
        .args(["btc-keygen"])
        .output()
        .expect("btc-keygen failed");

    assert!(output.status.success(), "btc-keygen should succeed");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("public_key") || stdout.contains("Public Key"),
        "Should output public key. Got: {}",
        stdout
    );
}

/// Test Bitcoin Schnorr signing and verification
#[test]
#[serial]
fn test_btc_sign_and_verify() {
    let workspace = TestWorkspace::new("btc-sign-verify");
    // First generate a key
    workspace
        .command()
        .args(["btc-keygen"])
        .output()
        .expect("btc-keygen failed");

    // Get public key
    let pubkey_output = workspace
        .command()
        .args(["btc-pubkey"])
        .output()
        .expect("btc-pubkey failed");

    let pubkey_stdout = String::from_utf8_lossy(&pubkey_output.stdout);
    let pubkey_json = extract_json(&pubkey_stdout).expect("No JSON from btc-pubkey");
    let pubkey: serde_json::Value = serde_json::from_str(&pubkey_json).unwrap();
    let public_key = pubkey["public_key"].as_str().unwrap();

    // Sign a message
    let message = "Hello, Bitcoin!";
    let sign_output = workspace
        .command()
        .args(["btc-sign", "--message", message])
        .output()
        .expect("btc-sign failed");

    assert!(sign_output.status.success(), "btc-sign should succeed");

    let sign_stdout = String::from_utf8_lossy(&sign_output.stdout);
    let sign_json = extract_json(&sign_stdout).expect("No JSON from btc-sign");
    let sign_result: serde_json::Value = serde_json::from_str(&sign_json).unwrap();
    let signature = sign_result["signature"].as_str().unwrap();

    // Verify the signature
    let verify_output = workspace
        .command()
        .args([
            "btc-verify",
            "--signature",
            signature,
            "--public-key",
            public_key,
            "--message",
            message,
        ])
        .output()
        .expect("btc-verify failed");

    assert!(verify_output.status.success(), "btc-verify should succeed");

    let verify_stdout = String::from_utf8_lossy(&verify_output.stdout);
    let verify_json = extract_json(&verify_stdout);

    // Check either JSON result or text output
    if let Some(json) = verify_json {
        let result: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(result["valid"], true, "Signature should be valid");
    } else {
        assert!(
            verify_stdout.to_lowercase().contains("valid")
                && !verify_stdout.to_lowercase().contains("invalid"),
            "Signature should be valid. Output: {}",
            verify_stdout
        );
    }
}

/// Test signature verification with wrong message fails
#[test]
#[serial]
fn test_btc_verify_wrong_message() {
    let workspace = TestWorkspace::new("btc-wrong-message");
    // Generate key
    workspace
        .command()
        .args(["btc-keygen"])
        .output()
        .expect("btc-keygen failed");

    // Get public key
    let pubkey_output = workspace
        .command()
        .args(["btc-pubkey"])
        .output()
        .expect("btc-pubkey failed");

    let pubkey_stdout = String::from_utf8_lossy(&pubkey_output.stdout);
    let pubkey_json = extract_json(&pubkey_stdout).unwrap();
    let pubkey: serde_json::Value = serde_json::from_str(&pubkey_json).unwrap();
    let public_key = pubkey["public_key"].as_str().unwrap();

    // Sign a message
    let sign_output = workspace
        .command()
        .args(["btc-sign", "--message", "Original message"])
        .output()
        .expect("btc-sign failed");

    let sign_stdout = String::from_utf8_lossy(&sign_output.stdout);
    let sign_json = extract_json(&sign_stdout).unwrap();
    let sign_result: serde_json::Value = serde_json::from_str(&sign_json).unwrap();
    let signature = sign_result["signature"].as_str().unwrap();

    // Verify with wrong message
    let verify_output = workspace
        .command()
        .args([
            "btc-verify",
            "--signature",
            signature,
            "--public-key",
            public_key,
            "--message",
            "Different message",
        ])
        .output()
        .expect("btc-verify failed");

    let verify_stdout = String::from_utf8_lossy(&verify_output.stdout);
    let verify_json = extract_json(&verify_stdout);

    // Check either JSON result or text output
    if let Some(json) = verify_json {
        let result: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(
            result["valid"], false,
            "Signature should be invalid for wrong message"
        );
    } else {
        assert!(
            verify_stdout.to_lowercase().contains("invalid")
                || verify_stdout.to_lowercase().contains("false"),
            "Signature should be invalid for wrong message. Output: {}",
            verify_stdout
        );
    }
}

/// Test group info regeneration
#[test]
#[serial]
fn test_dkg_info() {
    let workspace = TestWorkspace::new("dkg-info");
    let prefix = get_unique_prefix();
    let (wallet1, _, _) = create_2_of_3_wallet(&workspace, &prefix);

    // Regenerate group info
    let output = workspace
        .command()
        .args(["dkg-info", "--name", &wallet1])
        .output()
        .expect("dkg-info failed");

    assert!(output.status.success(), "dkg-info should succeed");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("group_info") || stdout.contains("Group"),
        "Should mention group_info. Output: {}",
        stdout
    );

    // Verify file exists
    let info_path = workspace.state_path(format!("{wallet1}/group_info.json"));
    assert!(info_path.exists(), "group_info.json should exist");

    // Verify content
    let content = fs::read_to_string(&info_path).expect("Failed to read group_info.json");
    let info: serde_json::Value = serde_json::from_str(&content).unwrap();

    assert_eq!(info["threshold"], 2);
    assert_eq!(info["total_parties"], 3);
}

/// Test address generation for different networks
#[test]
#[serial]
fn test_address_networks() {
    let workspace = TestWorkspace::new("address-networks");
    // Generate key first
    workspace
        .command()
        .args(["btc-keygen"])
        .output()
        .expect("btc-keygen failed");

    // Mainnet
    let mainnet = workspace
        .command()
        .args(["btc-address"])
        .output()
        .expect("btc-address failed");
    let mainnet_stdout = String::from_utf8_lossy(&mainnet.stdout);
    assert!(
        mainnet_stdout.contains("bc1p"),
        "Mainnet address should start with bc1p. Got: {}",
        mainnet_stdout
    );

    // Testnet
    let testnet = workspace
        .command()
        .args(["btc-address-testnet"])
        .output()
        .expect("btc-address-testnet failed");
    let testnet_stdout = String::from_utf8_lossy(&testnet.stdout);
    assert!(
        testnet_stdout.contains("tb1p"),
        "Testnet address should start with tb1p. Got: {}",
        testnet_stdout
    );

    // Signet
    let signet = workspace
        .command()
        .args(["btc-address-signet"])
        .output()
        .expect("btc-address-signet failed");
    let signet_stdout = String::from_utf8_lossy(&signet.stdout);
    assert!(
        signet_stdout.contains("tb1p"),
        "Signet address should start with tb1p. Got: {}",
        signet_stdout
    );
}
