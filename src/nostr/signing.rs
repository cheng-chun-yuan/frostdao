//! Nostr signing payload helpers.
//!
//! Relay messages are accepted elsewhere. This module only encodes and decodes
//! typed encrypted plaintexts, then adapts them into protocol-level signing
//! coordinator inputs.

use anyhow::Result;

use super::events::{
    NostrProtocolMessage, SigningNonceEvent, SigningNoncePlaintext, SigningShareEvent,
    SigningSharePlaintext,
};
use crate::protocol::signing_coordinator::{SigningNonceInput, SigningShareInput};

pub fn encrypt_signing_nonce_plaintext(
    plaintext: &SigningNoncePlaintext,
    conversation_key: &[u8; 32],
) -> Result<String> {
    let data = serde_json::to_vec(plaintext)?;
    crate::crypto::nip44::encrypt(&data, conversation_key)
}

pub fn decrypt_signing_nonce_plaintext(
    ciphertext: &str,
    conversation_key: &[u8; 32],
    message: &NostrProtocolMessage,
    event: &SigningNonceEvent,
) -> Result<SigningNoncePlaintext> {
    let data = crate::crypto::nip44::decrypt(ciphertext, conversation_key)?;
    let plaintext: SigningNoncePlaintext = serde_json::from_slice(&data)?;
    plaintext.validate_for_envelope(message, event)?;
    Ok(plaintext)
}

pub fn encrypt_signing_share_plaintext(
    plaintext: &SigningSharePlaintext,
    conversation_key: &[u8; 32],
) -> Result<String> {
    let data = serde_json::to_vec(plaintext)?;
    crate::crypto::nip44::encrypt(&data, conversation_key)
}

pub fn decrypt_signing_share_plaintext(
    ciphertext: &str,
    conversation_key: &[u8; 32],
    message: &NostrProtocolMessage,
    event: &SigningShareEvent,
) -> Result<SigningSharePlaintext> {
    let data = crate::crypto::nip44::decrypt(ciphertext, conversation_key)?;
    let plaintext: SigningSharePlaintext = serde_json::from_slice(&data)?;
    plaintext.validate_for_envelope(message, event)?;
    Ok(plaintext)
}

impl From<SigningNoncePlaintext> for SigningNonceInput {
    fn from(plaintext: SigningNoncePlaintext) -> Self {
        Self {
            wallet: plaintext.wallet,
            session: plaintext.session,
            attempt_id: plaintext.attempt_id,
            signer_set: plaintext.signer_set,
            party_index: plaintext.party_index,
            sighash_fingerprint: plaintext.sighash_fingerprint,
            public_nonce: plaintext.public_nonce,
        }
    }
}

