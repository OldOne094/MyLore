//! Title normalization and matching (MISSION-025, DOMAIN_MODEL §2.4 / identity).
//!
//! Produces a canonical fold of a title so that variants that are "the same
//! title" compare equal, feeding MISSION-026 IdentityService exact + fuzzy
//! matching. Pure and side-effect free.
//!
//! Fold rules (in order):
//!   1. Unicode NFC composition.
//!   2. Case fold (full Unicode lowercase).
//!   3. Width normalization: fullwidth ASCII → halfwidth, fullwidth space →
//!      ASCII space, halfwidth katakana → fullwidth (script-aware).
//!   4. Decompose (NFD) and drop combining marks (diacritics, Arabic harakat),
//!      except Japanese voicing marks U+3099/U+309A (パン stays distinct from ハン).
//!   5. Arabic consonant variants: أ/إ/آ/ٱ → ا, ى → ي, ة → ه (mirrors the
//!      FTS fold in `infrastructure::fts`, generalized to the domain layer).
//!   6. Script-aware whitespace/punctuation: scripts without spaces (Han,
//!      kana, hangul) drop whitespace and punctuation entirely; space-separated
//!      scripts collapse whitespace runs to one space and drop punctuation.

use unicode_normalization::char::is_combining_mark;
use unicode_normalization::UnicodeNormalization;

/// Canonical folded form of a title, used for exact matching and as the input
/// to fuzzy scoring. Never shown to the user — display keeps the original.
pub fn fold_title(text: &str) -> String {
    // 1. Compose (é, が …) so precomposed and decomposed spellings match.
    let nfc: String = text.nfc().collect();
    let mut decomposed: String = String::with_capacity(text.len());

    // 2. Case fold, 3. width normalize, 4. NFD + drop combining marks
    //    (diacritics, Arabic harakat, hamza), keeping kana voicing marks.
    for ch in nfc.chars() {
        for lower in ch.to_lowercase() {
            let width_normalized = normalize_width(lower);
            for c in width_normalized.nfd() {
                if is_combining_mark(c) && !is_kana_voicing(c) {
                    continue;
                }
                decomposed.push(c);
            }
        }
    }

    // 5. Arabic consonant variants (after decomposition the base letters remain)
    //    and the cross-separator (HUNTER×HUNTER vs HUNTERxHUNTER).
    let mapped: String = decomposed
        .chars()
        .map(|ch| match ch {
            'أ' | 'إ' | 'آ' | 'ٱ' => 'ا',
            'ى' => 'ي',
            'ة' => 'ه',
            '×' => 'x',
            ch => ch,
        })
        .collect();

    // 6. Script-aware whitespace / punctuation filtering. Kana voicing marks
    //    are kept as combining marks (they attach to the preceding letter).
    let keep = |c: char| c.is_alphanumeric() || is_kana_voicing(c);
    if is_spaceless_script(&mapped) {
        mapped.chars().filter(|&c| keep(c)).collect()
    } else {
        let mut out = String::with_capacity(mapped.len());
        let mut pending_space = false;
        for ch in mapped.chars() {
            if keep(ch) {
                if pending_space && !out.is_empty() && !is_kana_voicing(ch) {
                    out.push(' ');
                }
                pending_space = false;
                out.push(ch);
            } else if is_word_separator(ch) {
                pending_space = true;
            }
        }
        out
    }
}

/// Characters that split words in space-separated scripts (treated like
/// whitespace): hyphens, underscores, slashes, dashes and the middle dot.
/// Dropped entirely in spaceless (CJK) scripts.
fn is_word_separator(ch: char) -> bool {
    ch.is_whitespace() || matches!(ch, '-' | '_' | '/' | '·' | '–' | '—')
}

/// Japanese combining voicing marks (dakuten / handakuten). These are
/// diacritics semantically but change meaning (パン vs ハン), so the fold keeps
/// them instead of stripping them like Latin accents.
fn is_kana_voicing(c: char) -> bool {
    matches!(c, '\u{3099}' | '\u{309A}')
}

/// Whether two titles are the same after folding.
pub fn title_matches(left: &str, right: &str) -> bool {
    fold_title(left) == fold_title(right)
}

/// Whether `haystack` contains `needle` after folding (substring on the folded
/// keys). Useful for the fuzzy stage of identity matching.
pub fn title_contains(haystack: &str, needle: &str) -> bool {
    let hay = fold_title(haystack);
    let needle = fold_title(needle);
    !needle.is_empty() && hay.contains(&needle)
}

