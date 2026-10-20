//! Prototype of the proposed Orchard/Ironwood spend-authority proof for ZIP 311
//! payment disclosures, using only the public orchard 0.15.5 API.
//!
//! Construction under discussion: for a spend in the transaction, a fresh Action
//! proof for the same note against the transaction's anchor and nullifier, with a
//! fresh alpha and rcv, a zero-value dummy output to the spent note's own address
//! (rho = nf), flags spends on / outputs off / cross-address disabled, verified
//! under the PostNu6_3 key; plus a spend-authorization signature under the new rk
//! over the disclosure digest. The digest here is a placeholder, because ZIP 311
//! does not yet define the encoding it covers.

use std::sync::OnceLock;

use ff::{Field, PrimeField};
use incrementalmerkletree::{Hashable, Level};
use orchard::{
    builder::SpendInfo,
    bundle::{BundleVersion, Flags},
    circuit::{Circuit, Instance, OrchardCircuitVersion, Proof, ProvingKey, VerifyingKey},
    keys::{FullViewingKey, Scope, SpendAuthorizingKey, SpendValidatingKey, SpendingKey},
    note::{ExtractedNoteCommitment, Nullifier, RandomSeed, Rho},
    primitives::redpallas::{Signature, SpendAuth, VerificationKey},
    tree::{MerkleHashOrchard, MerklePath},
    value::{NoteValue, ValueCommitTrapdoor, ValueCommitment},
    Address, Anchor, Note, NoteVersion,
};
use pasta_curves::pallas;
use rand::{rngs::OsRng, RngCore};

/// The two pools under test: Orchard (v2 note plaintexts) and Ironwood (v3).
const POOLS: [(NoteVersion, BundleVersion); 2] = [
    (NoteVersion::V2, BundleVersion::orchard_v3()),
    (NoteVersion::V3, BundleVersion::ironwood_v3()),
];

/// Placeholder for the ZIP 311 digest of the unsigned disclosure.
const DIGEST: &[u8] = b"zip311-digest-placeholder-32byte";

fn keys() -> &'static (ProvingKey, VerifyingKey) {
    static KEYS: OnceLock<(ProvingKey, VerifyingKey)> = OnceLock::new();
    KEYS.get_or_init(|| {
        (
            ProvingKey::build(OrchardCircuitVersion::PostNu6_3),
            VerifyingKey::build(OrchardCircuitVersion::PostNu6_3),
        )
    })
}

fn random_rho() -> Rho {
    loop {
        let rho = Rho::from_bytes(&pallas::Base::random(OsRng).to_repr());
        if bool::from(rho.is_some()) {
            return rho.unwrap();
        }
    }
}

fn new_note(recipient: Address, value: u64, rho: Rho, version: NoteVersion) -> Note {
    loop {
        let mut bytes = [0u8; 32];
        OsRng.fill_bytes(&mut bytes);
        let rseed = RandomSeed::from_bytes(bytes, &rho);
        if bool::from(rseed.is_none()) {
            continue;
        }
        let note = Note::from_parts(
            recipient,
            NoteValue::from_raw(value),
            rho,
            rseed.unwrap(),
            version,
        );
        if bool::from(note.is_some()) {
            return note.unwrap();
        }
    }
}

fn random_rcv() -> ValueCommitTrapdoor {
    // `ValueCommitTrapdoor::random` is crate-private; `from_bytes` is public.
    ValueCommitTrapdoor::from_bytes(pallas::Scalar::random(OsRng).to_repr()).unwrap()
}

fn address_of(seed: [u8; 32]) -> Address {
    FullViewingKey::from(&SpendingKey::from_bytes(seed).unwrap()).address_at(0u32, Scope::External)
}

/// A note as spent by an earlier transaction: the public (anchor, nf) plus the
/// owner's secrets.
struct Spent {
    sk: SpendingKey,
    fvk: FullViewingKey,
    note: Note,
    path: MerklePath,
    anchor: Anchor,
    nf: Nullifier,
}

fn spent_note(value: u64, version: NoteVersion) -> Spent {
    let mut seed = [0u8; 32];
    OsRng.fill_bytes(&mut seed);
    let sk = SpendingKey::from_bytes(seed).unwrap();
    let fvk = FullViewingKey::from(&sk);
    let note = new_note(
        fvk.address_at(0u32, Scope::External),
        value,
        random_rho(),
        version,
    );

    // Synthetic path: the level-0 sibling is an unrelated note's commitment, higher
    // siblings are empty-subtree roots. Enough to exercise the circuit's Merkle check;
    // not a real tree.
    let unrelated = new_note(address_of([7u8; 32]), 5, random_rho(), NoteVersion::V2);
    let auth_path: [MerkleHashOrchard; 32] = core::array::from_fn(|level| {
        if level == 0 {
            MerkleHashOrchard::from_cmx(&ExtractedNoteCommitment::from(unrelated.commitment()))
        } else {
            MerkleHashOrchard::empty_root(Level::from(level as u8))
        }
    });
    let path = MerklePath::from_parts(OsRng.next_u32() & 0xffff, auth_path);
    let anchor = path.root(ExtractedNoteCommitment::from(note.commitment()));
    let nf = note.nullifier(&fvk);
    Spent {
        sk,
        fvk,
        note,
        path,
        anchor,
        nf,
    }
}

/// One disclosure entry: the public inputs it carries, the proof and the signature.
struct Entry {
    cv_net: ValueCommitment,
    rk: VerificationKey<SpendAuth>,
    cmx: ExtractedNoteCommitment,
    proof: Proof,
    sig: Signature<SpendAuth>,
}

