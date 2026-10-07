// Local dev server: serves the site and runs api/tx.js the way Vercel does.
// If dev-fixture.json exists, its transaction is served without the network.
//   node dev.mjs  ->  http://localhost:8787
import { createServer } from "node:http";
import { readFile } from "node:fs/promises";
import { existsSync } from "node:fs";
import { extname, join, normalize } from "node:path";
import { fileURLToPath } from "node:url";
import handler from "./api/tx.js";

const root = fileURLToPath(new URL(".", import.meta.url));
const types = { ".html": "text/html", ".css": "text/css", ".js": "text/javascript", ".wasm": "application/wasm", ".json": "application/json", ".svg": "image/svg+xml" };
const fixturePath = join(root, "dev-fixture.json");
const fixture = existsSync(fixturePath) ? JSON.parse(await readFile(fixturePath, "utf8")) : null;

createServer(async (req, res) => {
  const url = new URL(req.url, "http://localhost");
  if (url.pathname === "/api/tx") {
    const query = Object.fromEntries(url.searchParams);
    if (fixture && query.txid === fixture.txid) {
      res.writeHead(200, { "content-type": "application/json" });
      return res.end(JSON.stringify({ hex: fixture.hex, height: 4200000, time: 1791460800 }));
    }
    const shim = {
      status(c) { res.statusCode = c; return shim; },
      setHeader: (k, v) => res.setHeader(k, v),
      json(o) { res.setHeader("content-type", "application/json"); res.end(JSON.stringify(o)); },
    };
    return handler({ query }, shim);
  }
  let p = normalize(decodeURIComponent(url.pathname)).replace(/^(\.\.[/\\])+/, "");
  if (p.endsWith("/")) p += "index.html";
  try {
    const body = await readFile(join(root, p));
    res.writeHead(200, { "content-type": types[extname(p)] || "application/octet-stream" });
    res.end(body);
  } catch {
    res.writeHead(404); res.end("Not found");
  }
}).listen(8787, () => console.log("http://localhost:8787"));
