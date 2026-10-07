//! zreceipt — selective payment disclosures for shielded Zcash transactions.
//!
//! A sender who holds a viewing key derives, for one shielded output, the
//! *outgoing cipher key* (`ock`). Publishing that single key lets anyone who
//! holds the raw transaction decrypt exactly that output — amount, recipient
//! and memo — and nothing else from the sender's wallet. The verifier
//! re-derives the note commitment from the decrypted note and checks it
//! against the commitment in the transaction, so a receipt cannot claim an
//! amount or recipient that the chain does not contain.
//!
//! This follows the output-disclosure part of the draft ZIP 311 (Sapling)
//! and applies the same construction to Orchard actions.

use anyhow::{anyhow, bail, Context, Result};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use serde::{Deserialize, Serialize};

use orchard::note_encryption::{DomainVersion, IronwoodVersion, NoteEncryptionDomain, OrchardVersion};
use orchard::Action;
use sapling::note_encryption::{SaplingDomain, Zip212Enforcement};
use zcash_keys::{address::UnifiedAddress, encoding::encode_payment_address_p, keys::UnifiedFullViewingKey};
use zcash_note_encryption::{try_output_recovery_with_ock, Domain, OutgoingCipherKey};
use zcash_primitives::transaction::Transaction;
use zcash_protocol::{
    consensus::{BranchId, Network},
    memo::{Memo, MemoBytes},
};

/// Prefix of the compact, shareable receipt encoding.
pub const RECEIPT_PREFIX: &str = "zrcpt1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Pool {
    Sapling,
    Orchard,
    /// The NU6.3 shielded pool (live on mainnet since block 3,428,143).
    Ironwood,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Net {
    Main,
    Test,
}

impl Net {
    pub fn params(self) -> Network {
        match self {
            Net::Main => Network::MainNetwork,
            Net::Test => Network::TestNetwork,
        }
    }
    pub fn parse(s: &str) -> Result<Self> {
        match s {
            "main" | "mainnet" => Ok(Net::Main),
            "test" | "testnet" => Ok(Net::Test),
            other => bail!("unknown network {other:?}; use \"main\" or \"test\""),
        }
    }
}

/// The disclosure itself: everything a verifier needs besides the raw transaction.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Receipt {
    pub v: u8,
    pub net: Net,
    pub txid: String,
    pub pool: Pool,
    /// Index of the Sapling output or Orchard action within the transaction.
    pub index: usize,
    /// Hex-encoded outgoing cipher key for this single output.
    pub ock: String,
}

impl Receipt {
    pub fn encode(&self) -> String {
        let json = serde_json::to_vec(self).expect("receipt serializes");
        format!("{RECEIPT_PREFIX}{}", URL_SAFE_NO_PAD.encode(json))
    }

    pub fn decode(s: &str) -> Result<Self> {
        let body = s
            .trim()
            .strip_prefix(RECEIPT_PREFIX)
            .ok_or_else(|| anyhow!("not a receipt: missing {RECEIPT_PREFIX} prefix"))?;
        let json = URL_SAFE_NO_PAD.decode(body).context("receipt is not valid base64url")?;
        let r: Receipt = serde_json::from_slice(&json).context("receipt payload is malformed")?;
        if r.v != 1 {
            bail!("unsupported receipt version {}", r.v);
        }
        Ok(r)
    }

    fn ock_bytes(&self) -> Result<OutgoingCipherKey> {
        let raw = hex::decode(&self.ock).context("ock is not hex")?;
        let arr: [u8; 32] = raw.try_into().map_err(|_| anyhow!("ock must be 32 bytes"))?;
        Ok(OutgoingCipherKey(arr))
    }
}

/// What a receipt reveals once verified against the chain.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Disclosed {
    pub txid: String,
    pub pool: Pool,
    pub index: usize,
    /// Amount in zatoshis (1 ZEC = 100_000_000 zatoshis).
    pub value_zat: u64,
    pub recipient: String,
    /// Memo text, if the memo is UTF-8 text. `None` for empty or binary memos.
    pub memo: Option<String>,
}

/// A receipt created by the sender, together with what it discloses.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreatedReceipt {
    pub receipt: String,
    pub disclosed: Disclosed,
}

