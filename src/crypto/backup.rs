//! Hardened backup metadata for FrostDAO secret-share mnemonics.
//!
//! The mnemonic remains the secret backup material. The manifest is public
//! metadata that lets a user verify the mnemonic belongs to the intended wallet
//! and party before storing or using it.

use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const BACKUP_FORMAT: &str = "frostdao-share-backup";
pub const BACKUP_VERSION: u16 = 1;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BackupWalletMetadata {
    pub wallet_name: String,
    pub party_index: u32,
    pub rank: u32,
    pub threshold: u32,
    pub total_parties: u32,
    pub hierarchical: bool,
    pub group_public_key: String,
    pub shared_key_polynomial: String,
    pub taproot_address_testnet: String,
    pub taproot_address_mainnet: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BackupManifest {
    pub format: String,
    pub version: u16,
    pub wallet_name: String,
    pub party_index: u32,
    pub rank: u32,
    pub threshold: u32,
    pub total_parties: u32,
    pub hierarchical: bool,
    pub group_public_key: String,
    pub shared_key_polynomial: String,
    pub taproot_address_testnet: String,
    pub taproot_address_mainnet: String,
    pub share_fingerprint: String,
    pub backup_id: String,
    pub warning: String,
}

impl BackupManifest {
    pub fn validate(&self) -> Result<()> {
        if self.format != BACKUP_FORMAT {
            bail!("unsupported backup format '{}'", self.format);
        }
        if self.version != BACKUP_VERSION {
            bail!("unsupported backup version {}", self.version);
        }
        if self.wallet_name.trim().is_empty() {
            bail!("wallet name cannot be empty");
        }
        if self.party_index == 0 {
            bail!("party index must be nonzero");
        }
        if self.threshold == 0 {
            bail!("threshold must be nonzero");
        }
        if self.total_parties == 0 {
            bail!("total parties must be nonzero");
        }
        if self.threshold > self.total_parties {
            bail!("threshold cannot exceed total parties");
        }
        if self.group_public_key.len() != 64 || hex::decode(&self.group_public_key).is_err() {
            bail!("group public key must be 32-byte hex");
        }
        validate_shared_key_polynomial(&self.shared_key_polynomial, &self.group_public_key)?;
        if self.share_fingerprint.len() != 64 || hex::decode(&self.share_fingerprint).is_err() {
            bail!("share fingerprint must be 32-byte hex");
        }
        if self.backup_id.len() != 32 || hex::decode(&self.backup_id).is_err() {
            bail!("backup id must be 16-byte hex");
        }
        Ok(())
    }
}

pub fn build_backup_manifest(
    metadata: BackupWalletMetadata,
    share_bytes: &[u8; 32],
) -> Result<BackupManifest> {
    validate_share_bytes(share_bytes)?;

    let share_fingerprint = share_fingerprint(
        share_bytes,
        &metadata.group_public_key,
        metadata.party_index,
        metadata.rank,
    );
    let backup_id = backup_id(&metadata, &share_fingerprint);

    let manifest = BackupManifest {
        format: BACKUP_FORMAT.to_string(),
        version: BACKUP_VERSION,
        wallet_name: metadata.wallet_name,
        party_index: metadata.party_index,
        rank: metadata.rank,
        threshold: metadata.threshold,
        total_parties: metadata.total_parties,
        hierarchical: metadata.hierarchical,
        group_public_key: metadata.group_public_key,
        shared_key_polynomial: metadata.shared_key_polynomial,
        taproot_address_testnet: metadata.taproot_address_testnet,
        taproot_address_mainnet: metadata.taproot_address_mainnet,
        share_fingerprint,
        backup_id,
        warning:
            "This manifest is public metadata. The 24-word mnemonic is the secret share backup."
                .to_string(),
    };
    manifest.validate()?;
    Ok(manifest)
}

pub fn verify_share_against_manifest(
    share_bytes: &[u8; 32],
    manifest: &BackupManifest,
) -> Result<()> {
    validate_share_bytes(share_bytes)?;
    manifest.validate()?;

    let computed = share_fingerprint(
        share_bytes,
        &manifest.group_public_key,
        manifest.party_index,
        manifest.rank,
    );

    if computed != manifest.share_fingerprint {
        bail!("mnemonic share does not match backup manifest");
    }

    let metadata = BackupWalletMetadata {
        wallet_name: manifest.wallet_name.clone(),
        party_index: manifest.party_index,
        rank: manifest.rank,
        threshold: manifest.threshold,
        total_parties: manifest.total_parties,
        hierarchical: manifest.hierarchical,
        group_public_key: manifest.group_public_key.clone(),
        shared_key_polynomial: manifest.shared_key_polynomial.clone(),
        taproot_address_testnet: manifest.taproot_address_testnet.clone(),
        taproot_address_mainnet: manifest.taproot_address_mainnet.clone(),
    };
    let expected_backup_id = backup_id(&metadata, &manifest.share_fingerprint);
    if expected_backup_id != manifest.backup_id {
        bail!("backup id does not match manifest contents");
    }

    Ok(())
}

fn validate_share_bytes(share_bytes: &[u8; 32]) -> Result<()> {
    if share_bytes.iter().all(|byte| *byte == 0) {
        bail!("share cannot be all zero");
    }
    Ok(())
}

fn validate_shared_key_polynomial(
    shared_key_polynomial: &str,
    group_public_key: &str,
) -> Result<()> {
    let bytes = hex::decode(shared_key_polynomial)
        .map_err(|_| anyhow::anyhow!("shared key polynomial must be hex"))?;
    if bytes.is_empty() || bytes.len() % 33 != 0 {
        bail!("shared key polynomial must contain compressed public coefficients");
    }
    if bytes[0] != 0x02 {
        bail!("shared key polynomial must start with the even-y group public key");
    }
    if hex::encode(&bytes[1..33]) != group_public_key {
        bail!("shared key polynomial does not match group public key");
    }
    Ok(())
}

fn share_fingerprint(
    share_bytes: &[u8; 32],
    group_public_key: &str,
    party_index: u32,
    rank: u32,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"FrostDAO/share-backup-fingerprint/v1");
    hasher.update(group_public_key.as_bytes());
    hasher.update(party_index.to_be_bytes());
    hasher.update(rank.to_be_bytes());
    hasher.update(share_bytes);
    hex::encode(hasher.finalize())
}

