//! Full-path test: a serialized v6 transaction with an Ironwood bundle goes
//! through the exact public API the website uses (hex in, receipt string out,
//! receipt string + hex in, disclosed payment out).
//!
//! The proof and signatures are placeholders: zreceipt never relies on them,
//! because disclosure only depends on the encrypted note and its commitment.
//! Consensus validity is the chain's job; the verifier fetches mined transactions.

use orchard::{
    builder::{Builder, BundleType},
    bundle::{Authorized, BundleVersion, Flags},
    keys::{FullViewingKey, Scope, SpendingKey},
    primitives::redpallas,
    value::NoteValue,
    Anchor, Proof,
};
use rand::rngs::OsRng;
use zcash_keys::keys::UnifiedSpendingKey;
use zcash_primitives::transaction::{Authorized as TxAuthorized, TransactionData};
use zcash_protocol::{
    consensus::{BlockHeight, BranchId, Network},
    value::ZatBalance,
};
use zip32::AccountId;

const PAID: u64 = 2_500_000_000; // 25 ZEC
const MEMO: &str = "Invoice #47 - web development";

fn ironwood_tx_hex() -> (String, String) {
    let usk = UnifiedSpendingKey::from_seed(&Network::TestNetwork, &[42u8; 32], AccountId::ZERO).unwrap();
    let ufvk = usk.to_unified_full_viewing_key();
    let sender = ufvk.orchard().unwrap();
    let recipient = FullViewingKey::from(&SpendingKey::from_zip32_seed(&[8u8; 32], 1, AccountId::ZERO).unwrap())
        .address_at(0u32, Scope::External);

    let mut memo = [0u8; 512];
    memo[..MEMO.len()].copy_from_slice(MEMO.as_bytes());

    let mut b = Builder::new(BundleType::DEFAULT, BundleVersion::ironwood_v3(), Flags::ENABLED, Anchor::empty_tree()).unwrap();
    b.add_output(Some(sender.to_ovk(Scope::External)), recipient, NoteValue::from_raw(PAID), memo).unwrap();
    let (bundle, _) = b.build::<i64>(OsRng).unwrap().unwrap();

    let authorized: orchard::Bundle<Authorized, ZatBalance> = bundle
        .try_map_value_balance(ZatBalance::from_i64)
        .unwrap()
        .map_authorization(
            &mut (),
            |_, _, _| redpallas::Signature::from([0u8; 64]),
            |_, _| Authorized::from_parts(Proof::new(vec![0u8; 7264]), redpallas::Signature::from([0u8; 64])),
        );

    let tx = TransactionData::<TxAuthorized>::from_parts_v6(
        BranchId::Nu6_3,
        0,
        BlockHeight::from_u32(4_200_000),
        None,
        None,
        None,
        Some(authorized),
    )
    .freeze()
    .unwrap();

    let mut bytes = Vec::new();
    tx.write(&mut bytes).unwrap();
    (hex::encode(bytes), ufvk.encode(&Network::TestNetwork))
}

#[test]
fn serialized_ironwood_transaction_end_to_end() {
    let (hex_tx, ufvk) = ironwood_tx_hex();

    let tx = zreceipt::parse_tx(&hex_tx).unwrap();
    let created = zreceipt::create_receipts(&tx, &ufvk, zreceipt::Net::Test).unwrap();
    assert_eq!(created.len(), 1);
    assert_eq!(created[0].disclosed.pool, zreceipt::Pool::Ironwood);

    let opened = zreceipt::verify_encoded(&hex_tx, &created[0].receipt).unwrap();
    assert_eq!(opened.value_zat, PAID);
    assert_eq!(opened.memo.as_deref(), Some(MEMO));
    assert_eq!(opened.txid, tx.txid().to_string());

    // Optional: write a fixture for local website testing.
    if let Ok(path) = std::env::var("ZRECEIPT_FIXTURE") {
        let fixture = serde_json::json!({
            "txid": opened.txid, "hex": hex_tx, "ufvk": ufvk, "receipt": created[0].receipt,
        });
        std::fs::write(path, serde_json::to_string_pretty(&fixture).unwrap()).unwrap();
    }
}

#[test]
fn receipt_for_another_transaction_is_rejected() {
    let (a, ufvk) = ironwood_tx_hex();
    let (b, _) = ironwood_tx_hex();
    let tx_a = zreceipt::parse_tx(&a).unwrap();
    let receipt = &zreceipt::create_receipts(&tx_a, &ufvk, zreceipt::Net::Test).unwrap()[0].receipt;
    let err = zreceipt::verify_encoded(&b, receipt).unwrap_err().to_string();
    assert!(err.contains("receipt is for transaction"), "{err}");
}