/// Parses a raw transaction from hex. The branch id is only consulted for
/// pre-v5 transactions, which carry no branch id in their encoding.
pub fn parse_tx(raw_hex: &str) -> Result<Transaction> {
    let bytes = hex::decode(raw_hex.trim()).context("transaction is not hex")?;
    Transaction::read(&bytes[..], BranchId::Nu6_3).context("could not parse transaction")
}

fn memo_text(bytes: &[u8; 512]) -> Option<String> {
    let mb = MemoBytes::from_bytes(bytes).ok()?;
    match Memo::try_from(mb).ok()? {
        Memo::Text(t) => {
            let s: String = t.into();
            Some(s)
        }
        _ => None,
    }
}

fn encode_orchard(net: Net, addr: orchard::Address) -> String {
    UnifiedAddress::from_receivers(Some(addr), None, None)
        .expect("orchard receiver alone forms a valid UA")
        .encode(&net.params())
}

fn encode_sapling(net: Net, addr: &sapling::PaymentAddress) -> String {
    encode_payment_address_p(&net.params(), addr)
}

/// Sender side: for every shielded output of `tx` that this viewing key can
/// recover (i.e. outputs this wallet created), return one receipt.
///
/// Change outputs back to the sender's own wallet are included; callers
/// usually present the list and let the user pick the payment to disclose.
pub fn create_receipts(tx: &Transaction, ufvk: &str, net: Net) -> Result<Vec<CreatedReceipt>> {
    let ufvk = UnifiedFullViewingKey::decode(&net.params(), ufvk.trim())
        .map_err(|e| anyhow!("invalid unified full viewing key: {e}"))?;
    let txid = tx.txid().to_string();
    let mut out = Vec::new();

    if let (Some(bundle), Some(dfvk)) = (tx.sapling_bundle(), ufvk.sapling()) {
        let domain = SaplingDomain::new(Zip212Enforcement::On);
        let ovks = [dfvk.to_ovk(zip32::Scope::External), dfvk.to_ovk(zip32::Scope::Internal)];
        for (index, output) in bundle.shielded_outputs().iter().enumerate() {
            for ovk in &ovks {
                let ock = <SaplingDomain as Domain>::derive_ock(
                    ovk,
                    output.cv(),
                    &output.cmu().to_bytes(),
                    output.ephemeral_key(),
                );
                if let Some((note, to, memo)) =
                    try_output_recovery_with_ock(&domain, &ock, output, output.out_ciphertext())
                {
                    let receipt = Receipt {
                        v: 1,
                        net,
                        txid: txid.clone(),
                        pool: Pool::Sapling,
                        index,
                        ock: hex::encode(ock.0),
                    };
                    out.push(CreatedReceipt {
                        receipt: receipt.encode(),
                        disclosed: Disclosed {
                            txid: txid.clone(),
                            pool: Pool::Sapling,
                            index,
                            value_zat: note.value().inner(),
                            recipient: encode_sapling(net, &to),
                            memo: memo_text(&memo),
                        },
                    });
                    break;
                }
            }
        }
    }

    if let Some(fvk) = ufvk.orchard() {
        if let Some(bundle) = tx.orchard_bundle() {
            out.extend(recover_actions::<OrchardVersion, _>(bundle.actions().iter(), fvk, net, &txid, Pool::Orchard));
        }
        if let Some(bundle) = tx.ironwood_bundle() {
            out.extend(recover_actions::<IronwoodVersion, _>(bundle.actions().iter(), fvk, net, &txid, Pool::Ironwood));
        }
    }

    Ok(out)
}

