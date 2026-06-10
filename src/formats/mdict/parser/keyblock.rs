use std::io::Read;

use adler32::adler32;
use encoding::label::encoding_from_whatwg_label;
use flate2::read::ZlibDecoder;
use ripemd::{Digest, Ripemd128};

use super::header::Header;
use crate::util::fast_decrypt;

// ── simple sequential byte reader ────────────────────────────────────────────

struct Reader<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    fn new(data: &'a [u8]) -> Self {
        Self { data, pos: 0 }
    }
    fn u8(&mut self) -> u8 {
        let v = self.data[self.pos];
        self.pos += 1;
        v
    }
    fn be_u16(&mut self) -> u16 {
        let v = u16::from_be_bytes(self.data[self.pos..self.pos + 2].try_into().unwrap());
        self.pos += 2;
        v
    }
    fn be_u32(&mut self) -> u32 {
        let v = u32::from_be_bytes(self.data[self.pos..self.pos + 4].try_into().unwrap());
        self.pos += 4;
        v
    }
    fn be_u64(&mut self) -> u64 {
        let v = u64::from_be_bytes(self.data[self.pos..self.pos + 8].try_into().unwrap());
        self.pos += 8;
        v
    }
    fn bytes(&mut self, n: usize) -> &'a [u8] {
        let s = &self.data[self.pos..self.pos + n];
        self.pos += n;
        s
    }
    fn skip(&mut self, n: usize) {
        self.pos += n;
    }
    fn remaining(&self) -> &'a [u8] {
        &self.data[self.pos..]
    }
}

// ── public types ─────────────────────────────────────────────────────────────

pub struct KeyBlockHeader {
    pub key_block_info_len: usize,
    pub key_blocks_len: usize,
}

pub struct KeyBlockSize {
    pub csize: usize,
    pub dsize: usize,
}

/// A key entry: the word text and its byte offset in the decompressed record buffer.
pub struct KeyEntry {
    pub text: String,
    pub record_offset: usize,
}

// ── key block header ─────────────────────────────────────────────────────────

pub fn parse_key_block_header<'a>(data: &'a [u8], header: &Header) -> (&'a [u8], KeyBlockHeader) {
    let mut r = Reader::new(data);
    let kbh = if header.version >= 2 {
        let buf = r.bytes(40);
        let checksum = r.be_u32();
        assert_eq!(
            adler32(buf).unwrap(),
            checksum,
            "key block header checksum mismatch"
        );
        let mut br = Reader::new(buf);
        let _block_num = br.be_u64();
        let _entry_num = br.be_u64();
        let _info_dsize = br.be_u64();
        let info_len = br.be_u64() as usize;
        let blocks_len = br.be_u64() as usize;
        KeyBlockHeader {
            key_block_info_len: info_len,
            key_blocks_len: blocks_len,
        }
    } else {
        let buf = r.bytes(16);
        let mut br = Reader::new(buf);
        let _block_num = br.be_u32();
        let _entry_num = br.be_u32();
        let info_len = br.be_u32() as usize;
        let blocks_len = br.be_u32() as usize;
        KeyBlockHeader {
            key_block_info_len: info_len,
            key_blocks_len: blocks_len,
        }
    };
    (r.remaining(), kbh)
}

// ── key block info (compressed sizes) ────────────────────────────────────────

pub fn parse_key_block_info<'a>(
    data: &'a [u8],
    info_len: usize,
    header: &Header,
) -> (&'a [u8], Vec<KeyBlockSize>) {
    let buf = &data[..info_len];
    let rest = &data[info_len..];

    let sizes = if header.version >= 2 {
        // First 4 bytes must be 0x02000000; may be encrypted+zlib-compressed.
        assert_eq!(
            &buf[0..4],
            b"\x02\x00\x00\x00",
            "key block info magic mismatch"
        );
        let mut decompressed = Vec::new();
        if header.encrypted & 0x02 != 0 {
            // Encrypt key: ripemd128(checksum_bytes ++ 0x3695_le32)
            let mut md = Ripemd128::new();
            let mut seed = buf[4..8].to_vec();
            seed.extend_from_slice(&0x3695u32.to_le_bytes());
            md.update(&seed);
            let key = md.finalize();
            let decrypted = fast_decrypt(&buf[8..], &key);
            ZlibDecoder::new(&decrypted[..])
                .read_to_end(&mut decompressed)
                .unwrap();
        } else {
            ZlibDecoder::new(&buf[8..])
                .read_to_end(&mut decompressed)
                .unwrap();
        }
        decode_info_v2(&decompressed, &header.encoding)
    } else {
        decode_info_v1(buf, &header.encoding)
    };

    (rest, sizes)
}

/// V1 key block info entry: be_u32 entries | u8 first_char_count | first_chars | u8 last_char_count | last_chars | be_u32 csize | be_u32 dsize
fn decode_info_v1(data: &[u8], encoding: &str) -> Vec<KeyBlockSize> {
    let (mult, term) = char_widths(encoding);
    let mut r = Reader::new(data);
    let mut out = Vec::new();
    while r.pos < r.data.len() {
        let _entries = r.be_u32();
        let first_chars = r.u8() as usize;
        r.skip(first_chars * mult + term);
        let last_chars = r.u8() as usize;
        r.skip(last_chars * mult + term);
        let csize = r.be_u32() as usize;
        let dsize = r.be_u32() as usize;
        out.push(KeyBlockSize { csize, dsize });
    }
    out
}

