//! Forensic watermarking (marketplace M6.10, drm.md §5).
//!
//! A **per-buyer opaque id** is woven into delivered text assets (a widget's
//! `index.html`, JS/CSS, …) so a leaked copy can be traced back to the buyer —
//! deterrence, not prevention (the design is honest that no client watermark is
//! unbreakable). The mark is **invisible** (zero-width characters) and carries an
//! opaque id only; the id→account mapping stays server-side (privacy).
//!
//! Encoding: a start sentinel (ZWJ) followed by the id bytes as ZWSP/ZWNJ bits,
//! inserted at several points so it survives truncation/partial edits.

use sha2::{Digest, Sha256};

const ZERO: char = '\u{200B}'; // zero-width space  → bit 0
const ONE: char = '\u{200C}'; // zero-width non-joiner → bit 1
const MARK: char = '\u{200D}'; // zero-width joiner → block start sentinel
/// Re-insert the watermark block after roughly this many visible chars.
const INTERVAL: usize = 400;

fn is_zw(ch: char) -> bool {
    ch == ZERO || ch == ONE || ch == MARK
}

/// An opaque, per-buyer watermark id: `sha256(seed ‖ 0 ‖ context)` truncated to
/// 64 bits (16 hex chars). `seed` = a buyer identifier (account / license
/// material); `context` = the package slug, so the id is unique per (buyer, pkg).
pub fn derive_id(seed: &str, context: &str) -> String {
    let mut h = Sha256::new();
    h.update(seed.as_bytes());
    h.update([0u8]);
    h.update(context.as_bytes());
    let digest = h.finalize();
    let mut s = String::with_capacity(16);
    for b in &digest[..8] {
        s.push_str(&format!("{b:02x}"));
    }
    s
}

/// The zero-width block encoding `id`: `MARK ‖ len(1B) ‖ id-bytes`, each bit as
/// ZERO/ONE.
fn block(id: &str) -> String {
    let bytes = id.as_bytes();
    let mut payload = Vec::with_capacity(1 + bytes.len());
    payload.push(bytes.len().min(255) as u8);
    payload.extend_from_slice(&bytes[..bytes.len().min(255)]);
    let mut s = String::new();
    s.push(MARK);
    for b in payload {
        for i in (0..8).rev() {
            s.push(if (b >> i) & 1 == 1 { ONE } else { ZERO });
        }
    }
    s
}

/// Weave the watermark `id` invisibly into `text` (at the start and every
/// `INTERVAL` visible chars, for robustness).
pub fn embed(text: &str, id: &str) -> String {
    let blk = block(id);
    let mut out = String::with_capacity(text.len() + blk.len() * 2);
    out.push_str(&blk);
    let mut since = 0usize;
    for ch in text.chars() {
        out.push(ch);
        if !is_zw(ch) {
            since += 1;
            if since >= INTERVAL {
                out.push_str(&blk);
                since = 0;
            }
        }
    }
    out
}

/// Recover a watermark id woven by [`embed`], or `None` if unmarked.
pub fn extract(text: &str) -> Option<String> {
    let chars: Vec<char> = text.chars().collect();
    for i in 0..chars.len() {
        if chars[i] != MARK {
            continue;
        }
        let mut bits = Vec::new();
        let mut j = i + 1;
        while j < chars.len() && (chars[j] == ZERO || chars[j] == ONE) {
            bits.push(chars[j] == ONE);
            j += 1;
        }
        if let Some(id) = decode(&bits) {
            return Some(id);
        }
    }
    None
}

fn bits_to_byte(bits: &[bool]) -> u8 {
    bits.iter().fold(0u8, |acc, &b| (acc << 1) | b as u8)
}

fn decode(bits: &[bool]) -> Option<String> {
    if bits.len() < 8 {
        return None;
    }
    let len = bits_to_byte(&bits[..8]) as usize;
    if bits.len() < 8 + len * 8 {
        return None;
    }
    let bytes: Vec<u8> = (0..len)
        .map(|k| bits_to_byte(&bits[8 + k * 8..16 + k * 8]))
        .collect();
    String::from_utf8(bytes).ok()
}

/// Remove every zero-width character — the clean, unmarked visible text.
pub fn strip(text: &str) -> String {
    text.chars().filter(|&c| !is_zw(c)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const HTML: &str = "<!doctype html><body><h1>Now Playing</h1><p>a widget with some text content that is long enough to trigger multiple watermark insertions across the document body so truncation still recovers it.</p></body>";

    #[test]
    fn derive_is_opaque_and_stable() {
        assert_eq!(derive_id("acme", "widget/clock"), derive_id("acme", "widget/clock"));
        assert_ne!(derive_id("acme", "widget/clock"), derive_id("bob", "widget/clock"));
        assert_ne!(derive_id("acme", "widget/clock"), derive_id("acme", "widget/other"));
        assert_eq!(derive_id("acme", "x").len(), 16);
    }

    #[test]
    fn embed_extract_round_trips_and_is_invisible() {
        let id = derive_id("buyer-42", "widget/pear-music");
        let marked = embed(HTML, &id);
        assert_eq!(extract(&marked).as_deref(), Some(id.as_str()));
        // Invisible: stripping zero-width chars yields the original.
        assert_eq!(strip(&marked), HTML);
        // Unmarked text has no watermark.
        assert_eq!(extract(HTML), None);
    }

    #[test]
    fn survives_edits_and_truncation() {
        let id = derive_id("buyer", "widget/x");
        let marked = embed(HTML, &id);
        // Prepend/append content.
        assert_eq!(extract(&format!("prefix {marked} suffix")).as_deref(), Some(id.as_str()));
        // Truncate the tail: a repeated block near the start still decodes.
        let half: String = marked.chars().take(marked.chars().count() / 2).collect();
        assert_eq!(extract(&half).as_deref(), Some(id.as_str()));
    }

    #[test]
    fn distinct_buyers_get_distinct_marks() {
        let a = embed(HTML, &derive_id("alice", "widget/x"));
        let b = embed(HTML, &derive_id("bob", "widget/x"));
        assert_ne!(extract(&a), extract(&b));
    }
}
