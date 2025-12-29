use wasm_bindgen::prelude::*;

#[cfg(target_arch = "wasm32")]
use serde::Serialize;

#[cfg(target_arch = "wasm32")]
use crate::{btc::schnorr as bitcoin_schnorr, protocol::keygen, protocol::signing};

/// Initialize panic hook for better error messages in browser
#[wasm_bindgen(start)]
pub fn init() {
    console_error_panic_hook::set_once();
}

/// Helper struct for WASM JSON serialization
#[cfg(target_arch = "wasm32")]
#[derive(Serialize)]
struct WasmCommandResult {
    output: String,
    result: String,
}

/// Convert CommandResult to JSON string for WASM
#[cfg(target_arch = "wasm32")]
fn command_result_to_json(cmd_result: crate::CommandResult) -> Result<String, JsValue> {
    let wasm_result = WasmCommandResult {
        output: cmd_result.output,
        result: cmd_result.result,
    };
    serde_json::to_string(&wasm_result)
        .map_err(|e| JsValue::from_str(&format!("JSON serialization error: {}", e)))
}

// WASM-exposed keygen functions

#[wasm_bindgen]
#[cfg_attr(not(target_arch = "wasm32"), allow(unused_variables))]
pub fn wasm_keygen_round1(
    threshold: u32,
    n_parties: u32,
    my_index: u32,
    rank: u32,
    hierarchical: bool,
) -> Result<String, JsValue> {
    #[cfg(target_arch = "wasm32")]
    {
        use crate::storage::LocalStorageImpl;
        let storage = LocalStorageImpl;
        let cmd_result =
            keygen::round1_core(threshold, n_parties, my_index, rank, hierarchical, &storage)
                .map_err(|e| JsValue::from_str(&format!("Error: {}", e)))?;
        command_result_to_json(cmd_result)
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        Err(JsValue::from_str(
            "WASM functions only available in WASM target",
        ))
    }
}

#[wasm_bindgen]
#[cfg_attr(not(target_arch = "wasm32"), allow(unused_variables))]
pub fn wasm_keygen_round2(data: String) -> Result<String, JsValue> {
    #[cfg(target_arch = "wasm32")]
    {
        use crate::storage::LocalStorageImpl;
        let storage = LocalStorageImpl;
        let cmd_result = keygen::round2_core(&data, &storage, false)
            .map_err(|e| JsValue::from_str(&format!("Error: {}", e)))?;
        command_result_to_json(cmd_result)
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        Err(JsValue::from_str(
            "WASM functions only available in WASM target",
        ))
    }
}

#[wasm_bindgen]
#[cfg_attr(not(target_arch = "wasm32"), allow(unused_variables))]
pub fn wasm_keygen_finalize(data: String) -> Result<String, JsValue> {
    #[cfg(target_arch = "wasm32")]
    {
        use crate::storage::LocalStorageImpl;
        let storage = LocalStorageImpl;
        let cmd_result = keygen::finalize_core(&data, &storage)
            .map_err(|e| JsValue::from_str(&format!("Error: {}", e)))?;
        command_result_to_json(cmd_result)
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        Err(JsValue::from_str(
            "WASM functions only available in WASM target",
        ))
    }
}

// WASM-exposed signing functions

#[wasm_bindgen]
#[cfg_attr(not(target_arch = "wasm32"), allow(unused_variables))]
pub fn wasm_generate_nonce(session: String) -> Result<String, JsValue> {
    #[cfg(target_arch = "wasm32")]
    {
        use crate::storage::LocalStorageImpl;
        let storage = LocalStorageImpl;
        let cmd_result = signing::generate_nonce_core(&session, &storage)
            .map_err(|e| JsValue::from_str(&format!("Error: {}", e)))?;
        command_result_to_json(cmd_result)
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        Err(JsValue::from_str(
            "WASM functions only available in WASM target",
        ))
    }
}