/// Width-normalize one character: fullwidth ASCII → ASCII, fullwidth space →
/// space, halfwidth katakana → fullwidth (with voicing marks as combining).
fn normalize_width(ch: char) -> char {
    match ch {
        '\u{FF01}'..='\u{FF5E}' => char::from_u32(u32::from(ch) - 0xFEE0).unwrap_or(ch),
        '\u{3000}' => ' ',
        '\u{FF61}'..='\u{FF9F}' => halfwidth_kana(ch),
        ch => ch,
    }
}

/// Halfwidth katakana / halfwidth punctuation / voicing marks → fullwidth.
fn halfwidth_kana(ch: char) -> char {
    let table: &[(char, char)] = &[
        ('\u{FF61}', '\u{3002}'),
        ('\u{FF62}', '\u{300C}'),
        ('\u{FF63}', '\u{300D}'),
        ('\u{FF64}', '\u{3001}'),
        ('\u{FF65}', '\u{30FB}'),
        ('\u{FF66}', '\u{30F2}'),
        ('\u{FF67}', '\u{30A1}'),
        ('\u{FF68}', '\u{30A3}'),
        ('\u{FF69}', '\u{30A5}'),
        ('\u{FF6A}', '\u{30A7}'),
        ('\u{FF6B}', '\u{30A9}'),
        ('\u{FF6C}', '\u{30E3}'),
        ('\u{FF6D}', '\u{30E5}'),
        ('\u{FF6E}', '\u{30E7}'),
        ('\u{FF6F}', '\u{30C3}'),
        ('\u{FF70}', '\u{30FC}'),
        ('\u{FF71}', '\u{30A2}'),
        ('\u{FF72}', '\u{30A4}'),
        ('\u{FF73}', '\u{30A6}'),
        ('\u{FF74}', '\u{30A8}'),
        ('\u{FF75}', '\u{30AA}'),
        ('\u{FF76}', '\u{30AB}'),
        ('\u{FF77}', '\u{30AD}'),
        ('\u{FF78}', '\u{30AF}'),
        ('\u{FF79}', '\u{30B1}'),
        ('\u{FF7A}', '\u{30B3}'),
        ('\u{FF7B}', '\u{30B5}'),
        ('\u{FF7C}', '\u{30B7}'),
        ('\u{FF7D}', '\u{30B9}'),
        ('\u{FF7E}', '\u{30BB}'),
        ('\u{FF7F}', '\u{30BD}'),
        ('\u{FF80}', '\u{30BF}'),
        ('\u{FF81}', '\u{30C1}'),
        ('\u{FF82}', '\u{30C3}'),
        ('\u{FF83}', '\u{30C4}'),
        ('\u{FF84}', '\u{30C6}'),
        ('\u{FF85}', '\u{30CA}'),
        ('\u{FF86}', '\u{30CB}'),
        ('\u{FF87}', '\u{30CC}'),
        ('\u{FF88}', '\u{30CD}'),
        ('\u{FF89}', '\u{30CE}'),
        ('\u{FF8A}', '\u{30CF}'),
        ('\u{FF8B}', '\u{30D2}'),
        ('\u{FF8C}', '\u{30D5}'),
        ('\u{FF8D}', '\u{30D8}'),
        ('\u{FF8E}', '\u{30DB}'),
        ('\u{FF8F}', '\u{30DE}'),
        ('\u{FF90}', '\u{30DF}'),
        ('\u{FF91}', '\u{30E0}'),
        ('\u{FF92}', '\u{30E1}'),
        ('\u{FF93}', '\u{30E2}'),
        ('\u{FF94}', '\u{30E4}'),
        ('\u{FF95}', '\u{30E6}'),
        ('\u{FF96}', '\u{30E8}'),
        ('\u{FF97}', '\u{30E9}'),
        ('\u{FF98}', '\u{30EA}'),
        ('\u{FF99}', '\u{30EB}'),
        ('\u{FF9A}', '\u{30EC}'),
        ('\u{FF9B}', '\u{30ED}'),
        ('\u{FF9C}', '\u{30EF}'),
        ('\u{FF9D}', '\u{30F3}'),
        ('\u{FF9E}', '\u{3099}'),
        ('\u{FF9F}', '\u{309A}'),
    ];
    table
        .iter()
        .find_map(|(from, to)| (*from == ch).then_some(*to))
        .unwrap_or(ch)
}