/// spends on, outputs off; bit 2 is 0, which means cross-address disabled for both pools.
fn flags(bundle_version: BundleVersion) -> Flags {
    Flags::from_byte(0x01, bundle_version).unwrap()
}

/// Creates an entry for `spent`. `dummy_recipient` overrides the dummy output's
/// address (the proposal uses the spent note's own address).
fn create_entry(
    spent: &Spent,
    version: NoteVersion,
    bundle_version: BundleVersion,
    dummy_recipient: Option<Address>,
) -> Result<Entry, String> {
    let (pk, _) = keys();
    // `Rho::from_nf_old` is crate-private; the nullifier's bytes give the same value.
    let rho = Rho::from_bytes(&spent.nf.to_bytes()).unwrap();
    let dummy = new_note(
        dummy_recipient.unwrap_or(spent.note.recipient()),
        0,
        rho,
        version,
    );

    let alpha = pallas::Scalar::random(OsRng);
    let rcv = random_rcv();
    let cv_net = ValueCommitment::derive(spent.note.value() - NoteValue::from_raw(0), rcv.clone());
    let rk = SpendValidatingKey::from(spent.fvk.clone()).randomize(&alpha);
    let cmx = ExtractedNoteCommitment::from(dummy.commitment());

    let spend =
        SpendInfo::new(spent.fvk.clone(), spent.note, spent.path.clone()).ok_or("spend info")?;
    let circuit =
        Circuit::from_action_context(spend, dummy, alpha, rcv, OrchardCircuitVersion::PostNu6_3)
            .ok_or("circuit")?;
    let instance = Instance::from_parts(
        spent.anchor,
        cv_net.clone(),
        spent.nf,
        rk.clone(),
        cmx,
        flags(bundle_version),
    )
    .ok_or("instance")?;
    let proof =
        Proof::create(pk, &[circuit], &[instance], OsRng).map_err(|e| format!("create: {e:?}"))?;
    let sig = SpendAuthorizingKey::from(&spent.sk)
        .randomize(&alpha)
        .sign(OsRng, DIGEST);
    Ok(Entry {
        cv_net,
        rk,
        cmx,
        proof,
        sig,
    })
}

/// Verifier side: `anchor` and `nf` come from the transaction, everything else from the entry.
fn verify_entry(
    anchor: Anchor,
    nf: Nullifier,
    entry: &Entry,
    bundle_version: BundleVersion,
    digest: &[u8],
) -> Result<(), &'static str> {
    let (_, vk) = keys();
    entry
        .rk
        .verify(digest, &entry.sig)
        .map_err(|_| "signature")?;
    let instance = Instance::from_parts(
        anchor,
        entry.cv_net.clone(),
        nf,
        entry.rk.clone(),
        entry.cmx,
        flags(bundle_version),
    )
    .ok_or("instance")?;
    entry.proof.verify(vk, &[instance]).map_err(|_| "proof")
}

#[test]
fn honest_entry_verifies() {
    for (version, bundle_version) in POOLS {
        let spent = spent_note(250_000, version);
        let entry = create_entry(&spent, version, bundle_version, None).expect("create");
        verify_entry(spent.anchor, spent.nf, &entry, bundle_version, DIGEST).expect("verify");
        assert_eq!(entry.proof.as_ref().len(), 4992, "one-Action proof size");
    }
}

#[test]
fn wrong_nullifier_wrong_anchor_or_other_digest_is_rejected() {
    for (version, bundle_version) in POOLS {
        let spent = spent_note(250_000, version);
        let entry = create_entry(&spent, version, bundle_version, None).unwrap();
        let other = spent_note(250_000, version);
        assert_eq!(
            verify_entry(spent.anchor, other.nf, &entry, bundle_version, DIGEST),
            Err("proof")
        );
        assert_eq!(
            verify_entry(other.anchor, spent.nf, &entry, bundle_version, DIGEST),
            Err("proof")
        );
        assert_eq!(
            verify_entry(
                spent.anchor,
                spent.nf,
                &entry,
                bundle_version,
                b"a different disclosure digest!!!"
            ),
            Err("signature")
        );
    }
}

#[test]
fn proof_for_own_note_does_not_verify_against_another_spend() {
    // The attacker holds their own note and key, not the victim's. Their valid entry
    // must not verify against the victim's public (anchor, nf).
    for (version, bundle_version) in POOLS {
        let victim = spent_note(250_000, version);
        let attacker = spent_note(250_000, version);
        let entry = create_entry(&attacker, version, bundle_version, None).unwrap();
        verify_entry(attacker.anchor, attacker.nf, &entry, bundle_version, DIGEST)
            .expect("valid for own spend");
        assert_eq!(
            verify_entry(victim.anchor, victim.nf, &entry, bundle_version, DIGEST),
            Err("proof")
        );
    }
}

#[test]
fn dummy_output_to_another_address_is_rejected() {
    // With cross-address disabled, the circuit requires the output address to equal
    // the spent note's address. Proving does not check constraints, so creation
    // succeeds and verification must fail.
    for (version, bundle_version) in POOLS {
        let spent = spent_note(250_000, version);
        let entry = create_entry(&spent, version, bundle_version, Some(address_of([9u8; 32])))
            .expect("proving does not check constraints");
        assert_eq!(
            verify_entry(spent.anchor, spent.nf, &entry, bundle_version, DIGEST),
            Err("proof")
        );
    }
}
