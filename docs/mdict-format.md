# MDX / MDD parsing, indexing, and query

MDict ships dictionaries as a pair of files:

- **`.mdx`** — the dictionary entries (keys + HTML/text bodies).
- **`.mdd`** — companion resource container (images, sounds, fonts,
  sometimes the dict's own CSS). Optional.

Both share an on-disk layout; the only meaningful difference is that
MDD records are arbitrary bytes keyed by virtual paths instead of
words keyed by strings.

The canonical reference is the
[bitbucket.org/xwang/mdict-analysis](https://bitbucket.org/xwang/mdict-analysis)
project; the SVGs in the repo root (`MDX.svg`, `MDD.svg`) are useful.

## File layout

```
┌─────────────────────────────────────┐
│ Header                              │  parsed in src/mdict/header.rs
│   length-prefixed UTF-16LE XML-ish  │
│   adler32 checksum                  │
├─────────────────────────────────────┤
│ Key block header                    │  src/mdict/keyblock.rs
│   block_num, entry_num, info_len,   │
│   blocks_len  (be_u32 or be_u64)    │
├─────────────────────────────────────┤
│ Key block info                      │  per-block (csize, dsize)
│   encrypted/compressed in V2        │
├─────────────────────────────────────┤
│ Key blocks                          │  zlib-compressed entries
│   each entry: be_u64 offset +       │
│   null-terminated key text          │
├─────────────────────────────────────┤
│ Record block size info              │  src/mdict/recordblock.rs
├─────────────────────────────────────┤
│ Record blocks                       │  zlib- or LZO-compressed
│   each block holds one or more      │
│   record bodies                     │
└─────────────────────────────────────┘
```

## Header parsing

[src/mdict/header.rs](../src/mdict/header.rs)

- Read a `be_u32` length, then that many bytes of UTF-16LE text.
- Adler32-check the buffer against the trailing `le_u32`.
- Parse the XML-ish key="value" attributes with a regex (good enough;
  the format isn't real XML).
- We extract `GeneratedByEngineVersion` (treated as 1 or 2),
  `Encrypted` (`"0"` / `"1"` / `"2"` / `"3"` — bitfield: 1 = encrypt
  record block, 2 = encrypt key info), and `Encoding`.

### MDD header lies about encoding

MDD files often inherit the producing MDX's `Encoding="UTF-8"` value
in the header, but the **keys are always stored as UTF-16LE**.
[src/mdict/mdd.rs](../src/mdict/mdd.rs) overrides `header.encoding` to
`"UTF-16LE"` after parsing so the shared key-block reader interprets
the keys correctly:

```rust
let (data, mut header) = parse_header(data).unwrap();
header.encoding = "UTF-16LE".to_string();
```

## Key blocks

[src/mdict/keyblock.rs](../src/mdict/keyblock.rs)

V2 key block info is wrapped in 8 bytes of metadata + zlib-compressed
body. When `Encrypted == "2" || "3"`, that body is also XOR-encrypted
with a Ripemd128(salt+0x3695) key — see `fast_decrypt` in
[src/util/mod.rs](../src/util/mod.rs).

Two encoding-aware quirks made the parser MDX/MDD-compatible:

1. **Text length is encoded as character count, not byte count.**
   Multiply by 2 for UTF-16 (`char_widths()`), add 1 byte terminator
   for UTF-8 or 2 bytes for UTF-16.

2. **Key terminators differ per encoding.** UTF-8 is null-terminated
   (`\0`); UTF-16 is double-null (`\0\0`). The naïve
   `take_till(|x| x == 0)` works for UTF-8 but mis-stops on the high
   byte of every ASCII char in UTF-16. `parse_block_items_inner`
   walks pairs of bytes when `is_utf16(encoding)` returns true.

## Record blocks & encryption methods

[src/mdict/recordblock.rs](../src/mdict/recordblock.rs)

Each record block starts with a 4-byte `enc` value whose high nibble
is the **encryption method** and low nibble is the **compression
method**:

| Value | Enc | Comp |
|-------|-----|------|
| `0x00` | none | raw |
| `0x01` | none | LZO |
| `0x02` | none | zlib |
| `0x10` | XOR (fast_decrypt) | raw |
| `0x11` | XOR | LZO |
| `0x12` | XOR | zlib |

`fast_decrypt` is the same RC4-flavored cipher used by
mdict-analysis. The 4 bytes immediately after `enc` are an adler32
checksum used as the cipher key seed; LZO needs the expected
decompressed size, which is why blocks carry `(csize, dsize)` in the
size info preamble.

## Robustness: corrupt blocks don't abort the indexer

At least one real-world MDD (the Cambridge 3rd edition shipped with
`Encrypted="2"`) contains one or two record blocks whose zlib stream
panics with `"corrupt deflate stream"`. Indexing 80 000 records and
losing the lot to a single bad block is a poor user experience.

[src/mdict/mdd.rs](../src/mdict/mdd.rs) wraps the per-record decode
in `panic::catch_unwind`, dedup'd by `block_offset_in_buf`. The
caller installs a noop panic hook for the scope of the iterator so
stderr stays clean; any block we have to skip emits one warning, not
hundreds:

```text
WARN mdd: failed to decode block at 0xfa13321 (csize=57117), skipping;
     first record in block: '\z_usb01242.spx'
```

## Indexing to redb

[src/formats/mdict/mod.rs](../src/formats/mdict/mod.rs)

Two parallel functions on `MdxDictionary`:

- `build_index(reindex)` builds `<file>.mdx.redb` with redb table
  `"mdx"` keyed by `&str` (headword), value `&[u8]` (definition bytes).
- `build_index_mdd(reindex)` builds `<file>.mdd.redb` with redb table
  `"mdd"` keyed by `&str` (normalized path), value `&[u8]` (resource bytes).

### Heal-empty-DB

A previous version of MDD parsing would panic on corrupt blocks and
leave behind a 0-byte shell. The current `build_index_mdd` checks
whether the `.redb` is missing or has zero rows before deciding to
skip — if empty, it gets re-indexed. The MDX side intentionally has
no equivalent because re-indexing large MDX files is expensive.

### Path normalization

MDD keys arrive as Windows paths like `\sound\z_okapi.spx`. They're
stored normalized — lowercase, `\` → `/`, trim leading `/` — so the
HTML side can look them up case-insensitively without worrying about
backslashes. See [src/mdict/mdd.rs::normalize_path](../src/mdict/mdd.rs).

Path normalization also strips MDX URL schemes that show up in HTML
`src` / `href` values: `file://`, `bword://`, `sound://`, `entry://`.
A reference to `<img src="file://x_okra.jpg">` becomes a lookup for
`x_okra.jpg` — same as the on-disk key.

## Query layer

[src/query/mod.rs](../src/query/mod.rs)

### `query_all(word) -> Vec<DictHit>`

Walks every dictionary enabled by [settings](settings.md) in
configured order. For each, prepares a single statement and looks up
the word.

Records that start with `@@@LINK=<target>` are **redirects** to
another entry in the same dictionary (`OK` → `okay`). The query
follows redirects up to `MAX_REDIRECTS = 5` hops; redirects don't
cross dictionaries.

### `search_suggestions(prefix, limit)`

Opens a range scan on the `"mdx"` redb table starting at `prefix`
and collects entries while they share the prefix. Results are
deduplicated across dictionaries via a `HashSet` so the word list
doesn't show the same word three times when it exists in three dicts.

### `lookup_resource(path)`

Tries the normalized path in each enabled MDD, falling through a
small list of fallbacks (`sound/x`, `audio/x`, `images/x`, `img/x`)
because different dictionaries shelve resources in different
sub-folders. Returns the first matching blob.

### `css_resources_in_mdd(mdd_file)`

Lighter-weight specialized lookup: pulls every `.css` resource out of
one specific MDD's index. Used at startup to populate the per-dict
stylesheet (some dictionaries — notably LDOCE5 — bundle their
`ldoceaz.css` inside the MDD instead of shipping it as a separate
file). See [rendering.md](rendering.md) for how the bytes get used.

## Index handle lifecycle

[src/formats/mdict/mod.rs](../src/formats/mdict/mod.rs)

Each `MdxDictionary` owns two `RwLock<Option<Arc<FstIndex>>>` fields —
one for the MDX FST, one for the MDD FST.

`FstIndex` wraps:
- `map: Map<Mmap>` — the FST map (lowercased key → row index), memory-mapped.
- `offsets: Mmap` — the flat array of 24-byte offset records, memory-mapped.

`open_fst()` / `open_mdd_fst()` return the cached `Arc<FstIndex>` or
memory-map the files on first call. This matters when the settings
dialog enables a previously-disabled dictionary that has just been
re-indexed — no restart needed.

`reset_pools()` is now a **no-op**. Cache invalidation happens naturally
when `registry::reload()` creates fresh `MdxDictionary` instances that
open new memory maps on their first query.

Memory-mapped files are read-only; multiple threads can share the same
`Arc<FstIndex>` safely.

## Gotchas

- The header field `Encoding` is sometimes missing. We default to
  empty string and the encoding-aware code paths treat that as
  "single-byte" (so it works for UTF-8 / Latin-1 MDX dictionaries).
- The `Encrypted` flag is a bitfield ("0", "1", "2", "3"), not a
  level. Bit 0 = record-block encryption, bit 1 = key-info
  encryption. Cambridge MDX ships with `"2"` (key info only).
- LZO decompression needs the expected output size — never pass a
  wrong `dsize`. The size info preamble's `dsize` is authoritative.
- `Mdx::new` and `Mdd::new` panic on truly malformed files. They're
  called once per startup so this is acceptable, but if you wrap
  them in a long-running loop, add `catch_unwind`.
- The on-disk `.mdx.redb` retains rows from before a redirect target
  was renamed; `build_index` overwrites existing keys on subsequent
  builds, so the table self-heals.
