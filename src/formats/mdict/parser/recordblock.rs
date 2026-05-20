use std::io::Read;

use flate2::read::ZlibDecoder;
use ripemd::{Digest, Ripemd128};

use crate::util::fast_decrypt;

pub struct RecordBlockSize {
    pub csize: usize,
    pub dsize: usize,
}

pub fn parse_record_block_sizes(data: &[u8], version: u8) -> (&[u8], Vec<RecordBlockSize>) {
    if version >= 2 {
        let records_num = u64::from_be_bytes(data[0..8].try_into().unwrap()) as usize;
        let _entries_num = u64::from_be_bytes(data[8..16].try_into().unwrap());
        let info_len = u64::from_be_bytes(data[16..24].try_into().unwrap()) as usize;
        let _buf_len = u64::from_be_bytes(data[24..32].try_into().unwrap());
        assert_eq!(records_num * 16, info_len);
        let mut pos = 32;
        let mut sizes = Vec::with_capacity(records_num);
        for _ in 0..records_num {
            let csize = u64::from_be_bytes(data[pos..pos + 8].try_into().unwrap()) as usize;
            let dsize =
                u64::from_be_bytes(data[pos + 8..pos + 16].try_into().unwrap()) as usize;
            sizes.push(RecordBlockSize { csize, dsize });
            pos += 16;
        }
        (&data[pos..], sizes)
    } else {
        let records_num = u32::from_be_bytes(data[0..4].try_into().unwrap()) as usize;
        let _entries_num = u32::from_be_bytes(data[4..8].try_into().unwrap());
        let info_len = u32::from_be_bytes(data[8..12].try_into().unwrap()) as usize;
        let _buf_len = u32::from_be_bytes(data[12..16].try_into().unwrap());
        assert_eq!(records_num * 8, info_len);
        let mut pos = 16;
        let mut sizes = Vec::with_capacity(records_num);
        for _ in 0..records_num {
            let csize = u32::from_be_bytes(data[pos..pos + 4].try_into().unwrap()) as usize;
            let dsize = u32::from_be_bytes(data[pos + 4..pos + 8].try_into().unwrap()) as usize;
            sizes.push(RecordBlockSize { csize, dsize });
            pos += 8;
        }
        (&data[pos..], sizes)
    }
}

/// Decompress one record block. Returns `Err` on corrupt/unknown data
/// so callers can skip the block rather than crashing.
///
/// Block layout: le_u32 enc_flags | 4-byte checksum | compressed payload
/// High nibble of enc_flags = encryption method, low nibble = compression.
pub fn decompress_record_block(
    buf: &[u8],
    csize: usize,
    dsize: usize,
) -> Result<Vec<u8>, &'static str> {
    if csize < 8 || buf.len() < csize {
        return Err("block too small");
    }
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
        0 => Ok(plain),
        1 => {
            let mut out = vec![0u8; dsize];
            let (_, err) = rust_lzo::LZOContext::decompress_to_slice(&plain, &mut out);
            if err != rust_lzo::LZOError::OK {
                return Err("LZO decompress failed");
            }
            Ok(out)
        }
        2 => {
            let mut v = Vec::with_capacity(dsize);
            ZlibDecoder::new(&plain[..])
                .read_to_end(&mut v)
                .map_err(|_| "zlib decompress failed")?;
            Ok(v)
        }
        _ => Err("unknown compression method"),
    }
}