fn backup_id(metadata: &BackupWalletMetadata, share_fingerprint: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"FrostDAO/share-backup-id/v1");
    hasher.update(metadata.wallet_name.as_bytes());
    hasher.update(metadata.group_public_key.as_bytes());
    hasher.update(metadata.party_index.to_be_bytes());
    hasher.update(metadata.rank.to_be_bytes());
    hasher.update(metadata.threshold.to_be_bytes());
    hasher.update(metadata.total_parties.to_be_bytes());
    hasher.update([metadata.hierarchical as u8]);
    hasher.update(share_fingerprint.as_bytes());
    let digest = hasher.finalize();
    hex::encode(&digest[..16])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn metadata() -> BackupWalletMetadata {
        BackupWalletMetadata {
            wallet_name: "treasury".to_string(),
            party_index: 2,
            rank: 1,
            threshold: 2,
            total_parties: 3,
            hierarchical: true,
            group_public_key: "11".repeat(32),
            shared_key_polynomial: format!("02{}", "11".repeat(32)),
            taproot_address_testnet: "tb1ptest".to_string(),
            taproot_address_mainnet: "bc1ptest".to_string(),
        }
    }

    #[test]
    fn manifest_roundtrip_verifies_share() {
        let share = [0x42u8; 32];
        let manifest = build_backup_manifest(metadata(), &share).unwrap();

        assert_eq!(manifest.format, BACKUP_FORMAT);
        assert_eq!(manifest.version, BACKUP_VERSION);
        assert!(manifest.validate().is_ok());
        assert!(verify_share_against_manifest(&share, &manifest).is_ok());
    }

    #[test]
    fn manifest_rejects_wrong_share() {
        let manifest = build_backup_manifest(metadata(), &[0x42u8; 32]).unwrap();
        let err = verify_share_against_manifest(&[0x43u8; 32], &manifest).unwrap_err();
        assert!(err.to_string().contains("does not match"));
    }

    #[test]
    fn manifest_rejects_tampered_backup_id() {
        let share = [0x42u8; 32];
        let mut manifest = build_backup_manifest(metadata(), &share).unwrap();
        manifest.backup_id = "00".repeat(16);
        let err = verify_share_against_manifest(&share, &manifest).unwrap_err();
        assert!(err.to_string().contains("backup id"));
    }

    #[test]
    fn manifest_rejects_zero_share() {
        assert!(build_backup_manifest(metadata(), &[0u8; 32]).is_err());
    }

    #[test]
    fn manifest_rejects_mismatched_shared_key_polynomial() {
        let mut metadata = metadata();
        metadata.shared_key_polynomial = format!("02{}", "22".repeat(32));

        let err = build_backup_manifest(metadata, &[0x42u8; 32]).unwrap_err();
        assert!(err
            .to_string()
            .contains("shared key polynomial does not match group public key"));
    }
}