#[wasm_bindgen]
#[cfg_attr(not(target_arch = "wasm32"), allow(unused_variables))]
pub fn wasm_sign(session: String, message: String, data: String) -> Result<String, JsValue> {
    #[cfg(target_arch = "wasm32")]
    {
        use crate::storage::LocalStorageImpl;
        let storage = LocalStorageImpl;
        let cmd_result = signing::create_signature_share_core(&session, &message, &data, &storage)
            .map_err(|e| JsValue::from_str(&format!("Error: {}", e)))?;
        command_result_to_json(cmd_result)
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        Err(JsValue::from_str(
            "WASM functions only available in WASM target",
        ))
    }
}

#[wasm_bindgen]
#[cfg_attr(not(target_arch = "wasm32"), allow(unused_variables))]
pub fn wasm_combine(data: String) -> Result<String, JsValue> {
    #[cfg(target_arch = "wasm32")]
    {
        use crate::storage::LocalStorageImpl;
        let storage = LocalStorageImpl;
        let cmd_result = signing::combine_signatures_core(&data, &storage)
            .map_err(|e| JsValue::from_str(&format!("Error: {}", e)))?;
        command_result_to_json(cmd_result)
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        Err(JsValue::from_str(
            "WASM functions only available in WASM target",
        ))
    }
}

#[wasm_bindgen]
#[cfg_attr(not(target_arch = "wasm32"), allow(unused_variables))]
pub fn wasm_verify(
    signature: String,
    public_key: String,
    message: String,
) -> Result<String, JsValue> {
    #[cfg(target_arch = "wasm32")]
    {
        let cmd_result = signing::verify_signature_core(&signature, &public_key, &message)
            .map_err(|e| JsValue::from_str(&format!("Error: {}", e)))?;
        command_result_to_json(cmd_result)
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        Err(JsValue::from_str(
            "WASM functions only available in WASM target",
        ))
    }
}

// ============================================================================
// WASM-exposed Bitcoin Schnorr (BIP340) functions
// ============================================================================

#[wasm_bindgen]
pub fn wasm_btc_keygen() -> Result<String, JsValue> {
    #[cfg(target_arch = "wasm32")]
    {
        use crate::storage::LocalStorageImpl;
        let storage = LocalStorageImpl;
        let cmd_result = bitcoin_schnorr::generate_keypair_core(&storage)
            .map_err(|e| JsValue::from_str(&format!("Error: {}", e)))?;
        command_result_to_json(cmd_result)
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        Err(JsValue::from_str(
            "WASM functions only available in WASM target",
        ))
    }
}

#[wasm_bindgen]
#[cfg_attr(not(target_arch = "wasm32"), allow(unused_variables))]
pub fn wasm_btc_import_key(secret_hex: String) -> Result<String, JsValue> {
    #[cfg(target_arch = "wasm32")]
    {
        use crate::storage::LocalStorageImpl;
        let storage = LocalStorageImpl;
        let cmd_result = bitcoin_schnorr::import_key_core(&secret_hex, &storage)
            .map_err(|e| JsValue::from_str(&format!("Error: {}", e)))?;
        command_result_to_json(cmd_result)
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        Err(JsValue::from_str(
            "WASM functions only available in WASM target",
        ))
    }
}

#[wasm_bindgen]
pub fn wasm_btc_get_pubkey() -> Result<String, JsValue> {
    #[cfg(target_arch = "wasm32")]
    {
        use crate::storage::LocalStorageImpl;
        let storage = LocalStorageImpl;
        let cmd_result = bitcoin_schnorr::get_public_key_core(&storage)
            .map_err(|e| JsValue::from_str(&format!("Error: {}", e)))?;
        command_result_to_json(cmd_result)
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        Err(JsValue::from_str(
            "WASM functions only available in WASM target",
        ))
    }
}

