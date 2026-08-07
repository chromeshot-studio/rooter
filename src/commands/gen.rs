use crate::cli::{Base64Mode, GenCommand};
use crate::util::format;
use anyhow::{bail, Context, Result};
use base64::{engine::general_purpose, Engine as _};
use md5::Digest as _;
use rand::Rng;
use std::path::PathBuf;

pub fn run(cmd: GenCommand) -> Result<()> {
    match cmd {
        GenCommand::Uuid { count } => gen_uuid(count),
        GenCommand::Password { length, simple } => gen_password(length, simple),
        GenCommand::Hash { text, file, algo } => gen_hash(text, file, &algo),
        GenCommand::Base64 { mode, text, file } => gen_base64(mode, text, file),
        GenCommand::Jwt { token } => decode_jwt(&token),
    }
}

fn gen_uuid(count: u32) -> Result<()> {
    for _ in 0..count.max(1) {
        println!("{}", uuid::Uuid::new_v4());
    }
    Ok(())
}

fn gen_password(length: usize, simple: bool) -> Result<()> {
    let length = length.clamp(4, 256);
    let charset: &[u8] = if simple {
        b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789"
    } else {
        b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789!@#$%^&*()-_=+[]{}"
    };

    let mut rng = rand::thread_rng();
    let password: String = (0..length)
        .map(|_| {
            let idx = rng.gen_range(0..charset.len());
            charset[idx] as char
        })
        .collect();

    println!("{password}");
    Ok(())
}

fn gen_hash(text: Option<String>, file: Option<PathBuf>, algo: &str) -> Result<()> {
    let bytes: Vec<u8> = match (text, file) {
        (_, Some(path)) => std::fs::read(&path).with_context(|| format!("couldn't read {}", path.display()))?,
        (Some(t), None) => t.into_bytes(),
        (None, None) => bail!("provide text to hash or --file <path>"),
    };

    let digest = match algo.to_lowercase().as_str() {
        "sha256" | "sha-256" => {
            let mut hasher = sha2::Sha256::new();
            hasher.update(&bytes);
            hex::encode(hasher.finalize())
        }
        "md5" => {
            let mut hasher = md5::Md5::new();
            hasher.update(&bytes);
            hex::encode(hasher.finalize())
        }
        other => bail!("unknown algorithm '{other}' - use sha256 or md5"),
    };

    println!("{digest}");
    Ok(())
}

fn gen_base64(mode: Base64Mode, text: Option<String>, file: Option<PathBuf>) -> Result<()> {
    let bytes: Vec<u8> = match (text, file) {
        (_, Some(path)) => std::fs::read(&path).with_context(|| format!("couldn't read {}", path.display()))?,
        (Some(t), None) => t.into_bytes(),
        (None, None) => bail!("provide text or --file <path>"),
    };

    match mode {
        Base64Mode::Encode => println!("{}", general_purpose::STANDARD.encode(bytes)),
        Base64Mode::Decode => {
            let text = String::from_utf8_lossy(&bytes).trim().to_string();
            let decoded = general_purpose::STANDARD
                .decode(text)
                .context("input isn't valid base64")?;
            match String::from_utf8(decoded.clone()) {
                Ok(s) => println!("{s}"),
                Err(_) => {
                    format::warn("decoded bytes aren't valid UTF-8, printing hex instead");
                    println!("{}", hex::encode(decoded));
                }
            }
        }
    }
    Ok(())
}

fn decode_jwt(token: &str) -> Result<()> {
    let parts: Vec<&str> = token.split('.').collect();
    if parts.len() < 2 {
        bail!("that doesn't look like a JWT (expected header.payload.signature)");
    }

    let header = decode_segment(parts[0]).context("couldn't decode header")?;
    let payload = decode_segment(parts[1]).context("couldn't decode payload")?;

    format::heading("Header");
    println!("{}", pretty(&header));

    format::heading("Payload");
    println!("{}", pretty(&payload));

    if let Ok(json) = serde_json::from_str::<serde_json::Value>(&payload) {
        if let Some(exp) = json.get("exp").and_then(|v| v.as_i64()) {
            if let Some(dt) = chrono::DateTime::from_timestamp(exp, 0) {
                let now = chrono::Utc::now();
                let status = if dt < now {
                    format!("expired {} ago", crate::commands::why::human_duration((now - dt).num_seconds().max(0) as u64))
                } else {
                    format!("valid for {} more", crate::commands::why::human_duration((dt - now).num_seconds().max(0) as u64))
                };
                format::row("Expiry", format!("{} ({status})", dt.with_timezone(&chrono::Local).format("%Y-%m-%d %H:%M:%S")));
            }
        }
    }

    format::info("");
    format::info("  note: this only decodes the token, it does not verify the signature");
    Ok(())
}

fn decode_segment(segment: &str) -> Result<String> {
    let bytes = general_purpose::URL_SAFE_NO_PAD
        .decode(segment)
        .or_else(|_| general_purpose::URL_SAFE.decode(segment))
        .context("invalid base64url segment")?;
    Ok(String::from_utf8_lossy(&bytes).to_string())
}

fn pretty(raw: &str) -> String {
    serde_json::from_str::<serde_json::Value>(raw)
        .and_then(|v| serde_json::to_string_pretty(&v))
        .unwrap_or_else(|_| raw.to_string())
}
