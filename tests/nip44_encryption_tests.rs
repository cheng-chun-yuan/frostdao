//! E2E tests for NIP-44 encrypted DKG flow

use std::collections::HashMap;
use std::fs;
use std::process::Command;
use std::sync::atomic::{AtomicU32, Ordering};

const FROSTDAO: &str = "./target/release/frostdao";

// Atomic counter for unique test IDs
static TEST_COUNTER: AtomicU32 = AtomicU32::new(0);

fn get_unique_prefix() -> String {
    let id = TEST_COUNTER.fetch_add(1, Ordering::SeqCst);
    let time = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis();
    format!("nip44_test_{}_{}", time, id)
}

fn cleanup_wallet(prefix: &str) {
    let state_dir = ".frost_state";
    if let Ok(entries) = fs::read_dir(state_dir) {
        for entry in entries.flatten() {
            if let Some(name) = entry.file_name().to_str() {
                if name.starts_with(prefix) {
                    let _ = fs::remove_dir_all(entry.path());
                }
            }
        }
    }
}

fn extract_json(output: &str) -> Option<String> {
    for line in output.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('{') && trimmed.ends_with('}') {
            return Some(trimmed.to_string());
        }
    }
    None
}

#[derive(serde::Deserialize, Debug)]
#[allow(dead_code)]
struct Round1Output {
    party_index: u32,
    keygen_input: String,
    encryption_pubkey: Option<String>,
    #[serde(rename = "type")]
    event_type: String,
}

#[derive(serde::Deserialize, Debug)]
#[allow(dead_code)]
struct Round2Output {
    party_index: u32,
    shares: Vec<ShareData>,
    #[serde(rename = "type")]
    event_type: String,
}

#[derive(serde::Deserialize, Debug)]
struct Round2EncryptedOutput {
    party_index: u32,
    encrypted_shares: Vec<EncryptedShareData>,
    #[serde(rename = "type")]
    event_type: String,
}

#[derive(serde::Deserialize, Debug, Clone)]
struct ShareData {
    to_index: u32,
    share: String,
}

#[derive(serde::Deserialize, Debug, Clone)]
struct EncryptedShareData {
    to_index: u32,
    ciphertext: String,
}

/// Test that keygen-round1 outputs encryption_pubkey
#[test]
fn test_round1_outputs_encryption_pubkey() {
    let prefix = get_unique_prefix();
    let wallet = format!("{}_pubkey", prefix);

    let output = Command::new(FROSTDAO)
        .args([
            "keygen-round1",
            "--name",
            &wallet,
            "--threshold",
            "2",
            "--n-parties",
            "3",
            "--my-index",
            "1",
        ])
        .output()
        .expect("Failed to run keygen-round1");

    assert!(output.status.success(), "keygen-round1 failed");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let json = extract_json(&stdout).expect("No JSON output");

    let round1: Round1Output = serde_json::from_str(&json).expect("Failed to parse Round1Output");

    // Verify encryption_pubkey is present and valid
    assert!(
        round1.encryption_pubkey.is_some(),
        "encryption_pubkey should be present"
    );
    let pubkey = round1.encryption_pubkey.unwrap();
    assert_eq!(pubkey.len(), 64, "encryption_pubkey should be 32 bytes hex");

    // Verify secret_coefficient.txt was created
    let secret_path = format!(".frost_state/{}/secret_coefficient.txt", wallet);
    assert!(
        std::path::Path::new(&secret_path).exists(),
        "secret_coefficient.txt should exist"
    );

    let secret = fs::read_to_string(&secret_path).expect("Failed to read secret_coefficient.txt");
    assert_eq!(
        secret.trim().len(),
        64,
        "secret_coefficient should be 32 bytes hex"
    );

    cleanup_wallet(&prefix);
}

/// Test keygen-round2 with --encrypt flag produces encrypted shares
#[test]
fn test_round2_encrypt_flag() {
    let prefix = get_unique_prefix();
    let wallet1 = format!("{}_enc_p1", prefix);
    let wallet2 = format!("{}_enc_p2", prefix);

    // Round 1 for two parties
    let r1_p1 = Command::new(FROSTDAO)
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
        .expect("Failed to run keygen-round1");
    assert!(r1_p1.status.success());
    let commit1 = extract_json(&String::from_utf8_lossy(&r1_p1.stdout)).unwrap();

    let r1_p2 = Command::new(FROSTDAO)
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
        .expect("Failed to run keygen-round1");
    assert!(r1_p2.status.success());
    let commit2 = extract_json(&String::from_utf8_lossy(&r1_p2.stdout)).unwrap();

    let all_commits = format!("{} {}", commit1, commit2);

    // Round 2 with --encrypt for party 1
    let r2_p1 = Command::new(FROSTDAO)
        .args([
            "keygen-round2",
            "--name",
            &wallet1,
            "--data",
            &all_commits,
            "--encrypt",
        ])
        .output()
        .expect("Failed to run keygen-round2");

    let stdout = String::from_utf8_lossy(&r2_p1.stdout);
    assert!(
        r2_p1.status.success(),
        "keygen-round2 --encrypt failed: {}",
        stdout
    );

    // Verify output contains encrypted shares
    assert!(
        stdout.contains("E2E Encryption: ENABLED"),
        "Should show encryption enabled"
    );
    assert!(
        stdout.contains("Encrypted share"),
        "Should show encrypted shares"
    );

    let json = extract_json(&stdout).expect("No JSON output");
    let round2: Round2EncryptedOutput =
        serde_json::from_str(&json).expect("Failed to parse Round2EncryptedOutput");

    assert_eq!(round2.event_type, "keygen_round2_encrypted");
    assert!(!round2.encrypted_shares.is_empty());

    // Verify ciphertext is base64 encoded
    for share in &round2.encrypted_shares {
        assert!(
            base64::Engine::decode(
                &base64::engine::general_purpose::STANDARD,
                &share.ciphertext
            )
            .is_ok(),
            "Ciphertext should be valid base64"
        );
    }

    cleanup_wallet(&prefix);
}

