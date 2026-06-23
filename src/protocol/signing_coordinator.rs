//! Signing coordination state shared by local and relay-backed flows.
//!
//! This module owns protocol-level signing attempt state. Transports such as
//! Nostr should validate and decrypt messages, then map them into these inputs.

use anyhow::{bail, Result};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SigningNonceInput {
    pub wallet: String,
    pub session: String,
    pub attempt_id: String,
    pub signer_set: Vec<u32>,
    pub party_index: u32,
    pub sighash_fingerprint: String,
    pub public_nonce: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SigningShareInput {
    pub wallet: String,
    pub session: String,
    pub attempt_id: String,
    pub signer_set: Vec<u32>,
    pub party_index: u32,
    pub sighash_fingerprint: String,
    pub signature_share: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SigningAttemptCollector {
    wallet: String,
    session: String,
    attempt_id: String,
    signer_set: Vec<u32>,
    threshold: u32,
    sighash_fingerprint: String,
    nonces: BTreeMap<u32, String>,
    shares: BTreeMap<u32, String>,
}

impl SigningAttemptCollector {
    pub fn new(
        wallet: impl Into<String>,
        session: impl Into<String>,
        attempt_id: impl Into<String>,
        signer_set: Vec<u32>,
        threshold: u32,
        sighash_fingerprint: impl Into<String>,
    ) -> Result<Self> {
        let wallet = wallet.into();
        let session = session.into();
        let attempt_id = attempt_id.into();
        let sighash_fingerprint = sighash_fingerprint.into();

        if wallet.trim().is_empty() {
            bail!("wallet cannot be empty");
        }
        if session.trim().is_empty() {
            bail!("session cannot be empty");
        }
        if attempt_id.trim().is_empty() {
            bail!("attempt_id cannot be empty");
        }
        if threshold == 0 {
            bail!("threshold must be nonzero");
        }
        if signer_set.is_empty() {
            bail!("signer_set cannot be empty");
        }
        if threshold as usize > signer_set.len() {
            bail!("threshold cannot exceed signer_set length");
        }
        if signer_set.contains(&0) {
            bail!("signer_set contains party index 0");
        }
        let unique: BTreeSet<u32> = signer_set.iter().copied().collect();
        if unique.len() != signer_set.len() {
            bail!("signer_set contains duplicate parties");
        }
        if sighash_fingerprint.trim().is_empty() {
            bail!("sighash_fingerprint cannot be empty");
        }

        Ok(Self {
            wallet,
            session,
            attempt_id,
            signer_set,
            threshold,
            sighash_fingerprint,
            nonces: BTreeMap::new(),
            shares: BTreeMap::new(),
        })
    }

    pub fn accept_nonce(&mut self, input: impl Into<SigningNonceInput>) -> Result<bool> {
        let input = input.into();
        self.validate_input_context(
            &input.wallet,
            &input.session,
            &input.attempt_id,
            &input.signer_set,
            input.party_index,
            &input.sighash_fingerprint,
        )?;
        if input.public_nonce.trim().is_empty() {
            bail!("public_nonce cannot be empty");
        }

        Ok(self
            .nonces
            .insert(input.party_index, input.public_nonce)
            .is_none())
    }

    pub fn accept_share(&mut self, input: impl Into<SigningShareInput>) -> Result<bool> {
        let input = input.into();
        self.validate_input_context(
            &input.wallet,
            &input.session,
            &input.attempt_id,
            &input.signer_set,
            input.party_index,
            &input.sighash_fingerprint,
        )?;
        if input.signature_share.trim().is_empty() {
            bail!("signature_share cannot be empty");
        }
        if !self.nonces.contains_key(&input.party_index) {
            bail!("signature share received before nonce for party");
        }

        Ok(self
            .shares
            .insert(input.party_index, input.signature_share)
            .is_none())
    }

    pub fn nonce_count(&self) -> usize {
        self.nonces.len()
    }

    pub fn share_count(&self) -> usize {
        self.shares.len()
    }

    pub fn has_nonce_threshold(&self) -> bool {
        self.nonce_count() >= self.threshold as usize
    }

    pub fn has_share_threshold(&self) -> bool {
        self.share_count() >= self.threshold as usize
    }

    pub fn nonces(&self) -> &BTreeMap<u32, String> {
        &self.nonces
    }

    pub fn shares(&self) -> &BTreeMap<u32, String> {
        &self.shares
    }

    fn validate_input_context(
        &self,
        wallet: &str,
        session: &str,
        attempt_id: &str,
        signer_set: &[u32],
        party_index: u32,
        sighash_fingerprint: &str,
    ) -> Result<()> {
        if wallet != self.wallet {
            bail!("signing input wallet does not match active attempt");
        }
        if session != self.session {
            bail!("signing input session does not match active attempt");
        }
        if attempt_id != self.attempt_id {
            bail!("signing input attempt_id does not match active attempt");
        }
        if signer_set != self.signer_set.as_slice() {
            bail!("signing input signer_set does not match active attempt");
        }
        if !self.signer_set.contains(&party_index) {
            bail!("signing input sender is not in active signer_set");
        }
        if sighash_fingerprint != self.sighash_fingerprint {
            bail!("signing input sighash_fingerprint does not match active attempt");
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn collector() -> SigningAttemptCollector {
        SigningAttemptCollector::new(
            "treasury",
            "session-a",
            "attempt-1",
            vec![1, 2, 3],
            2,
            "001122334455...aabbccddeeff",
        )
        .unwrap()
    }

    fn nonce(party_index: u32) -> SigningNonceInput {
        SigningNonceInput {
            wallet: "treasury".to_string(),
            session: "session-a".to_string(),
            attempt_id: "attempt-1".to_string(),
            signer_set: vec![1, 2, 3],
            party_index,
            sighash_fingerprint: "001122334455...aabbccddeeff".to_string(),
            public_nonce: format!("nonce-{party_index}"),
        }
    }

    fn share(party_index: u32) -> SigningShareInput {
        SigningShareInput {
            wallet: "treasury".to_string(),
            session: "session-a".to_string(),
            attempt_id: "attempt-1".to_string(),
            signer_set: vec![1, 2, 3],
            party_index,
            sighash_fingerprint: "001122334455...aabbccddeeff".to_string(),
            signature_share: format!("share-{party_index}"),
        }
    }

    #[test]
    fn signing_attempt_collector_tracks_threshold_progress() {
        let mut attempt = collector();

        assert!(attempt.accept_nonce(nonce(1)).unwrap());
        assert!(attempt.accept_nonce(nonce(2)).unwrap());
        assert!(!attempt.accept_nonce(nonce(2)).unwrap());
        assert_eq!(attempt.nonce_count(), 2);
        assert!(attempt.has_nonce_threshold());
        assert_eq!(attempt.nonces().get(&1).unwrap(), "nonce-1");

        assert!(attempt.accept_share(share(1)).unwrap());
        assert!(attempt.accept_share(share(2)).unwrap());
        assert_eq!(attempt.share_count(), 2);
        assert!(attempt.has_share_threshold());
        assert_eq!(attempt.shares().get(&2).unwrap(), "share-2");
    }

    #[test]
    fn signing_attempt_collector_rejects_wrong_attempt_context() {
        let mut attempt = collector();

        let mut wrong_wallet = nonce(1);
        wrong_wallet.wallet = "other".to_string();
        assert!(attempt.accept_nonce(wrong_wallet).is_err());

        let mut wrong_session = nonce(1);
        wrong_session.session = "other-session".to_string();
        assert!(attempt.accept_nonce(wrong_session).is_err());

        let mut wrong_attempt = nonce(1);
        wrong_attempt.attempt_id = "attempt-2".to_string();
        assert!(attempt.accept_nonce(wrong_attempt).is_err());

        let mut wrong_signers = nonce(1);
        wrong_signers.signer_set = vec![1, 3, 2];
        assert!(attempt.accept_nonce(wrong_signers).is_err());

        let mut wrong_fingerprint = nonce(1);
        wrong_fingerprint.sighash_fingerprint = "bad".to_string();
        assert!(attempt.accept_nonce(wrong_fingerprint).is_err());

        let outsider = nonce(4);
        assert!(attempt.accept_nonce(outsider).is_err());
    }

    #[test]
    fn signing_attempt_collector_requires_nonce_before_share() {
        let mut attempt = collector();

        assert!(attempt.accept_share(share(1)).is_err());
        attempt.accept_nonce(nonce(1)).unwrap();
        assert!(attempt.accept_share(share(1)).unwrap());
    }

    #[test]
    fn signing_attempt_collector_rejects_invalid_configuration() {
        assert!(
            SigningAttemptCollector::new("treasury", "session", "attempt", vec![1], 2, "fp")
                .is_err()
        );
        assert!(SigningAttemptCollector::new(
            "treasury",
            "session",
            "attempt",
            vec![1, 1],
            1,
            "fp"
        )
        .is_err());
        assert!(SigningAttemptCollector::new(
            "treasury",
            "session",
            "attempt",
            vec![0, 1],
            1,
            "fp"
        )
        .is_err());
    }
}
