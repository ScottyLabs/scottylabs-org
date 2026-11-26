//! Searching published projects for the Projects page (spec P-5). It mirrors `matchesSearch` in sites/web/src/lib/projects/filter.ts, which filters the cards the page has loaded: every word of the search must appear, ignoring case and accents, in the name, summary, description, committee, kind or status. Cards carry only the start of the description, so the page also asks the service, which looks through the whole of it.

use std::collections::HashMap;
use std::hash::{BuildHasher, RandomState};
use std::sync::{Arc, LazyLock, Mutex, PoisonError};

use unicode_normalization::UnicodeNormalization;
use uuid::Uuid;

use super::doc::ProjectDoc;
use crate::richtext;

/// Projects whose text is kept at most. Past this the cache starts over; a site has far fewer published projects.
const MAX_KEPT: usize = 4096;

/// What reading a published project's description gives: the excerpt on its card and the folded text a search looks through. Both parse the description's HTML, so they are kept per project in memory and worked out again only when the text changes; neither a list nor a search parses anything for a project already read.
pub struct ReadText {
    pub excerpt: String,
    /// [`searchable_text`], folded.
    pub folded: String,
}

struct Kept {
    /// Of every field the text comes from, so an edit to any of them is noticed.
    fingerprint: u64,
    text: Arc<ReadText>,
}

struct TextCache {
    /// Keyed per process, so nobody can pick texts that collide.
    hasher: RandomState,
    kept: Mutex<HashMap<Uuid, Kept>>,
}

static TEXTS: LazyLock<TextCache> = LazyLock::new(|| TextCache {
    hasher: RandomState::new(),
    kept: Mutex::new(HashMap::new()),
});

/// The excerpt and folded text of project `id` in the version `doc`. CPU bound the first time a version is read, so callers run it off the async runtime.
pub fn read(id: Uuid, doc: &ProjectDoc) -> Arc<ReadText> {
    let cache = &*TEXTS;
    let fingerprint = cache.hasher.hash_one((
        doc.name.as_str(),
        doc.summary.as_str(),
        doc.description.as_str(),
        doc.committee.name(),
        doc.kind.label(),
        doc.status.label(),
    ));
    {
        let kept = cache.kept.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(entry) = kept
            .get(&id)
            .filter(|entry| entry.fingerprint == fingerprint)
        {
            return Arc::clone(&entry.text);
        }
    }
    let text = Arc::new(ReadText {
        excerpt: doc.excerpt(),
        folded: fold(&searchable_text(doc)),
    });
    let mut kept = cache.kept.lock().unwrap_or_else(PoisonError::into_inner);
    if kept.len() >= MAX_KEPT && !kept.contains_key(&id) {
        kept.clear();
    }
    kept.insert(
        id,
        Kept {
            fingerprint,
            text: Arc::clone(&text),
        },
    );
    text
}

/// The longest search read; the rest is ignored. The Projects page keeps at most 80 characters.
pub const MAX_QUERY_CHARS: usize = 100;

/// A search text cut to [`MAX_QUERY_CHARS`].
pub fn capped(q: &str) -> String {
    q.chars().take(MAX_QUERY_CHARS).collect()
}

/// Everything a search looks through, joined as `searchableText` joins it, with the whole description as plain text where the page has only the excerpt. Parses the description, so callers run it off the async runtime.
pub fn searchable_text(doc: &ProjectDoc) -> String {
    [
        doc.name.as_str(),
        doc.summary.as_str(),
        &richtext::plain_text(&doc.description),
        doc.committee.name(),
        doc.kind.label(),
        doc.status.label(),
    ]
    .join(" ")
}

/// Which of `texts` (from [`searchable_text`]) hold every word of `q`, in order. Everything matches a search without words.
#[cfg(test)]
pub fn matches(q: &str, texts: &[String]) -> Vec<bool> {
    let folded: Vec<String> = texts.iter().map(|text| fold(text)).collect();
    matches_folded(q, folded.iter().map(String::as_str))
}

/// [`matches`] for texts already folded, such as [`ReadText::folded`]. Folding is per character, and a word never holds a space, so folding the joined fields equals joining the folded fields.
pub fn matches_folded<'a>(q: &str, folded: impl IntoIterator<Item = &'a str>) -> Vec<bool> {
    let q = fold(q);
    let words: Vec<&str> = q.split(is_space).filter(|word| !word.is_empty()).collect();
    folded
        .into_iter()
        .map(|text| words.iter().all(|word| text.contains(word)))
        .collect()
}