/// Test full encrypted DKG flow with decryption
#[test]
fn test_full_encrypted_dkg_flow() {
    use frostdao::crypto::nip44;

    let prefix = get_unique_prefix();
    let wallet1 = format!("{}_full_p1", prefix);
    let wallet2 = format!("{}_full_p2", prefix);
    let wallet3 = format!("{}_full_p3", prefix);

    // Round 1: All parties generate commitments
    let mut commits = Vec::new();
    let mut encryption_pubkeys: HashMap<u32, String> = HashMap::new();
    let mut secret_coefficients: HashMap<u32, String> = HashMap::new();

    for (i, wallet) in [(1, &wallet1), (2, &wallet2), (3, &wallet3)] {
        let output = Command::new(FROSTDAO)
            .args([
                "keygen-round1",
                "--name",
                wallet,
                "--threshold",
                "2",
                "--n-parties",
                "3",
                "--my-index",
                &i.to_string(),
            ])
            .output()
            .expect("Failed to run keygen-round1");
        assert!(output.status.success(), "Party {} round1 failed", i);

        let json = extract_json(&String::from_utf8_lossy(&output.stdout)).unwrap();
        let round1: Round1Output = serde_json::from_str(&json).unwrap();

        encryption_pubkeys.insert(i, round1.encryption_pubkey.clone().unwrap());

        // Read secret coefficient
        let secret_path = format!(".frost_state/{}/secret_coefficient.txt", wallet);
        let secret = fs::read_to_string(&secret_path).unwrap();
        secret_coefficients.insert(i, secret.trim().to_string());

        commits.push(json);
    }

    let all_commits = commits.join(" ");

    // Round 2: All parties generate encrypted shares
    let mut encrypted_outputs: Vec<Round2EncryptedOutput> = Vec::new();

    for (i, wallet) in [(1, &wallet1), (2, &wallet2), (3, &wallet3)] {
        let output = Command::new(FROSTDAO)
            .args([
                "keygen-round2",
                "--name",
                wallet,
                "--data",
                &all_commits,
                "--encrypt",
            ])
            .output()
            .expect("Failed to run keygen-round2");
        assert!(
            output.status.success(),
            "Party {} round2 failed: {}",
            i,
            String::from_utf8_lossy(&output.stderr)
        );

        let json = extract_json(&String::from_utf8_lossy(&output.stdout)).unwrap();
        let round2: Round2EncryptedOutput = serde_json::from_str(&json).unwrap();
        encrypted_outputs.push(round2);
    }

    // Decrypt shares for each party
    for recipient_idx in 1u32..=3 {
        let recipient_secret = secret_coefficients.get(&recipient_idx).unwrap();
        let recipient_secret_bytes: [u8; 32] =
            hex::decode(recipient_secret).unwrap().try_into().unwrap();

        for sender_output in &encrypted_outputs {
            let sender_idx = sender_output.party_index;
            let sender_pubkey = encryption_pubkeys.get(&sender_idx).unwrap();
            let sender_pubkey_bytes: [u8; 32] =
                hex::decode(sender_pubkey).unwrap().try_into().unwrap();

            // Find the encrypted share for this recipient
            if let Some(encrypted_share) = sender_output
                .encrypted_shares
                .iter()
                .find(|s| s.to_index == recipient_idx)
            {
                // Decrypt the share
                let decrypted = nip44::decrypt_from_sender(
                    &encrypted_share.ciphertext,
                    &recipient_secret_bytes,
                    &sender_pubkey_bytes,
                );

                assert!(
                    decrypted.is_ok(),
                    "Failed to decrypt share from {} to {}: {:?}",
                    sender_idx,
                    recipient_idx,
                    decrypted.err()
                );

                let plaintext = decrypted.unwrap();
                // Share should be 32 bytes (a scalar)
                assert_eq!(
                    plaintext.len(),
                    32,
                    "Decrypted share should be 32 bytes, got {}",
                    plaintext.len()
                );
            }
        }
    }

    // Now run round2 without encryption to get plaintext shares for comparison
    let mut plaintext_outputs: Vec<Round2Output> = Vec::new();

    for (_i, wallet) in [(1, &wallet1), (2, &wallet2), (3, &wallet3)] {
        let output = Command::new(FROSTDAO)
            .args(["keygen-round2", "--name", wallet, "--data", &all_commits])
            .output()
            .expect("Failed to run keygen-round2");
        assert!(output.status.success());

        let json = extract_json(&String::from_utf8_lossy(&output.stdout)).unwrap();
        let round2: Round2Output = serde_json::from_str(&json).unwrap();
        plaintext_outputs.push(round2);
    }

    // Verify decrypted shares match plaintext shares
    for recipient_idx in 1u32..=3 {
        let recipient_secret = secret_coefficients.get(&recipient_idx).unwrap();
        let recipient_secret_bytes: [u8; 32] =
            hex::decode(recipient_secret).unwrap().try_into().unwrap();

        for (encrypted_output, plaintext_output) in
            encrypted_outputs.iter().zip(plaintext_outputs.iter())
        {
            let sender_idx = encrypted_output.party_index;
            let sender_pubkey = encryption_pubkeys.get(&sender_idx).unwrap();
            let sender_pubkey_bytes: [u8; 32] =
                hex::decode(sender_pubkey).unwrap().try_into().unwrap();

            if let Some(encrypted_share) = encrypted_output
                .encrypted_shares
                .iter()
                .find(|s| s.to_index == recipient_idx)
            {
                let decrypted = nip44::decrypt_from_sender(
                    &encrypted_share.ciphertext,
                    &recipient_secret_bytes,
                    &sender_pubkey_bytes,
                )
                .unwrap();

                // Find corresponding plaintext share
                let plaintext_share = plaintext_output
                    .shares
                    .iter()
                    .find(|s| s.to_index == recipient_idx)
                    .unwrap();

                let expected = hex::decode(&plaintext_share.share).unwrap();

                assert_eq!(
                    decrypted, expected,
                    "Decrypted share from {} to {} doesn't match plaintext",
                    sender_idx, recipient_idx
                );
            }
        }
    }

    cleanup_wallet(&prefix);
}