/// V2 key block info entry: be_u64 entries | be_u16 first_char_count | first_chars | be_u16 last_char_count | last_chars | be_u64 csize | be_u64 dsize
fn decode_info_v2(data: &[u8], encoding: &str) -> Vec<KeyBlockSize> {
    let (mult, term) = char_widths(encoding);
    let mut r = Reader::new(data);
    let mut out = Vec::new();
    while r.pos < r.data.len() {
        let _entries = r.be_u64();
        let first_chars = r.be_u16() as usize;
        r.skip(first_chars * mult + term);
        let last_chars = r.be_u16() as usize;
        r.skip(last_chars * mult + term);
        let csize = r.be_u64() as usize;
        let dsize = r.be_u64() as usize;
        out.push(KeyBlockSize { csize, dsize });
    }
    out
}

// ── key blocks ───────────────────────────────────────────────────────────────

pub fn parse_key_blocks<'a>(
    data: &'a [u8],
    blocks_len: usize,
    header: &Header,
    sizes: &[KeyBlockSize],
) -> (&'a [u8], Vec<KeyEntry>) {
    let mut buf = &data[..blocks_len];
    let rest = &data[blocks_len..];
    let offset_size = if header.version >= 2 { 8 } else { 4 };

    let mut entries = Vec::new();
    for size in sizes {
        let decompressed = decompress_block(buf, size.csize, size.dsize);
        buf = &buf[size.csize..];
        decode_key_entries(&decompressed, &header.encoding, offset_size, &mut entries);
    }

    (rest, entries)
}

/// Decompress one key block (encrypt type in low nibble of first le_u32).
fn decompress_block(buf: &[u8], csize: usize, dsize: usize) -> Vec<u8> {
    let enc = u32::from_le_bytes(buf[0..4].try_into().unwrap());
    let enc_method = (enc >> 4) & 0xf;
    let comp_method = enc & 0xf;
    let checksum = &buf[4..8];
    let payload = &buf[8..csize];

    let mut md = Ripemd128::new();
    md.update(checksum);
    let key = md.finalize();

    let plain: Vec<u8> = match enc_method {
        0 => payload.to_vec(),
        1 => fast_decrypt(payload, &key),
        _ => payload.to_vec(),
    };

    match comp_method {
        0 => plain,
        1 => {
            let mut out = vec![0u8; dsize];
            let (_, err) = rust_lzo::LZOContext::decompress_to_slice(&plain, &mut out);
            assert!(
                err == rust_lzo::LZOError::OK,
                "LZO key block decompression failed"
            );
            out
        }
        2 => {
            let mut v = Vec::new();
            ZlibDecoder::new(&plain[..]).read_to_end(&mut v).unwrap();
            v
        }
        _ => panic!("unknown compression method {comp_method}"),
    }
}

/// Walk one decompressed key block and collect (offset, text) pairs.
fn decode_key_entries(data: &[u8], encoding: &str, offset_size: usize, out: &mut Vec<KeyEntry>) {
    let utf16 = is_utf16(encoding);
    let term = if utf16 { 2 } else { 1 };
    let mut pos = 0;

    while pos < data.len() {
        let offset = if offset_size == 8 {
            let v = u64::from_be_bytes(data[pos..pos + 8].try_into().unwrap()) as usize;
            pos += 8;
            v
        } else {
            let v = u32::from_be_bytes(data[pos..pos + 4].try_into().unwrap()) as usize;
            pos += 4;
            v
        };

        // Find null terminator
        let text_len = if utf16 {
            let mut l = 0;
            while pos + l + 1 < data.len() {
                if data[pos + l] == 0 && data[pos + l + 1] == 0 {
                    break;
                }
                l += 2;
            }
            l
        } else {
            let mut l = 0;
            while pos + l < data.len() && data[pos + l] != 0 {
                l += 1;
            }
            l
        };

        let text = decode_text(&data[pos..pos + text_len], encoding);
        out.push(KeyEntry {
            text,
            record_offset: offset,
        });
        pos += text_len + term;
    }
}

fn decode_text(bytes: &[u8], encoding: &str) -> String {
    if is_utf16(encoding) {
        let words: Vec<u16> = bytes
            .chunks_exact(2)
            .map(|c| u16::from_le_bytes([c[0], c[1]]))
            .collect();
        String::from_utf16_lossy(&words).to_string()
    } else if encoding.is_empty() || encoding.eq_ignore_ascii_case("utf-8") {
        String::from_utf8_lossy(bytes).into_owned()
    } else {
        encoding_from_whatwg_label(encoding)
            .and_then(|enc| enc.decode(bytes, encoding::DecoderTrap::Ignore).ok())
            .unwrap_or_else(|| String::from_utf8_lossy(bytes).into_owned())
    }
}

fn char_widths(encoding: &str) -> (usize, usize) {
    if is_utf16(encoding) { (2, 2) } else { (1, 1) }
}

fn is_utf16(encoding: &str) -> bool {
    let lower = encoding.to_ascii_lowercase();
    lower.contains("utf-16") || lower.contains("utf16")
}
