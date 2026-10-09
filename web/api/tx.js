// Vercel serverless function: returns the public raw transaction for a txid.
// It only proxies public blockchain data; receipts and viewing keys never reach it.
const SOURCES = {
  test: "https://testnet.zcashexplorer.app/transactions/",
  main: "https://mainnet.zcashexplorer.app/transactions/",
};

export default async function handler(req, res) {
  const { txid = "", net = "test" } = req.query;
  if (!/^[0-9a-f]{64}$/.test(String(txid))) {
    return res.status(400).json({ error: "A transaction ID is 64 hexadecimal characters." });
  }
  const base = SOURCES[net];
  if (!base) return res.status(400).json({ error: "Network must be test or main." });

  try {
    const r = await fetch(`${base}${txid}/raw`, { headers: { accept: "application/json", "user-agent": "Mozilla/5.0 (compatible; zreceipt/1.0; +https://github.com/zreceipt)" } });
    if (r.status === 404) {
      return res.status(404).json({ error: `No transaction with this ID on ${net === "main" ? "mainnet" : "testnet"}. Check the network.` });
    }
    if (!r.ok) return res.status(502).json({ error: `The block explorer answered with HTTP ${r.status}. Try again, or paste the raw transaction.` });
    const j = await r.json();
    if (!j.hex) return res.status(502).json({ error: "The block explorer did not include the raw transaction." });
    res.setHeader("Cache-Control", j.height ? "public, max-age=86400" : "no-store");
    return res.status(200).json({ hex: j.hex, height: j.height ?? null, time: j.blocktime ?? j.time ?? null });
  } catch {
    return res.status(502).json({ error: "Could not reach the block explorer. Try again, or paste the raw transaction." });
  }
}