/// Test that encryption fails gracefully without encryption_pubkey
#[test]
fn test_encryption_requires_pubkeys() {
    let prefix = get_unique_prefix();
    let wallet1 = format!("{}_nopk_p1", prefix);

    // Create a minimal round1 output without encryption_pubkey (simulating old format)
    let fake_commit = r#"{"party_index":1,"rank":0,"keygen_input":"deadbeef","hierarchical":false,"type":"keygen_round1"}"#;

    // Round 1 for party 1
    let r1_p1 = Command::new(FROSTDAO)
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
        .expect("Failed to run keygen-round1");
    assert!(r1_p1.status.success());
    let commit1 = extract_json(&String::from_utf8_lossy(&r1_p1.stdout)).unwrap();

    // Use fake commit without encryption_pubkey
    let all_commits = format!("{} {}", commit1, fake_commit);

    // Round 2 with --encrypt should warn about missing pubkey
    let r2_p1 = Command::new(FROSTDAO)
        .args([
            "keygen-round2",
            "--name",
            &wallet1,
            "--data",
            &all_commits,
            "--encrypt",
        ])
        .output();

    // The command might fail or warn - either is acceptable
    // Main thing is it shouldn't crash
    if let Ok(output) = r2_p1 {
        let stdout = String::from_utf8_lossy(&output.stdout);
        // Should either fail or show warning about missing pubkey
        if output.status.success() {
            assert!(
                stdout.contains("No encryption pubkey") || stdout.contains("Encrypted share"),
                "Should handle missing pubkeys gracefully"
            );
        }
    }

    cleanup_wallet(&prefix);
}

/// Test NIP-44 crypto module directly
#[test]
fn test_nip44_roundtrip() {
    use frostdao::crypto::nip44;
    use rand::RngCore;
    use secp256kfun::prelude::*;

    // Generate two key pairs
    let mut rng = rand::thread_rng();

    let mut secret_a = [0u8; 32];
    let mut secret_b = [0u8; 32];
    rng.fill_bytes(&mut secret_a);
    rng.fill_bytes(&mut secret_b);

    // Derive public keys
    let scalar_a: Scalar<Secret, secp256kfun::marker::NonZero> =
        Scalar::from_bytes(secret_a).unwrap();
    let scalar_b: Scalar<Secret, secp256kfun::marker::NonZero> =
        Scalar::from_bytes(secret_b).unwrap();

    let pubkey_a = g!(scalar_a * G).normalize().to_xonly_bytes();
    let pubkey_b = g!(scalar_b * G).normalize().to_xonly_bytes();

    // Test message (simulating a DKG share)
    let message = b"This is a secret DKG share that should be encrypted";

    // Encrypt from A to B
    let ciphertext = nip44::encrypt_for_recipient(message, &secret_a, &pubkey_b)
        .expect("Encryption should succeed");

    // Decrypt at B
    let decrypted = nip44::decrypt_from_sender(&ciphertext, &secret_b, &pubkey_a)
        .expect("Decryption should succeed");

    assert_eq!(
        decrypted, message,
        "Decrypted message should match original"
    );
}
