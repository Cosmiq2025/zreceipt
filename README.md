# zreceipt

Prove one shielded Zcash payment to an auditor, bank or tax office without revealing your balance, your other payments or your own address.

A receipt is a link. Whoever opens it gets the amount, recipient and memo of exactly one payment, and their browser checks that against the transaction on the Zcash blockchain. Nothing else in the sender's wallet is disclosed, and nobody has to trust this website.

## How it works

Every shielded output in a Zcash transaction carries an `out_ciphertext`, encrypted under a one-time *outgoing cipher key* (`ock`) that the sender's wallet can derive from its outgoing viewing key. zreceipt:

1. **Create:** takes the raw transaction and the sender's unified full viewing key, derives the `ock` for each output the wallet created, and packages one `ock` with the txid, pool and output index into a receipt (`zrcpt1…`).
2. **Verify:** fetches the raw transaction, uses the `ock` to recover the recipient's transmission key and the ephemeral secret, decrypts the note and re-derives its note commitment. The commitment must equal the one recorded in the transaction, so the amount, recipient and memo cannot be altered.

An `ock` opens a single output. It reveals nothing about the sender's other notes, balance or address, and it cannot be used to spend.

This implements the output-disclosure part of the draft [ZIP 311 (Payment Disclosures)](https://zips.z.cash/zip-0311), which specifies Sapling and has no reference implementation, and applies the same construction to **Orchard** and the new **Ironwood** pool introduced by NU6.3.

## Supported

| Pool | Create | Verify | Tests |
|---|---|---|---|
| Ironwood (NU6.3, v6 transactions) | yes | yes | builder round trip, full serialized v6 transaction |
| Orchard | yes | yes | builder round trip |
| Sapling | yes | yes | same code path, not yet tested on chain |

Not yet: proving which wallet *sent* the payment (the spend-authority part of ZIP 311).

## Layout

```
src/lib.rs        engine: create_receipts, verify_receipt, receipt encoding
src/main.rs       command-line tool
tests/            cryptographic round-trip and full-transaction tests
wasm/             WebAssembly bindings used by the website
web/              static site + one serverless function that fetches raw transactions
web/brand/        logo files (SVG and PNG)
tools/brand.py    generates the logo files
```

## Run

```sh
cargo test --release

# command line
cargo run --release -- create --net test --tx @tx.hex --ufvk uviewtest1...
cargo run --release -- verify --tx @tx.hex --receipt zrcpt1...

# website, locally
cargo build --release -p zreceipt-wasm --target wasm32-unknown-unknown
wasm-bindgen target/wasm32-unknown-unknown/release/zreceipt_wasm.wasm --target web --out-dir web/pkg
node web/dev.mjs        # http://localhost:8787
```

The built WebAssembly is committed in `web/pkg`, so deploying the `web` folder needs no Rust toolchain.

## Brand

The mark is a tear-off receipt with one line pulled out of it, in Zcash yellow: the payment you disclose. The site is product-first: black, white and one accent, with the real verify screen as the hero. Type is Instrument Sans with IBM Plex Mono for keys and hashes, self-hosted, so the site makes no third-party requests.

## Privacy model

- The viewing key is used only in the creator's browser and is never sent anywhere.
- The receipt travels in the URL fragment (`#zrcpt1…`), which browsers do not send to servers.
- The server only fetches public raw transactions by txid from a block explorer. A verifier who does not want to contact it can paste the raw transaction instead.

## License

MIT OR Apache-2.0
