pub mod parser;

use std::fs::{self, File};
use std::io::{BufWriter, Write};
use std::path::PathBuf;
use std::sync::{Arc, RwLock};

use fst::automaton::{AlwaysMatch, Levenshtein, Str as FstStr};
use fst::{Automaton, IntoStreamer, Map, MapBuilder, Streamer};
use memmap2::Mmap;
use tracing::{info, warn};

use crate::dictionary::Dictionary;
use parser::mdd::{Mdd, normalize_path};
use parser::mdx::Mdx;
use parser::recordblock::decompress_record_block;

// ── offset record (24 bytes per entry) ───────────────────────────────────────

/// Packed encoding of where one entry lives in the source file.
///
/// Layout (little-endian):
///   [0..8]   file_offset    u64 — byte offset of compressed block in source file
///   [8..12]  block_csize    u32
///   [12..16] block_dsize    u32
///   [16..20] start_in_block u32
///   [20..24] end_in_block   u32
const REC_LEN: usize = 24;

fn encode_offset(file_offset: u64, csize: u32, dsize: u32, start: u32, end: u32) -> [u8; REC_LEN] {
    let mut b = [0u8; REC_LEN];
    b[0..8].copy_from_slice(&file_offset.to_le_bytes());
    b[8..12].copy_from_slice(&csize.to_le_bytes());
    b[12..16].copy_from_slice(&dsize.to_le_bytes());
    b[16..20].copy_from_slice(&start.to_le_bytes());
    b[20..24].copy_from_slice(&end.to_le_bytes());
    b
}

fn decode_offset(b: &[u8]) -> Option<(u64, u32, u32, u32, u32)> {
    if b.len() < REC_LEN { return None; }
    let file_offset = u64::from_le_bytes(b[0..8].try_into().unwrap());
    let csize       = u32::from_le_bytes(b[8..12].try_into().unwrap());
    let dsize       = u32::from_le_bytes(b[12..16].try_into().unwrap());
    let start       = u32::from_le_bytes(b[16..20].try_into().unwrap());
    let end         = u32::from_le_bytes(b[20..24].try_into().unwrap());
    Some((file_offset, csize, dsize, start, end))
}

const MAX_REDIRECTS: u8 = 5;

// ── FST index ─────────────────────────────────────────────────────────────────

struct FstIndex {
    map: Map<Mmap>,
    offsets: Mmap,
}

impl FstIndex {
    fn open(fst_path: &str, off_path: &str) -> Option<Arc<Self>> {
        let fst_mmap = unsafe { Mmap::map(&File::open(fst_path).ok()?).ok()? };
        let map = Map::new(fst_mmap).ok()?;
        let offsets = unsafe { Mmap::map(&File::open(off_path).ok()?).ok()? };
        Some(Arc::new(Self { map, offsets }))
    }

    fn get_record(&self, key: &str) -> Option<&[u8]> {
        let row = self.map.get(key.as_bytes())? as usize;
        let start = row * REC_LEN;
        self.offsets.get(start..start + REC_LEN)
    }
}

// ── file read helper (pread — no cursor, thread-safe) ─────────────────────────

#[cfg(unix)]
fn read_block_at(file: &File, offset: u64, len: usize) -> std::io::Result<Vec<u8>> {
    use std::os::unix::fs::FileExt;
    let mut buf = vec![0u8; len];
    file.read_at(&mut buf, offset)?;
    Ok(buf)
}

#[cfg(not(unix))]
fn read_block_at(file: &File, offset: u64, len: usize) -> std::io::Result<Vec<u8>> {
    use std::io::{Seek, SeekFrom};
    let mut f = file.try_clone()?;
    f.seek(SeekFrom::Start(offset))?;
    let mut buf = vec![0u8; len];
    f.read_exact(&mut buf)?;
    Ok(buf)
}

fn read_entry(file: &File, file_offset: u64, csize: u32, dsize: u32, start: u32, end: u32) -> Option<Vec<u8>> {
    let block = read_block_at(file, file_offset, csize as usize).ok()?;
    let decompressed = decompress_record_block(&block, csize as usize, dsize as usize).ok()?;
    let end = (end as usize).min(decompressed.len());
    let start = (start as usize).min(end);
    Some(decompressed[start..end].to_vec())
}

