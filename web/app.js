// Shared browser logic. All cryptography runs locally in WebAssembly;
// the server is only used to fetch the public raw transaction.
import init, { createReceipts, verifyReceipt, inspectReceipt } from "./pkg/zreceipt_wasm.js";

let ready;
export function engine() {
  ready ??= init();
  return ready.then(() => ({ createReceipts, verifyReceipt, inspectReceipt }));
}

export const EXPLORER = {
  test: "https://testnet.zcashexplorer.app/transactions/",
  main: "https://mainnet.zcashexplorer.app/transactions/",
};

export function normalizeTxid(raw) {
  const t = raw.trim().toLowerCase();
  if (!/^[0-9a-f]{64}$/.test(t)) throw new Error("A transaction ID is 64 hexadecimal characters. Check that you copied all of it.");
  return t;
}

/** Fetches the public raw transaction through our proxy. */
export async function fetchTx(txid, net) {
  const res = await fetch(`/api/tx?net=${encodeURIComponent(net)}&txid=${encodeURIComponent(txid)}`);
  const body = await res.json().catch(() => ({}));
  if (!res.ok) throw new Error(body.error || `The explorer did not return this transaction (HTTP ${res.status}).`);
  return body; // { hex, height, time }
}

export function formatZec(zat) {
  const whole = Math.floor(zat / 1e8);
  let frac = String(zat % 1e8).padStart(8, "0").replace(/0+$/, "");
  if (frac.length < 2) frac = frac.padEnd(2, "0");   // money style: 25.00, 0.10
  return `${whole.toLocaleString("en-US")}.${frac}`;
}

export function formatTime(unix) {
  if (!unix) return "";
  return new Date(unix * 1000).toLocaleString("en-GB", {
    year: "numeric", month: "short", day: "numeric", hour: "2-digit", minute: "2-digit", timeZoneName: "short",
  });
}

export const POOL_NAME = { sapling: "Sapling", orchard: "Orchard", ironwood: "Ironwood" };

export function shortAddr(a) {
  return a.length > 34 ? `${a.slice(0, 18)}…${a.slice(-12)}` : a;
}

export function el(tag, attrs = {}, ...kids) {
  const n = document.createElement(tag);
  for (const [k, v] of Object.entries(attrs)) {
    if (k === "class") n.className = v;
    else if (k === "text") n.textContent = v;
    else n.setAttribute(k, v);
  }
  for (const k of kids) if (k != null) n.append(k);
  return n;
}

export const SEAL_SVG = `<svg class="seal" viewBox="0 0 128 128" fill="none" stroke="currentColor" aria-hidden="true">
<defs><path id="sealring" d="M64 64 m-46 0 a46 46 0 1 1 92 0 a46 46 0 1 1 -92 0"/></defs>
<circle cx="64" cy="64" r="60" stroke-width="3" style="fill:var(--sheet);fill-opacity:.9"/><circle cx="64" cy="64" r="36" stroke-width="1.5"/>
<text font-family="Public Sans, sans-serif" font-size="11" font-weight="700" fill="currentColor" stroke="none"><textPath href="#sealring" textLength="284" lengthAdjust="spacing">VERIFIED ON ZCASH • ONE PAYMENT ONLY •</textPath></text>
<path d="M48 65 l11 11 l22 -24" stroke-width="5" stroke-linecap="round" stroke-linejoin="round"/></svg>`;