/// `fold` in filter.ts: compatibility decomposition (NFKD), then diacritics removed, then lowercase, so "cafe" finds "Caf\u{e9}". Done here rather than with Postgres's `normalize`, which works only in a UTF-8 database.
fn fold(text: &str) -> String {
    text.nfkd()
        .filter(|c| !is_diacritic(*c))
        .collect::<String>()
        .to_lowercase()
}

/// What `\s` matches in JavaScript, which splits the page's search into words.
fn is_space(c: char) -> bool {
    matches!(
        c,
        '\t' | '\n' | '\u{b}' | '\u{c}' | '\r' | ' ' | '\u{a0}' | '\u{1680}' | '\u{2000}'
            ..='\u{200a}'
                | '\u{2028}'
                | '\u{2029}'
                | '\u{202f}'
                | '\u{205f}'
                | '\u{3000}'
                | '\u{feff}'
    )
}

/// Whether a character has the Unicode `Diacritic` property, which `fold` removes with `/\p{Diacritic}/u`.
fn is_diacritic(c: char) -> bool {
    DIACRITIC_BOUNDS.partition_point(|bound| *bound <= u32::from(c)) % 2 == 1
}

/// The code points where the `Diacritic` property starts or stops, in order: the first of each run of diacritics, then the first code point after it. A code point is a diacritic when an odd number of bounds are at or below it. Listed from what `/\p{Diacritic}/u` matches in Deno 2.9, so the service folds text as the page does; regenerate it the same way if a newer Unicode version changes the property.
const DIACRITIC_BOUNDS: [u32; 428] = [
    0x005E, 0x005F, 0x0060, 0x0061, 0x00A8, 0x00A9, 0x00AF, 0x00B0, 0x00B4, 0x00B5, 0x00B7, 0x00B9,
    0x02B0, 0x034F, 0x0350, 0x0358, 0x035D, 0x0363, 0x0374, 0x0376, 0x037A, 0x037B, 0x0384, 0x0386,
    0x0483, 0x0488, 0x0559, 0x055A, 0x0591, 0x05A2, 0x05A3, 0x05BE, 0x05BF, 0x05C0, 0x05C1, 0x05C3,
    0x05C4, 0x05C5, 0x064B, 0x0653, 0x0657, 0x0659, 0x06DF, 0x06E1, 0x06E5, 0x06E7, 0x06EA, 0x06ED,
    0x0730, 0x074B, 0x07A6, 0x07B1, 0x07EB, 0x07F6, 0x0818, 0x081A, 0x0898, 0x08A0, 0x08C9, 0x08D3,
    0x08E3, 0x08FF, 0x093C, 0x093D, 0x094D, 0x094E, 0x0951, 0x0955, 0x0971, 0x0972, 0x09BC, 0x09BD,
    0x09CD, 0x09CE, 0x0A3C, 0x0A3D, 0x0A4D, 0x0A4E, 0x0ABC, 0x0ABD, 0x0ACD, 0x0ACE, 0x0AFD, 0x0B00,
    0x0B3C, 0x0B3D, 0x0B4D, 0x0B4E, 0x0B55, 0x0B56, 0x0BCD, 0x0BCE, 0x0C3C, 0x0C3D, 0x0C4D, 0x0C4E,
    0x0CBC, 0x0CBD, 0x0CCD, 0x0CCE, 0x0D3B, 0x0D3D, 0x0D4D, 0x0D4E, 0x0DCA, 0x0DCB, 0x0E3A, 0x0E3B,
    0x0E47, 0x0E4D, 0x0E4E, 0x0E4F, 0x0EBA, 0x0EBB, 0x0EC8, 0x0ECD, 0x0F18, 0x0F1A, 0x0F35, 0x0F36,
    0x0F37, 0x0F38, 0x0F39, 0x0F3A, 0x0F3E, 0x0F40, 0x0F82, 0x0F85, 0x0F86, 0x0F88, 0x0FC6, 0x0FC7,
    0x1037, 0x1038, 0x1039, 0x103B, 0x1063, 0x1065, 0x1069, 0x106E, 0x1087, 0x108E, 0x108F, 0x1090,
    0x109A, 0x109C, 0x135D, 0x1360, 0x1714, 0x1716, 0x1734, 0x1735, 0x17C9, 0x17D4, 0x17DD, 0x17DE,
    0x1939, 0x193C, 0x1A60, 0x1A61, 0x1A75, 0x1A7D, 0x1A7F, 0x1A80, 0x1AB0, 0x1ABF, 0x1AC1, 0x1ACC,
    0x1B34, 0x1B35, 0x1B44, 0x1B45, 0x1B6B, 0x1B74, 0x1BAA, 0x1BAC, 0x1BE6, 0x1BE7, 0x1BF2, 0x1BF4,
    0x1C36, 0x1C38, 0x1C78, 0x1C7E, 0x1CD0, 0x1CE9, 0x1CED, 0x1CEE, 0x1CF4, 0x1CF5, 0x1CF7, 0x1CFA,
    0x1D2C, 0x1D6B, 0x1DC4, 0x1DD0, 0x1DF5, 0x1E00, 0x1FBD, 0x1FBE, 0x1FBF, 0x1FC2, 0x1FCD, 0x1FD0,
    0x1FDD, 0x1FE0, 0x1FED, 0x1FF0, 0x1FFD, 0x1FFF, 0x2CEF, 0x2CF2, 0x2E2F, 0x2E30, 0x302A, 0x3030,
    0x3099, 0x309D, 0x30FC, 0x30FD, 0xA66F, 0xA670, 0xA67C, 0xA67E, 0xA67F, 0xA680, 0xA69C, 0xA69E,
    0xA6F0, 0xA6F2, 0xA700, 0xA722, 0xA788, 0xA78B, 0xA7F8, 0xA7FA, 0xA806, 0xA807, 0xA82C, 0xA82D,
    0xA8C4, 0xA8C5, 0xA8E0, 0xA8F2, 0xA92B, 0xA92F, 0xA953, 0xA954, 0xA9B3, 0xA9B4, 0xA9C0, 0xA9C1,
    0xA9E5, 0xA9E6, 0xAA7B, 0xAA7E, 0xAABF, 0xAAC3, 0xAAF6, 0xAAF7, 0xAB5B, 0xAB60, 0xAB69, 0xAB6C,
    0xABEC, 0xABEE, 0xFB1E, 0xFB1F, 0xFE20, 0xFE30, 0xFF3E, 0xFF3F, 0xFF40, 0xFF41, 0xFF70, 0xFF71,
    0xFF9E, 0xFFA0, 0xFFE3, 0xFFE4, 0x102E0, 0x102E1, 0x10780, 0x10786, 0x10787, 0x107B1, 0x107B2,
    0x107BB, 0x10A38, 0x10A3B, 0x10A3F, 0x10A40, 0x10AE5, 0x10AE7, 0x10D22, 0x10D28, 0x10D4E,
    0x10D4F, 0x10D69, 0x10D6E, 0x10EFD, 0x10F00, 0x10F46, 0x10F51, 0x10F82, 0x10F86, 0x11046,
    0x11047, 0x11070, 0x11071, 0x110B9, 0x110BB, 0x11133, 0x11135, 0x11173, 0x11174, 0x111C0,
    0x111C1, 0x111CA, 0x111CD, 0x11235, 0x11237, 0x112E9, 0x112EB, 0x1133B, 0x1133D, 0x1134D,
    0x1134E, 0x11366, 0x1136D, 0x11370, 0x11375, 0x113CE, 0x113D1, 0x113D2, 0x113D4, 0x113E1,
    0x113E3, 0x11442, 0x11443, 0x11446, 0x11447, 0x114C2, 0x114C4, 0x115BF, 0x115C1, 0x1163F,
    0x11640, 0x116B6, 0x116B8, 0x1172B, 0x1172C, 0x11839, 0x1183B, 0x1193D, 0x1193F, 0x11943,
    0x11944, 0x119E0, 0x119E1, 0x11A34, 0x11A35, 0x11A47, 0x11A48, 0x11A99, 0x11A9A, 0x11C3F,
    0x11C40, 0x11D42, 0x11D43, 0x11D44, 0x11D46, 0x11D97, 0x11D98, 0x11F41, 0x11F43, 0x11F5A,
    0x11F5B, 0x13447, 0x13456, 0x1612F, 0x16130, 0x16AF0, 0x16AF5, 0x16B30, 0x16B37, 0x16D6B,
    0x16D6D, 0x16F8F, 0x16FA0, 0x16FF0, 0x16FF2, 0x1AFF0, 0x1AFF4, 0x1AFF5, 0x1AFFC, 0x1AFFD,
    0x1AFFF, 0x1CF00, 0x1CF2E, 0x1CF30, 0x1CF47, 0x1D167, 0x1D16A, 0x1D16D, 0x1D173, 0x1D17B,
    0x1D183, 0x1D185, 0x1D18C, 0x1D1AA, 0x1D1AE, 0x1E030, 0x1E06E, 0x1E130, 0x1E137, 0x1E2AE,
    0x1E2AF, 0x1E2EC, 0x1E2F0, 0x1E5EE, 0x1E5F0, 0x1E8D0, 0x1E8D7, 0x1E944, 0x1E947, 0x1E948,
    0x1E94B,
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn diacritics_are_found_like_the_page_finds_them() {
        for diacritic in [
            '^', '`', '\u{a8}', '\u{b4}', '\u{b7}', '\u{301}', '\u{308}', '\u{3099}',
        ] {
            assert!(is_diacritic(diacritic), "{diacritic:?}");
        }
        for letter in ['a', 'Z', '_', '\u{e9}', '\u{f8}', '\u{3042}', '\u{1f600}'] {
            assert!(!is_diacritic(letter), "{letter:?}");
        }
        assert!(DIACRITIC_BOUNDS.windows(2).all(|pair| pair[0] < pair[1]));
    }

    #[test]
    fn text_loses_its_accents_and_case() {
        assert_eq!(fold("Caf\u{e9} \u{c5}NGSTR\u{d6}M"), "cafe angstrom");
        // Already decomposed text folds the same way.
        assert_eq!(fold("Cafe\u{301} A\u{30a}NGSTRO\u{308}M"), "cafe angstrom");
        assert_eq!(fold("\u{3a3}\u{39f}\u{3a3}"), "\u{3c3}\u{3bf}\u{3c2}");
        // Compatibility forms, as NFKD in the page: the fi ligature and full-width letters.
        assert_eq!(fold("\u{fb01}le \u{ff21}\u{ff22}"), "file ab");
    }

    #[test]
    fn every_word_must_match_somewhere() {
        let texts = [
            "CMU Maps Room-level directions across campus".to_owned(),
            "Caf\u{e9} finder Tech".to_owned(),
        ];
        assert_eq!(matches("maps campus", &texts), [true, false]);
        assert_eq!(matches("CAFE tech", &texts), [false, true]);
        assert_eq!(matches("maps tech", &texts), [false, false]);
        assert_eq!(matches(" \t ", &texts), [true, true]);
        assert!(matches("maps", &[]).is_empty());
    }

    #[test]
    fn read_text_is_kept_until_the_project_changes() {
        use crate::models::Committee;

        let id = Uuid::now_v7();
        let mut doc = ProjectDoc::new("Caf\u{e9} finder".into(), Committee::Tech);
        doc.description = "<p>Find <strong>coffee</strong> near you.</p>".into();
        let first = read(id, &doc);
        assert_eq!(first.excerpt, "Find coffee near you.");
        assert!(first.folded.starts_with("cafe finder"));
        assert!(
            Arc::ptr_eq(&first, &read(id, &doc)),
            "read once per version"
        );
        assert_eq!(
            matches_folded("COFFEE cafe", [first.folded.as_str()]),
            [true]
        );

        doc.description = "<p>Find tea.</p>".into();
        let edited = read(id, &doc);
        assert!(!Arc::ptr_eq(&first, &edited));
        assert_eq!(edited.excerpt, "Find tea.");
        assert_eq!(matches_folded("coffee", [edited.folded.as_str()]), [false]);
        doc.summary = "Hot drinks".into();
        assert!(read(id, &doc).folded.contains("hot drinks"));
    }

    #[test]
    fn words_are_split_on_javascript_white_space() {
        let words: Vec<&str> = "a\u{feff}b\u{3000}c\td"
            .split(is_space)
            .filter(|word| !word.is_empty())
            .collect();
        assert_eq!(words, ["a", "b", "c", "d"]);
        assert_eq!(capped(&"x".repeat(150)).len(), MAX_QUERY_CHARS);
    }
}