#[wasm_bindgen]
#[cfg_attr(not(target_arch = "wasm32"), allow(unused_variables))]
pub fn wasm_btc_sign(message: String) -> Result<String, JsValue> {
    #[cfg(target_arch = "wasm32")]
    {
        use crate::storage::LocalStorageImpl;
        let storage = LocalStorageImpl;
        let cmd_result = bitcoin_schnorr::sign_message_core(message.as_bytes(), None, &storage)
            .map_err(|e| JsValue::from_str(&format!("Error: {}", e)))?;
        command_result_to_json(cmd_result)
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        Err(JsValue::from_str(
            "WASM functions only available in WASM target",
        ))
    }
}

#[wasm_bindgen]
#[cfg_attr(not(target_arch = "wasm32"), allow(unused_variables))]
pub fn wasm_btc_sign_hex(message_hex: String) -> Result<String, JsValue> {
    #[cfg(target_arch = "wasm32")]
    {
        use crate::storage::LocalStorageImpl;
        let storage = LocalStorageImpl;
        let message = hex::decode(&message_hex)
            .map_err(|e| JsValue::from_str(&format!("Invalid hex: {}", e)))?;
        let cmd_result = bitcoin_schnorr::sign_message_core(&message, None, &storage)
            .map_err(|e| JsValue::from_str(&format!("Error: {}", e)))?;
        command_result_to_json(cmd_result)
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        Err(JsValue::from_str(
            "WASM functions only available in WASM target",
        ))
    }
}

#[wasm_bindgen]
#[cfg_attr(not(target_arch = "wasm32"), allow(unused_variables))]
pub fn wasm_btc_verify(
    signature: String,
    public_key: String,
    message: String,
) -> Result<String, JsValue> {
    #[cfg(target_arch = "wasm32")]
    {
        let cmd_result =
            bitcoin_schnorr::verify_signature_core(&signature, &public_key, message.as_bytes())
                .map_err(|e| JsValue::from_str(&format!("Error: {}", e)))?;
        command_result_to_json(cmd_result)
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        Err(JsValue::from_str(
            "WASM functions only available in WASM target",
        ))
    }
}

#[wasm_bindgen]
#[cfg_attr(not(target_arch = "wasm32"), allow(unused_variables))]
pub fn wasm_btc_verify_hex(
    signature: String,
    public_key: String,
    message_hex: String,
) -> Result<String, JsValue> {
    #[cfg(target_arch = "wasm32")]
    {
        let message = hex::decode(&message_hex)
            .map_err(|e| JsValue::from_str(&format!("Invalid hex: {}", e)))?;
        let cmd_result = bitcoin_schnorr::verify_signature_core(&signature, &public_key, &message)
            .map_err(|e| JsValue::from_str(&format!("Error: {}", e)))?;
        command_result_to_json(cmd_result)
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        Err(JsValue::from_str(
            "WASM functions only available in WASM target",
        ))
    }
}

#[wasm_bindgen]
#[cfg_attr(not(target_arch = "wasm32"), allow(unused_variables))]
pub fn wasm_btc_sign_taproot(sighash_hex: String) -> Result<String, JsValue> {
    #[cfg(target_arch = "wasm32")]
    {
        use crate::storage::LocalStorageImpl;
        let storage = LocalStorageImpl;
        let cmd_result = bitcoin_schnorr::sign_taproot_sighash_core(&sighash_hex, &storage)
            .map_err(|e| JsValue::from_str(&format!("Error: {}", e)))?;
        command_result_to_json(cmd_result)
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        Err(JsValue::from_str(
            "WASM functions only available in WASM target",
        ))
    }
}

// NIP-44 E2E Encryption WASM functions

