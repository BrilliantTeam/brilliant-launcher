use crate::err::R;
use crate::bail;
use anyhow::Context;
use serde::Serialize;
use serde_json::Value;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

const TIMEOUT: Duration = Duration::from_secs(5);
const DEFAULT_PORT: u16 = 25565;
const PROTOCOL_UNKNOWN: i32 = -1;
const MAX_PACKET: usize = 2 * 1024 * 1024;

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ServerStatus {
    pub host: String,
    pub online: i64,
    pub max: i64,
    pub motd: String,
    pub version: String,
    pub favicon: String,
    pub latency_ms: u128,
}

fn write_varint(buf: &mut Vec<u8>, value: i32) {
    let mut v = value as u32;
    loop {
        let mut byte = (v & 0x7F) as u8;
        v >>= 7;
        if v != 0 {
            byte |= 0x80;
        }
        buf.push(byte);
        if v == 0 {
            break;
        }
    }
}

async fn read_varint(stream: &mut TcpStream) -> R<i32> {
    let mut result: i32 = 0;
    for shift in 0..5 {
        let mut byte = [0u8; 1];
        stream.read_exact(&mut byte).await.context("讀取封包長度失敗")?;
        result |= ((byte[0] & 0x7F) as i32) << (shift * 7);
        if byte[0] & 0x80 == 0 {
            return Ok(result);
        }
    }
    bail!("VarInt 過長，可能不是 Minecraft 伺服器")
}

fn framed(payload: Vec<u8>) -> Vec<u8> {
    let mut out = Vec::with_capacity(payload.len() + 5);
    write_varint(&mut out, payload.len() as i32);
    out.extend(payload);
    out
}

fn plain_text(value: &Value) -> String {
    match value {
        Value::String(s) => s.clone(),
        Value::Array(list) => list.iter().map(plain_text).collect(),
        Value::Object(obj) => {
            let mut out = obj.get("text").and_then(|v| v.as_str()).unwrap_or("").to_string();
            if let Some(extra) = obj.get("extra") {
                out.push_str(&plain_text(extra));
            }
            out
        }
        _ => String::new(),
    }
}

fn strip_codes(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars();
    while let Some(c) = chars.next() {
        if c == '§' {
            chars.next();
        } else {
            out.push(c);
        }
    }
    out
}

pub async fn ping(target: &str) -> R<ServerStatus> {
    let (host, port) = match target.rsplit_once(':') {
        Some((h, p)) => (h.to_string(), p.parse().unwrap_or(DEFAULT_PORT)),
        None => (target.to_string(), DEFAULT_PORT),
    };

    let started = std::time::Instant::now();
    let mut stream = tokio::time::timeout(TIMEOUT, TcpStream::connect((host.as_str(), port)))
        .await
        .with_context(|| format!("連線 {host}:{port} 逾時"))?
        .with_context(|| format!("連線 {host}:{port} 失敗"))?;
    stream.set_nodelay(true).ok();

    let mut handshake = vec![0x00u8];
    write_varint(&mut handshake, PROTOCOL_UNKNOWN);
    write_varint(&mut handshake, host.len() as i32);
    handshake.extend(host.as_bytes());
    handshake.extend(port.to_be_bytes());
    write_varint(&mut handshake, 1);

    stream.write_all(&framed(handshake)).await?;
    stream.write_all(&framed(vec![0x00])).await?;
    stream.flush().await?;

    let json = tokio::time::timeout(TIMEOUT, async {
        let _len = read_varint(&mut stream).await?;
        let packet_id = read_varint(&mut stream).await?;
        if packet_id != 0x00 {
            bail!("非預期的回應封包 {packet_id}");
        }
        let str_len = read_varint(&mut stream).await?;
        if str_len < 0 || str_len as usize > MAX_PACKET {
            bail!("回應長度異常：{str_len}");
        }
        let mut buf = vec![0u8; str_len as usize];
        stream.read_exact(&mut buf).await?;
        Ok::<Vec<u8>, crate::err::Error>(buf)
    })
    .await
    .with_context(|| format!("{host}:{port} 沒有回應"))??;

    let value: Value = serde_json::from_slice(&json).context("伺服器回應不是合法 JSON")?;
    Ok(ServerStatus {
        host: target.to_string(),
        online: value.pointer("/players/online").and_then(|v| v.as_i64()).unwrap_or(0),
        max: value.pointer("/players/max").and_then(|v| v.as_i64()).unwrap_or(0),
        motd: strip_codes(&plain_text(value.get("description").unwrap_or(&Value::Null))),
        version: value
            .pointer("/version/name")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        favicon: value.get("favicon").and_then(|v| v.as_str()).unwrap_or("").to_string(),
        latency_ms: started.elapsed().as_millis(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn varint_matches_protocol_spec() {
        let cases: [(i32, &[u8]); 5] = [
            (0, &[0x00]),
            (1, &[0x01]),
            (127, &[0x7F]),
            (128, &[0x80, 0x01]),
            (-1, &[0xFF, 0xFF, 0xFF, 0xFF, 0x0F]),
        ];
        for (value, expected) in cases {
            let mut buf = Vec::new();
            write_varint(&mut buf, value);
            assert_eq!(buf, expected, "varint({value})");
        }
    }

    #[test]
    fn motd_flattens_chat_components() {
        let v: Value = serde_json::json!({
            "text": "§a輝煌",
            "extra": [{ "text": "・" }, { "text": "Brilliant" }]
        });
        assert_eq!(strip_codes(&plain_text(&v)), "輝煌・Brilliant");
    }
}
