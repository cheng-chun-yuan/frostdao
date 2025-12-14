use wasm_bindgen::prelude::*;

/// Initialize panic hook for better error messages in browser
#[wasm_bindgen(start)]
pub fn init() {
    console_error_panic_hook::set_once();
}

// WASM-exposed keygen functions

#[wasm_bindgen]
pub fn wasm_keygen_round1(threshold: u32, n_parties: u32, my_index: u32) -> Result<String, JsValue> {
    #[cfg(target_arch = "wasm32")]
    {
        use crate::storage::LocalStorageImpl;
        let storage = LocalStorageImpl;
        keygen::round1_core(threshold, n_parties, my_index, &storage)
            .map_err(|e| JsValue::from_str(&format!("Error: {}", e)))
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        Err(JsValue::from_str("WASM functions only available in WASM target"))
    }
}

#[wasm_bindgen]
pub fn wasm_keygen_round2(data: String) -> Result<String, JsValue> {
    #[cfg(target_arch = "wasm32")]
    {
        use crate::storage::LocalStorageImpl;
        let storage = LocalStorageImpl;
        keygen::round2_core(&data, &storage)
            .map_err(|e| JsValue::from_str(&format!("Error: {}", e)))
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        Err(JsValue::from_str("WASM functions only available in WASM target"))
    }
}

#[wasm_bindgen]
pub fn wasm_keygen_finalize(data: String) -> Result<String, JsValue> {
    #[cfg(target_arch = "wasm32")]
    {
        use crate::storage::LocalStorageImpl;
        let storage = LocalStorageImpl;
        keygen::finalize_core(&data, &storage)
            .map_err(|e| JsValue::from_str(&format!("Error: {}", e)))
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        Err(JsValue::from_str("WASM functions only available in WASM target"))
    }
}

// WASM-exposed signing functions

#[wasm_bindgen]
pub fn wasm_sign_nonce(session: String) -> Result<String, JsValue> {
    #[cfg(target_arch = "wasm32")]
    {
        use crate::storage::LocalStorageImpl;
        let storage = LocalStorageImpl;
        signing::generate_nonce_core(&session, &storage)
            .map_err(|e| JsValue::from_str(&format!("Error: {}", e)))
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        Err(JsValue::from_str("WASM functions only available in WASM target"))
    }
}

#[wasm_bindgen]
pub fn wasm_sign(session: String, message: String, data: String) -> Result<String, JsValue> {
    #[cfg(target_arch = "wasm32")]
    {
        use crate::storage::LocalStorageImpl;
        let storage = LocalStorageImpl;
        signing::create_signature_share_core(&session, &message, &data, &storage)
            .map_err(|e| JsValue::from_str(&format!("Error: {}", e)))
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        Err(JsValue::from_str("WASM functions only available in WASM target"))
    }
}

#[wasm_bindgen]
pub fn wasm_combine(data: String) -> Result<String, JsValue> {
    #[cfg(target_arch = "wasm32")]
    {
        use crate::storage::LocalStorageImpl;
        let storage = LocalStorageImpl;
        signing::combine_signatures_core(&data, &storage)
            .map_err(|e| JsValue::from_str(&format!("Error: {}", e)))
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        Err(JsValue::from_str("WASM functions only available in WASM target"))
    }
}