// ── MdxDictionary ─────────────────────────────────────────────────────────────

pub struct MdxDictionary {
    name: String,
    mdx_path: String,
    mdd_path: Option<String>,
    fst: RwLock<Option<Arc<FstIndex>>>,
    mdd_fst: RwLock<Option<Arc<FstIndex>>>,
    mdx_file: RwLock<Option<Arc<File>>>,
    mdd_file: RwLock<Option<Arc<File>>>,
}

impl MdxDictionary {
    pub fn new(mdx_path: &str) -> Self {
        let name = PathBuf::from(mdx_path)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or(mdx_path)
            .to_string();

        let mdd_path = {
            let p = PathBuf::from(mdx_path).with_extension("mdd");
            if p.exists() { Some(p.to_string_lossy().into_owned()) } else { None }
        };

        MdxDictionary {
            name,
            mdx_path: mdx_path.to_string(),
            mdd_path,
            fst: RwLock::new(None),
            mdd_fst: RwLock::new(None),
            mdx_file: RwLock::new(None),
            mdd_file: RwLock::new(None),
        }
    }

    fn open_fst(&self) -> Option<Arc<FstIndex>> {
        {
            if let Some(idx) = self.fst.read().unwrap().as_ref() {
                return Some(idx.clone());
            }
        }
        let fst_path = format!("{}.fst", self.mdx_path);
        let off_path = format!("{}.offsets", self.mdx_path);
        match FstIndex::open(&fst_path, &off_path) {
            Some(idx) => {
                *self.fst.write().unwrap() = Some(idx.clone());
                Some(idx)
            }
            None => {
                warn!("{}: cannot open FST index {fst_path}", self.name);
                None
            }
        }
    }

    fn open_mdd_fst(&self) -> Option<Arc<FstIndex>> {
        let mdd_path = self.mdd_path.as_ref()?;
        {
            if let Some(idx) = self.mdd_fst.read().unwrap().as_ref() {
                return Some(idx.clone());
            }
        }
        let fst_path = format!("{mdd_path}.fst");
        let off_path = format!("{mdd_path}.offsets");
        match FstIndex::open(&fst_path, &off_path) {
            Some(idx) => {
                *self.mdd_fst.write().unwrap() = Some(idx.clone());
                Some(idx)
            }
            None => {
                warn!("{}: cannot open MDD FST index {fst_path}", self.name);
                None
            }
        }
    }

    fn open_mdx_file(&self) -> Option<Arc<File>> {
        {
            if let Some(f) = self.mdx_file.read().unwrap().as_ref() {
                return Some(f.clone());
            }
        }
        match File::open(&self.mdx_path) {
            Ok(f) => {
                let arc = Arc::new(f);
                *self.mdx_file.write().unwrap() = Some(arc.clone());
                Some(arc)
            }
            Err(e) => {
                warn!("{}: cannot open {}: {e}", self.name, self.mdx_path);
                None
            }
        }
    }

    fn open_mdd_file(&self) -> Option<Arc<File>> {
        let mdd_path = self.mdd_path.as_ref()?;
        {
            if let Some(f) = self.mdd_file.read().unwrap().as_ref() {
                return Some(f.clone());
            }
        }
        match File::open(mdd_path) {
            Ok(f) => {
                let arc = Arc::new(f);
                *self.mdd_file.write().unwrap() = Some(arc.clone());
                Some(arc)
            }
            Err(e) => {
                warn!("{}: cannot open {mdd_path}: {e}", self.name);
                None
            }
        }
    }

    fn invalidate_cache(&self) {
        *self.fst.write().unwrap() = None;
        *self.mdd_fst.write().unwrap() = None;
        *self.mdx_file.write().unwrap() = None;
        *self.mdd_file.write().unwrap() = None;
    }

    fn read_entry_bytes(&self, rec: &[u8]) -> Option<Vec<u8>> {
        let (file_offset, csize, dsize, start, end) = decode_offset(rec)?;
        let file = self.open_mdx_file()?;
        read_entry(&file, file_offset, csize, dsize, start, end)
    }

