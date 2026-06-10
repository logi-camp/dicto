use std::collections::HashMap;

use adler32::adler32;
use encoding::{DecoderTrap, Encoding, all::UTF_16LE};
use tracing::info;

pub struct Header {
    pub version: u8,
    /// Encryption bitfield: bit 0 = record blocks, bit 1 = key info block.
    pub encrypted: u8,
    /// Text encoding for MDX key strings, e.g. "UTF-8". Empty → treat as UTF-8.
    pub encoding: String,
}

/// Parse the MDX/MDD file header.
///
/// Layout: be_u32 length | UTF-16LE XML-ish attrs | le_u32 adler32 checksum.
/// Returns remaining bytes after the header.
pub fn parse_header(data: &[u8]) -> (&[u8], Header) {
    let len = u32::from_be_bytes(data[0..4].try_into().unwrap()) as usize;
    let buf = &data[4..4 + len];
    let checksum = u32::from_le_bytes(data[4 + len..8 + len].try_into().unwrap());
    assert_eq!(adler32(buf).unwrap(), checksum, "header adler32 mismatch");

    let xml = UTF_16LE
        .decode(buf, DecoderTrap::Strict)
        .expect("header is not valid UTF-16LE");

    let attrs = parse_attrs(&xml);
    info!("mdict header attrs: {:?}", attrs);

    let version = attrs
        .get("GeneratedByEngineVersion")
        .and_then(|v| v.trim().chars().next())
        .and_then(|c| c.to_digit(10))
        .unwrap_or(2) as u8;

    let encrypted = attrs
        .get("Encrypted")
        .and_then(|v| v.trim().parse::<u8>().ok())
        .unwrap_or(0);

    let encoding = attrs.get("Encoding").cloned().unwrap_or_default();

    (
        &data[8 + len..],
        Header {
            version,
            encrypted,
            encoding,
        },
    )
}

/// Scan `key="value"` pairs from the MDX XML-ish header string.
fn parse_attrs(xml: &str) -> HashMap<String, String> {
    let mut map = HashMap::new();
    let mut s = xml;
    loop {
        let Some(eq) = s.find("=\"") else { break };
        let key = s[..eq]
            .split_ascii_whitespace()
            .next_back()
            .unwrap_or("")
            .to_string();
        s = &s[eq + 2..];
        let Some(close) = s.find('"') else { break };
        if !key.is_empty() {
            map.insert(key, s[..close].to_string());
        }
        s = &s[close + 1..];
    }
    map
}
