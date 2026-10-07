//! zreceipt CLI.
//!
//!   zreceipt create --net test --tx <raw-tx-hex-or-@file> --ufvk <uview...>
//!   zreceipt verify --tx <raw-tx-hex-or-@file> --receipt <zrcpt1...>

use anyhow::{anyhow, bail, Context, Result};
use std::collections::HashMap;

fn arg_value(raw: &str) -> Result<String> {
    match raw.strip_prefix('@') {
        Some(path) => std::fs::read_to_string(path).with_context(|| format!("reading {path}")),
        None => Ok(raw.to_string()),
    }
}

fn parse_flags(args: &[String]) -> Result<HashMap<String, String>> {
    let mut m = HashMap::new();
    let mut it = args.iter();
    while let Some(k) = it.next() {
        let key = k.strip_prefix("--").ok_or_else(|| anyhow!("unexpected argument {k}"))?;
        let v = it.next().ok_or_else(|| anyhow!("--{key} needs a value"))?;
        m.insert(key.to_string(), arg_value(v)?);
    }
    Ok(m)
}

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (cmd, rest) = args.split_first().ok_or_else(|| anyhow!("usage: zreceipt <create|verify> ..."))?;
    let flags = parse_flags(rest)?;
    let get = |k: &str| flags.get(k).cloned().ok_or_else(|| anyhow!("missing --{k}"));

    match cmd.as_str() {
        "create" => {
            let net = zreceipt::Net::parse(flags.get("net").map(String::as_str).unwrap_or("test"))?;
            let tx = zreceipt::parse_tx(&get("tx")?)?;
            let receipts = zreceipt::create_receipts(&tx, &get("ufvk")?, net)?;
            if receipts.is_empty() {
                bail!("this viewing key did not create any shielded output in this transaction");
            }
            println!("{}", serde_json::to_string_pretty(&receipts)?);
        }
        "verify" => {
            let d = zreceipt::verify_encoded(&get("tx")?, &get("receipt")?)?;
            println!("{}", serde_json::to_string_pretty(&d)?);
        }
        other => bail!("unknown command {other}"),
    }
    Ok(())
}
