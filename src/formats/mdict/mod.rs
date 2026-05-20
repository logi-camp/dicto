pub mod parser;

use std::fs::{self, File};
use std::path::PathBuf;
use std::sync::{Arc, RwLock};

use redb::{Database, ReadableTable, ReadableTableMetadata, TableDefinition};
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

// ── redb table definitions ────────────────────────────────────────────────────

const MDX_TABLE: TableDefinition<&str, &[u8]> = TableDefinition::new("mdx");
const MDD_TABLE: TableDefinition<&str, &[u8]> = TableDefinition::new("mdd");

const MAX_REDIRECTS: u8 = 5;

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
    db: RwLock<Option<Arc<Database>>>,
    mdd_db: RwLock<Option<Arc<Database>>>,
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
            db: RwLock::new(None),
            mdd_db: RwLock::new(None),
            mdx_file: RwLock::new(None),
            mdd_file: RwLock::new(None),
        }
    }

    fn open_db(&self) -> Option<Arc<Database>> {
        {
            if let Some(db) = self.db.read().unwrap().as_ref() {
                return Some(db.clone());
            }
        }
        let db_path = format!("{}.redb", self.mdx_path);
        match Database::open(&db_path) {
            Ok(db) => {
                let arc = Arc::new(db);
                *self.db.write().unwrap() = Some(arc.clone());
                Some(arc)
            }
            Err(e) => {
                warn!("{}: cannot open index {db_path}: {e}", self.name);
                None
            }
        }
    }

    fn open_mdd_db(&self) -> Option<Arc<Database>> {
        let mdd_path = self.mdd_path.as_ref()?;
        {
            if let Some(db) = self.mdd_db.read().unwrap().as_ref() {
                return Some(db.clone());
            }
        }
        let db_path = format!("{mdd_path}.redb");
        match Database::open(&db_path) {
            Ok(db) => {
                let arc = Arc::new(db);
                *self.mdd_db.write().unwrap() = Some(arc.clone());
                Some(arc)
            }
            Err(e) => {
                warn!("{}: cannot open MDD index {db_path}: {e}", self.name);
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
        *self.db.write().unwrap() = None;
        *self.mdd_db.write().unwrap() = None;
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
        let db = self.open_db()?;
        let read_txn = db.begin_read().ok()?;
        let table = read_txn.open_table(MDX_TABLE).ok()?;

        let mut target = word.to_string();
        let mut hops: u8 = 0;

        loop {
            let bytes = table.get(target.as_str()).ok()??.value().to_vec();
            let raw = self.read_entry_bytes(&bytes)?;
            let text = String::from_utf8_lossy(&raw).into_owned();
            match parse_link(&text) {
                Some(next) if hops < MAX_REDIRECTS => {
                    target = next;
                    hops += 1;
                }
                Some(_) => return None,
                None => return Some(text),
            }
        }
    }

    fn suggestions(&self, prefix: &str, limit: usize) -> Vec<String> {
        let Some(db) = self.open_db() else { return vec![] };
        let Ok(read_txn) = db.begin_read() else { return vec![] };
        let Ok(table) = read_txn.open_table(MDX_TABLE) else { return vec![] };
        let Ok(range) = table.range(prefix..) else { return vec![] };

        let mut results = Vec::new();
        for entry in range.flatten() {
            let key = entry.0.value();
            if !key.starts_with(prefix) { break; }
            results.push(key.to_string());
            if results.len() >= limit { break; }
        }
        results
    }

    fn resource(&self, path: &str) -> Option<Vec<u8>> {
        let db = self.open_mdd_db()?;
        let read_txn = db.begin_read().ok()?;
        let table = read_txn.open_table(MDD_TABLE).ok()?;

        let key = normalize_path(path);
        for cand in candidate_keys(&key) {
            if let Ok(Some(guard)) = table.get(cand.as_str()) {
                return self.read_mdd_entry_bytes(guard.value());
            }
        }
        None
    }

    fn css_resources(&self) -> Vec<(String, String)> {
        let Some(db) = self.open_mdd_db() else { return vec![] };
        let Ok(read_txn) = db.begin_read() else { return vec![] };
        let Ok(table) = read_txn.open_table(MDD_TABLE) else { return vec![] };
        let Ok(iter) = table.iter() else { return vec![] };

        iter.flatten()
            .filter(|e| e.0.value().ends_with(".css"))
            .filter_map(|e| {
                let name = e.0.value().to_string();
                let data = self.read_mdd_entry_bytes(e.1.value())?;
                let body = String::from_utf8_lossy(&data).into_owned();
                Some((name, body))
            })
            .collect()
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
        let mdx_ready = PathBuf::from(format!("{}.redb", self.mdx_path)).exists();
        let mdd_ready = self.mdd_path.as_ref()
            .map(|p| PathBuf::from(format!("{p}.redb")).exists())
            .unwrap_or(true);
        mdx_ready && mdd_ready
    }
}

// ── index builders ────────────────────────────────────────────────────────────

fn build_mdx_index(file: &str, force: bool) -> anyhow::Result<()> {
    let db_path = format!("{file}.redb");
    if PathBuf::from(&db_path).exists() {
        if force { fs::remove_file(&db_path)?; } else { return Ok(()); }
    }
    info!("indexing MDX {} -> {}", file, db_path);
    let f = File::open(file)?;
    let mmap = unsafe { memmap2::Mmap::map(&f)? };
    let mdx = Mdx::parse(&mmap);

    let db = Database::create(&db_path)?;
    let write_txn = db.begin_write()?;
    {
        let mut table = write_txn.open_table(MDX_TABLE)?;
        for entry in &mdx.entries {
            let rec = encode_offset(
                entry.file_offset,
                entry.block_csize,
                entry.block_dsize,
                entry.start_in_block,
                entry.end_in_block,
            );
            table.insert(entry.text.as_str(), rec.as_slice())?;
        }
    }
    write_txn.commit()?;
    info!("MDX indexed {} entries: {}", mdx.entries.len(), db_path);
    Ok(())
}

fn build_mdd_index(file: &str, force: bool) -> anyhow::Result<()> {
    let db_path = format!("{file}.redb");
    let needs = if PathBuf::from(&db_path).exists() {
        force || mdd_db_empty(&db_path)
    } else {
        true
    };
    if !needs { return Ok(()); }
    if PathBuf::from(&db_path).exists() { fs::remove_file(&db_path)?; }

    info!("indexing MDD {} -> {}", file, db_path);
    let f = File::open(file)?;
    let mmap = unsafe { memmap2::Mmap::map(&f)? };
    let mdd = Mdd::parse(&mmap);

    let db = Database::create(&db_path)?;
    let write_txn = db.begin_write()?;
    {
        let mut table = write_txn.open_table(MDD_TABLE)?;
        for entry in &mdd.entries {
            let key = normalize_path(&entry.path);
            let rec = encode_offset(
                entry.file_offset,
                entry.block_csize,
                entry.block_dsize,
                entry.start_in_block,
                entry.end_in_block,
            );
            table.insert(key.as_str(), rec.as_slice())?;
        }
    }
    write_txn.commit()?;
    info!("MDD indexed {} entries: {}", mdd.entries.len(), db_path);
    Ok(())
}

fn mdd_db_empty(db_path: &str) -> bool {
    let Ok(db) = Database::open(db_path) else { return true };
    let Ok(read_txn) = db.begin_read() else { return true };
    match read_txn.open_table(MDD_TABLE) {
        Err(_) => true,
        Ok(t) => t.len().unwrap_or(0) == 0,
    }
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