/// Scripts that do not use spaces between words (Han, kana, hangul). Used to
/// decide whitespace/punctuation handling for the whole string.
fn is_spaceless_script(text: &str) -> bool {
    text.chars().any(|c| {
        matches!(
            c,
            '\u{3040}'..='\u{30FF}'      // hiragana + katakana
            | '\u{31F0}'..='\u{31FF}'    // katakana phonetic extensions
            | '\u{4E00}'..='\u{9FFF}'    // CJK unified ideographs
            | '\u{3400}'..='\u{4DBF}'    // CJK ext A
            | '\u{F900}'..='\u{FAFF}'    // CJK compatibility ideographs
            | '\u{1100}'..='\u{11FF}'    // hangul jamo
            | '\u{AC00}'..='\u{D7A3}'    // hangul syllables
            | '\u{3005}'..='\u{3007}'    // iteration marks / ideographic number
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn folds_case_and_latin_diacritics() {
        assert_eq!(fold_title("Café"), fold_title("cafe"));
        assert_eq!(fold_title("Pokémon"), fold_title("POKEMON"));
        assert_eq!(fold_title("Škola"), fold_title("skola"));
        assert_eq!(fold_title("naïve"), "naive");
    }

    #[test]
    fn nfc_makes_decomposed_equal_precomposed() {
        // "é" as e + U+0301 (decomposed) must equal the precomposed form.
        let decomposed = "cafe\u{301}";
        assert!(title_matches(decomposed, "café"));
        assert_eq!(fold_title(decomposed), fold_title("café"));
    }

    #[test]
    fn cyrillic_lowercases() {
        assert!(title_matches("ФУЛЛ АЛКОГОЛИК", "фулл алкоголик"));
    }

    #[test]
    fn collapses_whitespace_and_drops_punctuation() {
        assert_eq!(
            fold_title("  Sword   of  the  Dawn!  "),
            "sword of the dawn"
        );
        assert!(title_matches("Hunter × Hunter", "HUNTER X HUNTER"));
        assert!(title_matches(
            "Sword of the Dawn: Part 2",
            "Sword of the Dawn Part 2"
        ));
        assert_eq!(fold_title("One-Punch Man"), "one punch man");
    }

    #[test]
    fn fullwidth_ascii_and_space_are_halfwidth() {
        assert_eq!(fold_title("ＳＮＫ　ＳＰＥＣＩＡＬ"), "snk special");
        assert!(title_matches("ＦＵＬＬ　ＷＩＤＴＨ", "full width"));
    }

    #[test]
    fn cjk_drops_spaces_and_punctuation() {
        assert!(title_matches("魔法使いの夜", "魔法 使い の 夜"));
        assert!(title_matches("鬼滅の刃 Season 2", "鬼滅の刃Season 2"));
        assert_eq!(fold_title("進撃の巨人！"), "進撃の巨人");
    }

    #[test]
    fn halfwidth_kana_maps_to_fullwidth() {
        assert!(title_matches("ﾊﾝﾀｰ×ﾊﾝﾀｰ", "ハンター×ハンター"));
        assert!(title_matches("ｶﾞﾝﾀﾞﾑ", "ガンダム"));
        assert_eq!(fold_title("ｼﾞｮｼﾞｮ"), fold_title("ジョジョ"));
    }

    #[test]
    fn kana_voicing_is_preserved() {
        assert!(
            !title_matches("パン", "ハン"),
            "voiced vs unvoiced must differ"
        );
        assert!(!title_matches("ば", "は"));
        assert!(title_matches("パン", "パン"));
    }

    #[test]
    fn arabic_diacritics_and_variants_fold() {
        assert!(title_matches("عَبْقَرِيَّةٌ", "عبقريه"));
        assert!(title_matches("أحمد", "احمد"));
        assert!(title_matches("فتى", "فتي"), "alef-maqsura folds to ya");
    }

    #[test]
    fn folded_key_is_stable_and_deterministic() {
        let a = fold_title("One  Punch Màn: The ★ Collection !");
        let b = fold_title("ONE PUNCH MAN the collection");
        assert_eq!(a, b);
    }

    #[test]
    fn contains_works_on_folded_keys() {
        assert!(title_contains("Sword of the Dawn", "SWORD"));
        assert!(title_contains("Sword of the Dawn", "sword of"));
        assert!(!title_contains("Sword of the Dawn", "spear"));
        assert!(!title_contains("anything", ""));
    }

    #[test]
    fn empty_and_whitespace_only_titles_fold_to_empty() {
        assert_eq!(fold_title(""), "");
        assert_eq!(fold_title("   !!!   "), "");
    }
}
