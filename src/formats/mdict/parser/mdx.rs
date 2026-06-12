use super::header::parse_header;
use super::keyblock::{KeyEntry, parse_key_block_header, parse_key_block_info, parse_key_blocks};
use super::recordblock::{RecordBlockSize, parse_record_block_sizes};

/// Location of one dictionary entry within the source file.
pub struct MdxEntry {
    pub text: String,
    /// Absolute byte offset of the compressed block within the source .mdx file.
    pub file_offset: u64,
    pub block_csize: u32,
    pub block_dsize: u32,
    pub start_in_block: u32,
    pub end_in_block: u32,
}

pub struct Mdx {
    pub entries: Vec<MdxEntry>,
}

impl Mdx {
    pub fn parse(data: &[u8]) -> anyhow::Result<Self> {
        let file_start = data.as_ptr() as usize;
        let (data, header) = parse_header(data);
        let (data, kbh) = parse_key_block_header(data, &header)?;
        let (data, sizes) =
            parse_key_block_info(data, kbh.key_block_info_len, &header, kbh.block_num)?;
        let (data, entries) = parse_key_blocks(data, kbh.key_blocks_len, &header, &sizes)?;
        let (data, block_sizes) = parse_record_block_sizes(data, header.version);

        let record_section_start = data.as_ptr() as usize - file_start;
        let entries = compute_entries(&entries, &block_sizes, record_section_start);
        Ok(Mdx { entries })
    }
}

fn compute_entries(
    keys: &[KeyEntry],
    block_sizes: &[RecordBlockSize],
    record_section_start: usize,
) -> Vec<MdxEntry> {
    let mut out = Vec::with_capacity(keys.len());
    let mut key_idx = 0;
    let mut debuf_offset = 0usize;
    let mut buf_offset = 0usize;

    for block in block_sizes {
        while key_idx < keys.len() {
            let entry = &keys[key_idx];
            if entry.record_offset >= debuf_offset + block.dsize {
                break;
            }

            let end_in_block = if key_idx + 1 < keys.len() {
                let next = &keys[key_idx + 1];
                if next.record_offset < debuf_offset + block.dsize {
                    next.record_offset - debuf_offset
                } else {
                    block.dsize
                }
            } else {
                block.dsize
            };

            out.push(MdxEntry {
                text: entry.text.clone(),
                file_offset: (record_section_start + buf_offset) as u64,
                block_csize: block.csize as u32,
                block_dsize: block.dsize as u32,
                start_in_block: (entry.record_offset - debuf_offset) as u32,
                end_in_block: end_in_block as u32,
            });
            key_idx += 1;
        }
        debuf_offset += block.dsize;
        buf_offset += block.csize;
    }

    out
}
