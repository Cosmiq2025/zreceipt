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

export const CHECK_SVG = `<svg viewBox="0 0 16 16" fill="none" aria-hidden="true"><circle cx="8" cy="8" r="8" fill="currentColor"/><path d="M4.6 8.2l2.2 2.2 4.6-4.8" stroke="#fff" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round"/></svg>`;
export const CROSS_SVG = `<svg viewBox="0 0 16 16" fill="none" aria-hidden="true"><circle cx="8" cy="8" r="8" fill="currentColor"/><path d="M5.3 5.3l5.4 5.4M10.7 5.3l-5.4 5.4" stroke="#fff" stroke-width="1.7" stroke-linecap="round"/></svg>`;
export const LOCK_SVG = `<svg viewBox="0 0 16 16" fill="none" aria-hidden="true"><rect x="3" y="7" width="10" height="7" rx="1.5" fill="currentColor"/><path d="M5.5 7V5a2.5 2.5 0 015 0v2" stroke="currentColor" stroke-width="1.6"/></svg>`;

const NET_NAME = { main: "Zcash mainnet", test: "Zcash testnet" };

/** The verified-payment view, shared by verify.html. */
export function renderResult(d, r, meta) {
  const root = el("div", { class: "result" });
  const badge = el("span", { class: "badge" });
  badge.innerHTML = CHECK_SVG;
  badge.append(`Verified on ${NET_NAME[r.net]}`);
  const amt = el("p", { class: "amount" }, `${formatZec(d.value_zat)} `, el("span", { text: "ZEC" }));
  const memo = el("p", { class: "memo", text: d.memo || "No memo" });
  const rows = el("dl", { class: "rows" });
  const row = (k, v, { href, mono } = {}) => {
    if (!v) return;
    const dd = el("dd", mono ? { class: "mono" } : {});
    if (href) dd.append(el("a", { href, target: "_blank", rel: "noopener", text: v })); else dd.textContent = v;
    rows.append(el("div", {}, el("dt", { text: k }), dd));
  };
  row("Paid to", d.recipient, { mono: true });
  row("Transaction", d.txid, { mono: true, href: EXPLORER[r.net] + d.txid });
  row("Block", meta.height ? `${meta.height.toLocaleString("en-US")} on ${formatTime(meta.time)}` : "");
  row("Pool", `${POOL_NAME[d.pool]}, output ${d.index}`);
  const hidden = el("div", { class: "hidden-list" }, el("h3", { text: "Not shared with you" }));
  const ul = el("ul");
  for (const t of ["Sender's balance", "Sender's other payments", "Sender's address", "Rest of the transaction"]) {
    const li = el("li"); li.innerHTML = LOCK_SVG; li.append(t); ul.append(li);
  }
  hidden.append(ul);
  root.append(badge, amt, memo, rows, hidden);
  return root;
}