    fn read_mdd_entry_bytes(&self, rec: &[u8]) -> Option<Vec<u8>> {
        let (file_offset, csize, dsize, start, end) = decode_offset(rec)?;
        let file = self.open_mdd_file()?;
        read_entry(&file, file_offset, csize, dsize, start, end)
    }
}

impl Dictionary for MdxDictionary {
    fn name(&self) -> &str {
        &self.name
    }

    fn lookup(&self, word: &str) -> Option<String> {
        let idx = self.open_fst()?;
        let mut target = word.to_lowercase();
        let mut hops: u8 = 0;

        loop {
            let rec = idx.get_record(&target)?.to_vec();
            let raw = self.read_entry_bytes(&rec)?;
            let text = String::from_utf8_lossy(&raw).into_owned();
            match parse_link(&text) {
                Some(next) if hops < MAX_REDIRECTS => {
                    target = next.to_lowercase();
                    hops += 1;
                }
                Some(_) => return None,
                None => return Some(text),
            }
        }
    }

    fn suggestions(&self, prefix: &str, limit: usize) -> Vec<String> {
        let Some(idx) = self.open_fst() else { return vec![] };
        let p = prefix.to_lowercase();

        let mut results: Vec<String> = Vec::new();
        let mut seen = std::collections::HashSet::new();

        // Prefix search — exact, fast, always first.
        let mut stream = idx.map.search(FstStr::new(&p).starts_with()).into_stream();
        while let Some((key, _)) = stream.next() {
            if let Ok(s) = std::str::from_utf8(key) {
                seen.insert(s.to_string());
                results.push(s.to_string());
            }
            if results.len() >= limit { break; }
        }

        // Fuzzy supplement: fill remaining slots when query is long enough to
        // have meaningful edit distance (avoids noise on 1-2 char inputs).
        if results.len() < limit && p.len() >= 3 {
            let dist = if p.len() < 6 { 1 } else { 2 };
            if let Ok(lev) = Levenshtein::new(&p, dist) {
                let mut fuzz = idx.map.search(&lev).into_stream();
                while let Some((key, _)) = fuzz.next() {
                    if let Ok(s) = std::str::from_utf8(key) {
                        if seen.insert(s.to_string()) {
                            results.push(s.to_string());
                        }
                    }
                    if results.len() >= limit { break; }
                }
            }
        }

        results
    }

    fn resource(&self, path: &str) -> Option<Vec<u8>> {
        let idx = self.open_mdd_fst()?;
        let key = normalize_path(path);
        for cand in candidate_keys(&key) {
            if let Some(rec) = idx.get_record(&cand) {
                let rec = rec.to_vec();
                return self.read_mdd_entry_bytes(&rec);
            }
        }
        None
    }

    fn css_resources(&self) -> Vec<(String, String)> {
        let Some(idx) = self.open_mdd_fst() else { return vec![] };
        let mut stream = idx.map.search(AlwaysMatch).into_stream();
        let mut results = Vec::new();
        while let Some((key, row)) = stream.next() {
            let Ok(name) = std::str::from_utf8(key) else { continue };
            if !name.ends_with(".css") { continue; }
            let start = row as usize * REC_LEN;
            let Some(rec) = idx.offsets.get(start..start + REC_LEN) else { continue };
            let rec = rec.to_vec();
            if let Some(data) = self.read_mdd_entry_bytes(&rec) {
                let body = String::from_utf8_lossy(&data).into_owned();
                results.push((name.to_string(), body));
            }
        }
        results
    }

    fn build_index(&self, force: bool) -> anyhow::Result<()> {
        self.invalidate_cache();
        build_mdx_index(&self.mdx_path, force)?;
        if let Some(mdd_path) = &self.mdd_path {
            build_mdd_index(mdd_path, force)?;
        }
        Ok(())
    }

    fn index_ready(&self) -> bool {
        let mdx_ready = PathBuf::from(format!("{}.fst", self.mdx_path)).exists();
        let mdd_ready = self.mdd_path.as_ref()
            .map(|p| PathBuf::from(format!("{p}.fst")).exists())
            .unwrap_or(true);
        mdx_ready && mdd_ready
    }
}

// ── index builders ────────────────────────────────────────────────────────────

