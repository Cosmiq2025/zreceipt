//! End-to-end cryptographic tests: a sender builds a shielded payment with the
//! official Orchard builder, creates a receipt from its viewing key, and an
//! independent verifier opens exactly that payment from the receipt alone.

use orchard::{
    builder::{Builder, BundleType},
    bundle::{BundleVersion, Flags},
    keys::{FullViewingKey, Scope, SpendingKey},
    note_encryption::{IronwoodVersion, OrchardVersion},
    value::NoteValue,
    Anchor,
};
use rand::rngs::OsRng;
use zcash_keys::keys::UnifiedSpendingKey;
use zcash_protocol::consensus::Network;
use zip32::AccountId;
use zreceipt::{open_action, recover_actions, Net, Pool, Receipt};

const PAID: u64 = 123_456_789;
const MEMO: &str = "Invoice #47 - web development, October";

fn memo_bytes(text: &str) -> [u8; 512] {
    let mut m = [0u8; 512];
    m[..text.len()].copy_from_slice(text.as_bytes());
    m
}

struct Fixture {
    sender_ufvk: String,
    sender_fvk: FullViewingKey,
    recipient: orchard::Address,
    stranger_fvk: FullViewingKey,
}

fn fixture() -> Fixture {
    let usk = UnifiedSpendingKey::from_seed(&Network::TestNetwork, &[7u8; 32], AccountId::ZERO).unwrap();
    let ufvk = usk.to_unified_full_viewing_key();
    let sender_fvk = ufvk.orchard().unwrap().clone();
    let recipient_fvk = FullViewingKey::from(&SpendingKey::from_zip32_seed(&[9u8; 32], 1, AccountId::ZERO).unwrap());
    let stranger_fvk = FullViewingKey::from(&SpendingKey::from_zip32_seed(&[3u8; 32], 1, AccountId::ZERO).unwrap());
    Fixture {
        sender_ufvk: ufvk.encode(&Network::TestNetwork),
        sender_fvk,
        recipient: recipient_fvk.address_at(0u32, Scope::External),
        stranger_fvk,
    }
}

fn build_payment(version: BundleVersion, f: &Fixture) -> orchard::bundle::Bundle<impl Sized + orchard::bundle::Authorization, i64> {
    let mut b = Builder::new(BundleType::DEFAULT, version, Flags::ENABLED, Anchor::empty_tree()).unwrap();
    b.add_output(
        Some(f.sender_fvk.to_ovk(Scope::External)),
        f.recipient,
        NoteValue::from_raw(PAID),
        memo_bytes(MEMO),
    )
    .unwrap();
    let (bundle, _meta) = b.build::<i64>(OsRng).unwrap().unwrap();
    bundle
}

#[test]
fn ironwood_payment_round_trip() {
    let f = fixture();
    let bundle = build_payment(BundleVersion::ironwood_v3(), &f);

    // The UFVK string a user would paste must decode to the same Orchard-family FVK.
    let decoded = zcash_keys::keys::UnifiedFullViewingKey::decode(&Network::TestNetwork, &f.sender_ufvk).unwrap();
    let fvk = decoded.orchard().unwrap();

    // Sender: exactly one action is ours (the rest are padding dummies).
    let created = recover_actions::<IronwoodVersion, _>(bundle.actions().iter(), fvk, Net::Test, "txid-under-test", Pool::Ironwood);
    assert_eq!(created.len(), 1, "expected exactly one disclosable payment");
    let c = &created[0];
    assert_eq!(c.disclosed.value_zat, PAID);
    assert_eq!(c.disclosed.memo.as_deref(), Some(MEMO));
    assert!(c.disclosed.recipient.starts_with("utest1"));

    // Verifier: only the encoded receipt + the public actions.
    let receipt = Receipt::decode(&c.receipt).unwrap();
    let ock = zcash_note_encryption::OutgoingCipherKey(hex::decode(&receipt.ock).unwrap().try_into().unwrap());
    let opened = open_action::<IronwoodVersion, _>(bundle.actions().iter(), &receipt, &ock, receipt.txid.clone()).unwrap();
    assert_eq!(opened, c.disclosed);
}

#[test]
fn orchard_payment_round_trip() {
    let f = fixture();
    let bundle = build_payment(BundleVersion::orchard_v2(), &f);
    let created = recover_actions::<OrchardVersion, _>(bundle.actions().iter(), &f.sender_fvk, Net::Test, "t", Pool::Orchard);
    assert_eq!(created.len(), 1);
    assert_eq!(created[0].disclosed.value_zat, PAID);
}

#[test]
fn receipt_opens_only_its_own_output() {
    let f = fixture();
    let bundle = build_payment(BundleVersion::ironwood_v3(), &f);
    let c = &recover_actions::<IronwoodVersion, _>(bundle.actions().iter(), &f.sender_fvk, Net::Test, "t", Pool::Ironwood)[0];
    let receipt = Receipt::decode(&c.receipt).unwrap();
    let ock = zcash_note_encryption::OutgoingCipherKey(hex::decode(&receipt.ock).unwrap().try_into().unwrap());

    // Pointing the same key at any other action in the bundle must fail.
    let n = bundle.actions().len();
    for i in (0..n).filter(|&i| i != receipt.index) {
        let other = Receipt { index: i, ..receipt.clone() };
        assert!(open_action::<IronwoodVersion, _>(bundle.actions().iter(), &other, &ock, "t".into()).is_err());
    }

    // A forged key must fail.
    let mut bad = ock.0;
    bad[0] ^= 1;
    let forged = zcash_note_encryption::OutgoingCipherKey(bad);
    assert!(open_action::<IronwoodVersion, _>(bundle.actions().iter(), &receipt, &forged, "t".into()).is_err());
}

#[test]
fn stranger_viewing_key_discloses_nothing() {
    let f = fixture();
    let bundle = build_payment(BundleVersion::ironwood_v3(), &f);
    let created = recover_actions::<IronwoodVersion, _>(bundle.actions().iter(), &f.stranger_fvk, Net::Test, "t", Pool::Ironwood);
    assert!(created.is_empty());
}

#[test]
fn receipt_encoding_rejects_garbage() {
    assert!(Receipt::decode("hello").is_err());
    assert!(Receipt::decode("zrcpt1!!!").is_err());
}
