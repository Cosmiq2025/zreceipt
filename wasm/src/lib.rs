//! Browser bindings: receipts are created and verified entirely client-side.
use wasm_bindgen::prelude::*;

fn js_err(e: anyhow::Error) -> JsValue {
    JsValue::from_str(&e.to_string())
}

/// Returns a JSON array of `{ receipt, disclosed }` for every output the key created.
#[wasm_bindgen(js_name = createReceipts)]
pub fn create_receipts(raw_tx_hex: &str, ufvk: &str, net: &str) -> Result<String, JsValue> {
    let net = zreceipt::Net::parse(net).map_err(js_err)?;
    let tx = zreceipt::parse_tx(raw_tx_hex).map_err(js_err)?;
    let r = zreceipt::create_receipts(&tx, ufvk, net).map_err(js_err)?;
    Ok(serde_json::to_string(&r).unwrap())
}

/// Verifies a receipt against the raw transaction; returns the disclosed payment as JSON.
#[wasm_bindgen(js_name = verifyReceipt)]
pub fn verify_receipt(raw_tx_hex: &str, receipt: &str) -> Result<String, JsValue> {
    let d = zreceipt::verify_encoded(raw_tx_hex, receipt).map_err(js_err)?;
    Ok(serde_json::to_string(&d).unwrap())
}

/// Decodes a receipt without the transaction, so the page knows which txid to fetch.
#[wasm_bindgen(js_name = inspectReceipt)]
pub fn inspect_receipt(receipt: &str) -> Result<String, JsValue> {
    let r = zreceipt::Receipt::decode(receipt).map_err(js_err)?;
    Ok(serde_json::to_string(&r).unwrap())
}