/// Encrypt a message for a recipient using NIP-44 v2
/// - plaintext_hex: Hex-encoded plaintext to encrypt
/// - sender_secret_hex: Sender's secret coefficient (32 bytes hex)
/// - recipient_pubkey_hex: Recipient's encryption pubkey (32 bytes hex)
/// Returns: Base64-encoded ciphertext
#[wasm_bindgen]
#[cfg_attr(not(target_arch = "wasm32"), allow(unused_variables))]
pub fn wasm_nip44_encrypt(
    plaintext_hex: String,
    sender_secret_hex: String,
    recipient_pubkey_hex: String,
) -> Result<String, JsValue> {
    #[cfg(target_arch = "wasm32")]
    {
        use crate::crypto::nip44;

        let plaintext = hex::decode(&plaintext_hex)
            .map_err(|e| JsValue::from_str(&format!("Invalid plaintext hex: {}", e)))?;

        let sender_secret: [u8; 32] = hex::decode(&sender_secret_hex)
            .map_err(|e| JsValue::from_str(&format!("Invalid sender secret hex: {}", e)))?
            .try_into()
            .map_err(|_| JsValue::from_str("Sender secret must be 32 bytes"))?;

        let recipient_pubkey: [u8; 32] = hex::decode(&recipient_pubkey_hex)
            .map_err(|e| JsValue::from_str(&format!("Invalid recipient pubkey hex: {}", e)))?
            .try_into()
            .map_err(|_| JsValue::from_str("Recipient pubkey must be 32 bytes"))?;

        nip44::encrypt_for_recipient(&plaintext, &sender_secret, &recipient_pubkey)
            .map_err(|e| JsValue::from_str(&format!("Encryption failed: {}", e)))
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        Err(JsValue::from_str(
            "WASM functions only available in WASM target",
        ))
    }
}

/// Decrypt a NIP-44 v2 encrypted message from a sender
/// - ciphertext_b64: Base64-encoded ciphertext
/// - recipient_secret_hex: Recipient's secret coefficient (32 bytes hex)
/// - sender_pubkey_hex: Sender's encryption pubkey (32 bytes hex)
/// Returns: Hex-encoded plaintext
#[wasm_bindgen]
#[cfg_attr(not(target_arch = "wasm32"), allow(unused_variables))]
pub fn wasm_nip44_decrypt(
    ciphertext_b64: String,
    recipient_secret_hex: String,
    sender_pubkey_hex: String,
) -> Result<String, JsValue> {
    #[cfg(target_arch = "wasm32")]
    {
        use crate::crypto::nip44;

        let recipient_secret: [u8; 32] = hex::decode(&recipient_secret_hex)
            .map_err(|e| JsValue::from_str(&format!("Invalid recipient secret hex: {}", e)))?
            .try_into()
            .map_err(|_| JsValue::from_str("Recipient secret must be 32 bytes"))?;

        let sender_pubkey: [u8; 32] = hex::decode(&sender_pubkey_hex)
            .map_err(|e| JsValue::from_str(&format!("Invalid sender pubkey hex: {}", e)))?
            .try_into()
            .map_err(|_| JsValue::from_str("Sender pubkey must be 32 bytes"))?;

        let plaintext =
            nip44::decrypt_from_sender(&ciphertext_b64, &recipient_secret, &sender_pubkey)
                .map_err(|e| JsValue::from_str(&format!("Decryption failed: {}", e)))?;

        Ok(hex::encode(plaintext))
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        Err(JsValue::from_str(
            "WASM functions only available in WASM target",
        ))
    }
}

/// Extract encryption pubkey from Round1Output JSON
/// Returns: 32-byte x-only pubkey in hex, or null if not present
#[wasm_bindgen]
#[cfg_attr(not(target_arch = "wasm32"), allow(unused_variables))]
pub fn wasm_extract_encryption_pubkey(round1_output_json: String) -> Result<String, JsValue> {
    #[cfg(target_arch = "wasm32")]
    {
        let output: keygen::Round1Output = serde_json::from_str(&round1_output_json)
            .map_err(|e| JsValue::from_str(&format!("Invalid Round1Output JSON: {}", e)))?;

        match output.encryption_pubkey {
            Some(pk) => Ok(pk),
            None => Err(JsValue::from_str("No encryption pubkey in Round1Output")),
        }
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        Err(JsValue::from_str(
            "WASM functions only available in WASM target",
        ))
    }
}
