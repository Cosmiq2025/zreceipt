//! Sapling: a sender builds a shielded payment with the official Sapling
//! builder, creates a receipt from its viewing key, and a verifier opens
//! exactly that payment from the receipt alone.

use rand::rngs::OsRng;
use sapling::{
    builder::{Builder, BundleType},
    prover::mock::{MockOutputProver, MockSpendProver},
    value::NoteValue,
    zip32::ExtendedSpendingKey,
    Anchor,
};
use sapling::note_encryption::Zip212Enforcement;
use zcash_keys::keys::UnifiedSpendingKey;
use zcash_protocol::consensus::Network;
use zip32::AccountId;
use zreceipt::{open_sapling_output, recover_sapling_outputs, Net, Pool, Receipt};

const PAID: u64 = 250_000;
const MEMO: &str = "Invoice 12";

fn memo_bytes(text: &str) -> [u8; 512] {
    let mut m = [0u8; 512];
    m[..text.len()].copy_from_slice(text.as_bytes());
    m
}

#[test]
fn sapling_payment_round_trip() {
    let usk = UnifiedSpendingKey::from_seed(&Network::TestNetwork, &[7u8; 32], AccountId::ZERO).unwrap();
    let ufvk = usk.to_unified_full_viewing_key();
    let dfvk = ufvk.sapling().unwrap().clone();
    let (_, recipient) = ExtendedSpendingKey::master(&[9u8; 32]).default_address();
    let (_, stranger) = (0, ExtendedSpendingKey::master(&[3u8; 32]).to_diversifiable_full_viewing_key());

    let mut b = Builder::new(Zip212Enforcement::On, BundleType::DEFAULT, Anchor::empty_tree());
    b.add_output(Some(dfvk.to_ovk(zip32::Scope::External)), recipient, NoteValue::from_raw(PAID), memo_bytes(MEMO))
        .unwrap();
    let (bundle, _) = b
        .build::<MockSpendProver, MockOutputProver, _, i64>(&[], OsRng)
        .unwrap()
        .unwrap();
    let outputs = bundle.shielded_outputs();

    // Sender: exactly one output is ours (the rest is padding).
    let created = recover_sapling_outputs(outputs, &dfvk, Net::Test, "abababababababababababababababababababababababababababababababab");
    assert_eq!(created.len(), 1);
    let c = &created[0];
    assert_eq!(c.disclosed.value_zat, PAID);
    assert!(!c.change, "a payment to someone else is not change");
    assert_eq!(c.disclosed.memo.as_deref(), Some(MEMO));
    assert!(c.disclosed.recipient.starts_with("ztestsapling1"));

    // Verifier: opens it from the encoded receipt alone.
    let receipt = Receipt::decode(&c.receipt).unwrap();
    assert_eq!(receipt.pool, Pool::Sapling);
    let ock = receipt.ock_bytes().unwrap();
    assert_eq!(open_sapling_output(outputs, &receipt, &ock, "abababababababababababababababababababababababababababababababab".into()).unwrap(), c.disclosed);

    // The same key opens no other output, and a forged key opens nothing.
    for i in (0..outputs.len()).filter(|&i| i != receipt.index) {
        assert!(open_sapling_output(outputs, &Receipt { index: i, ..receipt.clone() }, &ock, "abababababababababababababababababababababababababababababababab".into()).is_err());
    }
    let mut bad = ock.0;
    bad[0] ^= 1;
    assert!(open_sapling_output(outputs, &receipt, &zcash_note_encryption::OutgoingCipherKey(bad), "abababababababababababababababababababababababababababababababab".into()).is_err());

    // Someone else's viewing key recovers nothing.
    assert!(recover_sapling_outputs(outputs, &stranger, Net::Test, "abababababababababababababababababababababababababababababababab").is_empty());
}
