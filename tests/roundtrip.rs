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
    let created = recover_actions::<IronwoodVersion, _>(bundle.actions().iter(), fvk, Net::Test, "abababababababababababababababababababababababababababababababab", Pool::Ironwood);
    assert_eq!(created.len(), 1, "expected exactly one disclosable payment");
    let c = &created[0];
    assert_eq!(c.disclosed.value_zat, PAID);
    assert_eq!(c.disclosed.memo.as_deref(), Some(MEMO));
    assert!(c.disclosed.recipient.starts_with("utest1"));
    assert!(!c.change);

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
    let created = recover_actions::<OrchardVersion, _>(bundle.actions().iter(), &f.sender_fvk, Net::Test, "abababababababababababababababababababababababababababababababab", Pool::Orchard);
    assert_eq!(created.len(), 1);
    let c = &created[0];
    assert_eq!(c.disclosed.value_zat, PAID);
    assert_eq!(c.disclosed.memo.as_deref(), Some(MEMO));

    // Verifier: opens the same payment from the encoded receipt alone.
    let receipt = Receipt::decode(&c.receipt).unwrap();
    assert_eq!(receipt.pool, Pool::Orchard);
    let ock = zcash_note_encryption::OutgoingCipherKey(hex::decode(&receipt.ock).unwrap().try_into().unwrap());
    let opened = open_action::<OrchardVersion, _>(bundle.actions().iter(), &receipt, &ock, receipt.txid.clone()).unwrap();
    assert_eq!(opened, c.disclosed);

    // An Orchard receipt does not open under the Ironwood note-encryption domain.
    assert!(open_action::<IronwoodVersion, _>(bundle.actions().iter(), &receipt, &ock, receipt.txid.clone()).is_err());
}

#[test]
fn receipt_opens_only_its_own_output() {
    let f = fixture();
    let bundle = build_payment(BundleVersion::ironwood_v3(), &f);
    let c = &recover_actions::<IronwoodVersion, _>(bundle.actions().iter(), &f.sender_fvk, Net::Test, "abababababababababababababababababababababababababababababababab", Pool::Ironwood)[0];
    let receipt = Receipt::decode(&c.receipt).unwrap();
    let ock = zcash_note_encryption::OutgoingCipherKey(hex::decode(&receipt.ock).unwrap().try_into().unwrap());

    // Pointing the same key at any other action in the bundle must fail.
    let n = bundle.actions().len();
    for i in (0..n).filter(|&i| i != receipt.index) {
        let other = Receipt { index: i, ..receipt.clone() };
        assert!(open_action::<IronwoodVersion, _>(bundle.actions().iter(), &other, &ock, "abababababababababababababababababababababababababababababababab".into()).is_err());
    }

    // A forged key must fail.
    let mut bad = ock.0;
    bad[0] ^= 1;
    let forged = zcash_note_encryption::OutgoingCipherKey(bad);
    assert!(open_action::<IronwoodVersion, _>(bundle.actions().iter(), &receipt, &forged, "abababababababababababababababababababababababababababababababab".into()).is_err());
}

#[test]
fn stranger_viewing_key_discloses_nothing() {
    let f = fixture();
    let bundle = build_payment(BundleVersion::ironwood_v3(), &f);
    let created = recover_actions::<IronwoodVersion, _>(bundle.actions().iter(), &f.stranger_fvk, Net::Test, "abababababababababababababababababababababababababababababababab", Pool::Ironwood);
    assert!(created.is_empty());
}

#[test]
fn receipt_encoding_rejects_garbage() {
    assert!(Receipt::decode("hello").is_err());
    assert!(Receipt::decode("zrcpt1!!!").is_err());
}

#[test]
fn edited_receipt_is_rejected_even_if_fields_are_unchanged() {
    let f = fixture();
    let bundle = build_payment(BundleVersion::ironwood_v3(), &f);
    let c = &recover_actions::<IronwoodVersion, _>(bundle.actions().iter(), &f.sender_fvk, Net::Test, "abababababababababababababababababababababababababababababababab", Pool::Ironwood)[0];
    let good = Receipt::decode(&c.receipt).unwrap();
    let body = |j: String| format!("zrcpt1{}", base64::Engine::encode(&base64::engine::general_purpose::URL_SAFE_NO_PAD, j));

    // Upper-case hex in the key decodes to the same bytes but is not the canonical encoding.
    let upper = body(serde_json::to_string(&Receipt { ock: good.ock.to_uppercase(), ..good.clone() }).unwrap());
    assert!(Receipt::decode(&upper).is_err());

    // Reordered and extra fields.
    let reordered = body(format!(
        r#"{{"ock":"{}","index":{},"pool":"ironwood","txid":"{}","net":"test","v":1}}"#,
        good.ock, good.index, good.txid
    ));
    assert!(Receipt::decode(&reordered).is_err());
    let mut extra: serde_json::Value = serde_json::to_value(&good).unwrap();
    extra["note"] = "x".into();
    assert!(Receipt::decode(&body(extra.to_string())).is_err());

    // Every single-character change to the receipt string is rejected or opens nothing.
    let alphabet = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let s = c.receipt.clone();
    for pos in 6..s.len() {
        for ch in alphabet.chars() {
            if s.as_bytes()[pos] as char == ch {
                continue;
            }
            let mut t = s.clone();
            t.replace_range(pos..pos + 1, &ch.to_string());
            if let Ok(r) = Receipt::decode(&t) {
                let ok = r.ock_bytes().ok().and_then(|ock| {
                    open_action::<IronwoodVersion, _>(bundle.actions().iter(), &r, &ock, r.txid.clone()).ok()
                });
                assert!(ok.is_none() || r.txid != good.txid || r.net != good.net, "edit at {pos} to {ch:?} still verified");
            }
        }
    }

    // The original still decodes.
    assert_eq!(Receipt::decode(&c.receipt).unwrap(), good);
}
