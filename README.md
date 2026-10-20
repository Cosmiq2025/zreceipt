# zreceipt

Prove one shielded Zcash payment to an auditor, bank or tax office without revealing your balance, your other payments or your own address.

A receipt is a link. Whoever opens it gets the amount, recipient and memo of exactly one payment, and their browser checks it against the transaction on the Zcash blockchain. Nothing else in the sender's wallet is disclosed.

**Live:** https://zreceipt-web.vercel.app

**Try it:** [verify a real receipt](https://zreceipt-web.vercel.app/verify.html#zrcpt1eyJ2IjoxLCJuZXQiOiJtYWluIiwidHhpZCI6ImZjNGNlMTAyYTg3ODE4ZTUwMGI4M2QxZDY0ZGM3NTUyZDY2ZGM4ZjgwNjA5ZTI0OTI2YTU2YTRhOWQ5OTcxODQiLCJwb29sIjoiaXJvbndvb2QiLCJpbmRleCI6MSwib2NrIjoiYjMyOTZlZGZlMzE4NWIzMjMyYTJjZjBjNGUwNDFiNjNkNTFhZTQ4NTAwZDhhNzkwNGJjNDhhZTBmNWU2NDcwOSJ9) for a 0.001 ZEC payment on Zcash mainnet, in the Ironwood pool ([transaction](https://mainnet.zcashexplorer.app/transactions/fc4ce102a87818e500b83d1d64dc7552d66dc8f80609e24926a56a4a9d997184)).

## How it works

Every shielded output in a Zcash transaction carries an `out_ciphertext`, encrypted under a one-time *outgoing cipher key* (`ock`) that the sender's wallet can derive from its outgoing viewing key. zreceipt:

1. **Create:** takes the raw transaction and the sender's unified full viewing key, derives the `ock` for each output the wallet created, and packages one `ock` with the txid, pool and output index into a receipt (`zrcpt1…`).
2. **Verify:** fetches the raw transaction, checks that it hashes to the txid in the receipt, uses the `ock` to recover the recipient's transmission key and the ephemeral secret, decrypts the note and re-derives its note commitment. The commitment must equal the one recorded in the transaction, so the amount, recipient and memo cannot be altered.

An `ock` opens a single output. It reveals nothing about the sender's other notes or balance, and it cannot be used to spend. (A receipt for a change output would show one of the sender's own addresses; the create page marks those.) Outputs the sender created without an outgoing viewing key cannot be disclosed.

This implements the output-recovery step of the draft [ZIP 311 (Payment Disclosures)](https://zips.z.cash/zip-0311), which specifies Sapling, and applies the same step to **Orchard** and the new **Ironwood** pool introduced by NU6.3, which ZIP 311 does not yet cover. ZIP 311 treats a payment disclosure as complete only with a spend-authority proof, which zreceipt does not implement yet, and it does not yet define an encoding, so the `zrcpt1` format is zreceipt's own.

## Supported

| Pool | Create | Verify | Tests |
|---|---|---|---|
| Ironwood (NU6.3, v6 transactions) | yes | yes | builder round trip, full serialized v6 transaction; one real mainnet payment verified on the live site |
| Orchard | yes | yes | builder round trip (create and verify) |
| Sapling | yes | yes | builder round trip (create and verify); post-Canopy (ZIP 212) notes only; not yet tested on chain |

Receipts must be in the exact encoding zreceipt produces, so an edited link is rejected even when it decodes to the same fields, and changing a field makes verification fail (see `tests/roundtrip.rs`). A transaction does not record its network, so the receipt's network is checked only by where the transaction is fetched from, not when you paste one.

Not yet:

- proving which wallet *sent* the payment (the spend-authority part of ZIP 311);
- checking that the transaction is in a block. The website fetches the transaction and its block height from the zcashexplorer.app block explorer through its own small server function. When you paste a raw transaction instead, the page checks the receipt against it but cannot tell whether it was ever mined, and says so.

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

The mark is a tear-off receipt with one line pulled out of it, in Zcash yellow: the payment you disclose. The site is product-first: black, white and one accent, with the real verify screen as the hero. Type is Onest with JetBrains Mono for keys and hashes, self-hosted, so the site makes no third-party requests.

## Privacy model

- The viewing key is used only in the creator's browser and is never sent anywhere.
- The receipt travels in the URL fragment (`#zrcpt1…`), which browsers do not send to servers.
- The server only fetches public raw transactions by txid from a block explorer, so it and the explorer see which txid is being checked. A verifier who does not want to contact it can paste the raw transaction instead; the page then cannot tell whether the transaction was mined, and says so.

## License

MIT