impl From<SigningSharePlaintext> for SigningShareInput {
    fn from(plaintext: SigningSharePlaintext) -> Self {
        Self {
            wallet: plaintext.wallet,
            session: plaintext.session,
            attempt_id: plaintext.attempt_id,
            signer_set: plaintext.signer_set,
            party_index: plaintext.party_index,
            sighash_fingerprint: plaintext.sighash_fingerprint,
            signature_share: plaintext.signature_share,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::nostr::{NostrMessageKind, NostrProtocolMessage};

    fn nonce(party_index: u32) -> SigningNoncePlaintext {
        SigningNoncePlaintext {
            wallet: "treasury".to_string(),
            session: "session-a".to_string(),
            attempt_id: "attempt-1".to_string(),
            signer_set: vec![1, 2, 3],
            party_index,
            to_index: 1,
            sighash_fingerprint: "001122334455...aabbccddeeff".to_string(),
            public_nonce: format!("nonce-{party_index}"),
        }
    }

    fn share(party_index: u32) -> SigningSharePlaintext {
        SigningSharePlaintext {
            wallet: "treasury".to_string(),
            session: "session-a".to_string(),
            attempt_id: "attempt-1".to_string(),
            signer_set: vec![1, 2, 3],
            party_index,
            to_index: 1,
            sighash_fingerprint: "001122334455...aabbccddeeff".to_string(),
            signature_share: format!("share-{party_index}"),
        }
    }

    #[test]
    fn signing_plaintext_helpers_encrypt_decrypt_and_validate() {
        let conversation_key = [7u8; 32];

        let nonce_plaintext = nonce(1);
        let nonce_ciphertext =
            encrypt_signing_nonce_plaintext(&nonce_plaintext, &conversation_key).unwrap();
        let nonce_event = SigningNonceEvent::new(1, 1, nonce_ciphertext.clone());
        let nonce_message = NostrProtocolMessage::new(
            "room-a",
            NostrMessageKind::SigningNonceEncrypted,
            1,
            &nonce_event,
        )
        .unwrap()
        .with_wallet("treasury")
        .with_session("session-a")
        .to_party(1)
        .unwrap();

        let decoded_nonce = decrypt_signing_nonce_plaintext(
            &nonce_ciphertext,
            &conversation_key,
            &nonce_message,
            &nonce_event,
        )
        .unwrap();
        assert_eq!(decoded_nonce, nonce_plaintext);

        let share_plaintext = share(1);
        let share_ciphertext =
            encrypt_signing_share_plaintext(&share_plaintext, &conversation_key).unwrap();
        let share_event = SigningShareEvent::new(1, 1, share_ciphertext.clone());
        let share_message = NostrProtocolMessage::new(
            "room-a",
            NostrMessageKind::SigningShareEncrypted,
            1,
            &share_event,
        )
        .unwrap()
        .with_wallet("treasury")
        .with_session("session-a")
        .to_party(1)
        .unwrap();

        let decoded_share = decrypt_signing_share_plaintext(
            &share_ciphertext,
            &conversation_key,
            &share_message,
            &share_event,
        )
        .unwrap();
        assert_eq!(decoded_share, share_plaintext);
    }

    #[test]
    fn signing_plaintext_helpers_reject_wrong_key_and_context() {
        let conversation_key = [7u8; 32];
        let wrong_key = [8u8; 32];

        let nonce_plaintext = nonce(1);
        let nonce_ciphertext =
            encrypt_signing_nonce_plaintext(&nonce_plaintext, &conversation_key).unwrap();
        let nonce_event = SigningNonceEvent::new(1, 1, nonce_ciphertext.clone());
        let nonce_message = NostrProtocolMessage::new(
            "room-a",
            NostrMessageKind::SigningNonceEncrypted,
            1,
            &nonce_event,
        )
        .unwrap()
        .with_wallet("treasury")
        .with_session("session-a")
        .to_party(1)
        .unwrap();

        assert!(decrypt_signing_nonce_plaintext(
            &nonce_ciphertext,
            &wrong_key,
            &nonce_message,
            &nonce_event,
        )
        .is_err());

        let wrong_session_message = NostrProtocolMessage::new(
            "room-a",
            NostrMessageKind::SigningNonceEncrypted,
            1,
            &nonce_event,
        )
        .unwrap()
        .with_wallet("treasury")
        .with_session("other-session")
        .to_party(1)
        .unwrap();

        assert!(decrypt_signing_nonce_plaintext(
            &nonce_ciphertext,
            &conversation_key,
            &wrong_session_message,
            &nonce_event,
        )
        .is_err());
    }

    #[test]
    fn signing_plaintexts_convert_to_protocol_inputs() {
        let nonce_input: SigningNonceInput = nonce(2).into();
        assert_eq!(nonce_input.wallet, "treasury");
        assert_eq!(nonce_input.party_index, 2);
        assert_eq!(nonce_input.public_nonce, "nonce-2");

        let share_input: SigningShareInput = share(3).into();
        assert_eq!(share_input.session, "session-a");
        assert_eq!(share_input.party_index, 3);
        assert_eq!(share_input.signature_share, "share-3");
    }
}