fn build_mdx_index(file: &str, force: bool) -> anyhow::Result<()> {
    let fst_path = format!("{file}.fst");
    let off_path = format!("{file}.offsets");
    if PathBuf::from(&fst_path).exists() {
        if force {
            fs::remove_file(&fst_path)?;
            fs::remove_file(&off_path).ok();
        } else {
            return Ok(());
        }
    }
    info!("indexing MDX {} -> {}", file, fst_path);
    let f = File::open(file)?;
    let mmap = unsafe { memmap2::Mmap::map(&f)? };
    let mdx = Mdx::parse(&mmap);

    // lowercase + sort + dedup (keep first occurrence of each lowercased key)
    let mut entries: Vec<(String, [u8; REC_LEN])> = mdx.entries.iter()
        .map(|e| (e.text.to_lowercase(),
                  encode_offset(e.file_offset, e.block_csize, e.block_dsize, e.start_in_block, e.end_in_block)))
        .collect();
    entries.sort_unstable_by(|a, b| a.0.cmp(&b.0));
    entries.dedup_by(|a, b| a.0 == b.0); // a is later; returning true removes a, keeps b (first)

    let mut off_writer = BufWriter::new(File::create(&off_path)?);
    let mut builder = MapBuilder::new(BufWriter::new(File::create(&fst_path)?))?;
    for (row, (key, rec)) in entries.iter().enumerate() {
        builder.insert(key.as_bytes(), row as u64)?;
        off_writer.write_all(rec)?;
    }
    builder.finish()?;
    info!("MDX indexed {} entries: {}", entries.len(), fst_path);
    Ok(())
}

fn build_mdd_index(file: &str, force: bool) -> anyhow::Result<()> {
    let fst_path = format!("{file}.fst");
    let off_path = format!("{file}.offsets");
    let needs = if PathBuf::from(&fst_path).exists() {
        force || mdd_fst_empty(&fst_path)
    } else {
        true
    };
    if !needs { return Ok(()); }
    if PathBuf::from(&fst_path).exists() {
        fs::remove_file(&fst_path)?;
        fs::remove_file(&off_path).ok();
    }

    info!("indexing MDD {} -> {}", file, fst_path);
    let f = File::open(file)?;
    let mmap = unsafe { memmap2::Mmap::map(&f)? };
    let mdd = Mdd::parse(&mmap);

    // normalize + sort + dedup (keep first occurrence of each key)
    let mut entries: Vec<(String, [u8; REC_LEN])> = mdd.entries.iter()
        .map(|e| (normalize_path(&e.path),
                  encode_offset(e.file_offset, e.block_csize, e.block_dsize, e.start_in_block, e.end_in_block)))
        .collect();
    entries.sort_unstable_by(|a, b| a.0.cmp(&b.0));
    entries.dedup_by(|a, b| a.0 == b.0);

    let mut off_writer = BufWriter::new(File::create(&off_path)?);
    let mut builder = MapBuilder::new(BufWriter::new(File::create(&fst_path)?))?;
    for (row, (key, rec)) in entries.iter().enumerate() {
        builder.insert(key.as_bytes(), row as u64)?;
        off_writer.write_all(rec)?;
    }
    builder.finish()?;
    info!("MDD indexed {} entries: {}", entries.len(), fst_path);
    Ok(())
}

fn mdd_fst_empty(fst_path: &str) -> bool {
    File::open(fst_path).ok()
        .and_then(|f| unsafe { Mmap::map(&f) }.ok())
        .and_then(|m| Map::new(m).ok())
        .map(|m| m.len() == 0)
        .unwrap_or(true)
}

// ── helpers ───────────────────────────────────────────────────────────────────

fn parse_link(def: &str) -> Option<String> {
    let rest = def.strip_prefix("@@@LINK=")?;
    let target: String = rest.chars().take_while(|c| *c != '\r' && *c != '\n' && *c != '\0').collect();
    let trimmed = target.trim();
    if trimmed.is_empty() { None } else { Some(trimmed.to_string()) }
}

fn candidate_keys(key: &str) -> Vec<String> {
    let mut v = vec![key.to_string()];
    if !key.contains('/') {
        v.push(format!("sound/{key}"));
        v.push(format!("audio/{key}"));
        v.push(format!("images/{key}"));
        v.push(format!("img/{key}"));
    }
    v.push(format!("/{key}"));
    v
}