/// Verifier side: check `receipt` against the raw transaction and return
/// what it discloses. Fails if the receipt does not belong to this
/// transaction or does not decrypt the referenced output.
pub fn verify_receipt(tx: &Transaction, receipt: &Receipt) -> Result<Disclosed> {
    let txid = tx.txid().to_string();
    if txid != receipt.txid {
        bail!("receipt is for transaction {}, not {txid}", receipt.txid);
    }
    let ock = receipt.ock_bytes()?;

    match receipt.pool {
        Pool::Sapling => {
            let bundle = tx.sapling_bundle().ok_or_else(|| anyhow!("transaction has no Sapling outputs"))?;
            let output = bundle
                .shielded_outputs()
                .get(receipt.index)
                .ok_or_else(|| anyhow!("Sapling output {} does not exist", receipt.index))?;
            let domain = SaplingDomain::new(Zip212Enforcement::On);
            let (note, to, memo) =
                try_output_recovery_with_ock(&domain, &ock, output, output.out_ciphertext())
                    .ok_or_else(|| anyhow!("receipt key does not open this output"))?;
            Ok(Disclosed {
                txid,
                pool: Pool::Sapling,
                index: receipt.index,
                value_zat: note.value().inner(),
                recipient: encode_sapling(receipt.net, &to),
                memo: memo_text(&memo),
            })
        }
        Pool::Orchard => {
            let bundle = tx.orchard_bundle().ok_or_else(|| anyhow!("transaction has no Orchard actions"))?;
            open_action::<OrchardVersion, _>(bundle.actions().iter(), receipt, &ock, txid)
        }
        Pool::Ironwood => {
            let bundle = tx.ironwood_bundle().ok_or_else(|| anyhow!("transaction has no Ironwood actions"))?;
            open_action::<IronwoodVersion, _>(bundle.actions().iter(), receipt, &ock, txid)
        }
    }
}

/// Orchard-family (Orchard, Ironwood) sender-side recovery over a list of actions.
pub fn recover_actions<'a, V: DomainVersion, T: 'a>(
    actions: impl IntoIterator<Item = &'a Action<T>>,
    fvk: &orchard::keys::FullViewingKey,
    net: Net,
    txid: &str,
    pool: Pool,
) -> Vec<CreatedReceipt> {
    let ovks = [
        fvk.to_ovk(orchard::keys::Scope::External),
        fvk.to_ovk(orchard::keys::Scope::Internal),
    ];
    let mut out = Vec::new();
    for (index, action) in actions.into_iter().enumerate() {
        let domain = NoteEncryptionDomain::<V>::for_action(action);
        for ovk in &ovks {
            let ock = <NoteEncryptionDomain<V> as Domain>::derive_ock(
                ovk,
                action.cv_net(),
                &action.cmx().to_bytes(),
                &action.encrypted_note().epk_bytes.into(),
            );
            if let Some((note, to, memo)) =
                try_output_recovery_with_ock(&domain, &ock, action, &action.encrypted_note().out_ciphertext)
            {
                let receipt = Receipt { v: 1, net, txid: txid.to_string(), pool, index, ock: hex::encode(ock.0) };
                out.push(CreatedReceipt {
                    receipt: receipt.encode(),
                    disclosed: Disclosed {
                        txid: txid.to_string(),
                        pool,
                        index,
                        value_zat: note.value().inner(),
                        recipient: encode_orchard(net, to),
                        memo: memo_text(&memo),
                    },
                });
                break;
            }
        }
    }
    out
}

/// Orchard-family verifier-side opening of a single action.
pub fn open_action<'a, V: DomainVersion, T: 'a>(
    actions: impl IntoIterator<Item = &'a Action<T>>,
    receipt: &Receipt,
    ock: &OutgoingCipherKey,
    txid: String,
) -> Result<Disclosed> {
    let action = actions
        .into_iter()
        .nth(receipt.index)
        .ok_or_else(|| anyhow!("action {} does not exist", receipt.index))?;
    let domain = NoteEncryptionDomain::<V>::for_action(action);
    let (note, to, memo) =
        try_output_recovery_with_ock(&domain, ock, action, &action.encrypted_note().out_ciphertext)
            .ok_or_else(|| anyhow!("receipt key does not open this output"))?;
    Ok(Disclosed {
        txid,
        pool: receipt.pool,
        index: receipt.index,
        value_zat: note.value().inner(),
        recipient: encode_orchard(receipt.net, to),
        memo: memo_text(&memo),
    })
}

/// Convenience wrapper: verify an encoded receipt against raw transaction hex.
pub fn verify_encoded(raw_tx_hex: &str, receipt: &str) -> Result<Disclosed> {
    let tx = parse_tx(raw_tx_hex)?;
    verify_receipt(&tx, &Receipt::decode(receipt)?)
}
