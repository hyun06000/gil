//! Hangul-only NFC/NFD for the `project_root` boundary.
//!
//! macOS stores folder names decomposed (NFD); a Host or Agent that retypes a Korean path usually
//! sends it composed (NFC). The bytes differ, the folder is the same. This module maps one form to
//! the other with the Unicode Hangul algorithm alone — no general normalization tables, no crate.
//! Compatibility jamo (U+3131 block, the lone `ㄱ`·`ㅏ` letters) belong to neither form and cannot be
//! matched to any folder; [`has_compatibility_jamo`] names them so the refusal can say so.

const S_BASE: u32 = 0xAC00;
const L_BASE: u32 = 0x1100;
const V_BASE: u32 = 0x1161;
const T_BASE: u32 = 0x11A7;
const L_COUNT: u32 = 19;
const V_COUNT: u32 = 21;
const T_COUNT: u32 = 28;
const N_COUNT: u32 = V_COUNT * T_COUNT;
const S_COUNT: u32 = L_COUNT * N_COUNT;

fn syllable(c: char) -> Option<u32> {
    let i = (c as u32).wrapping_sub(S_BASE);
    (i < S_COUNT).then_some(i)
}

/// Hangul syllables become L·V(·T) jamo; everything else is copied.
pub fn nfd(text: &str) -> String {
    let mut out = String::with_capacity(text.len() * 2);
    for c in text.chars() {
        match syllable(c) {
            Some(i) => {
                out.push(char::from_u32(L_BASE + i / N_COUNT).expect("leading jamo"));
                out.push(char::from_u32(V_BASE + (i % N_COUNT) / T_COUNT).expect("vowel jamo"));
                let t = i % T_COUNT;
                if t != 0 { out.push(char::from_u32(T_BASE + t).expect("trailing jamo")); }
            }
            None => out.push(c),
        }
    }
    out
}

/// L·V(·T) jamo sequences become syllables; everything else is copied.
pub fn nfc(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        let l = (c as u32).wrapping_sub(L_BASE);
        if l < L_COUNT {
            if let Some(&v) = chars.peek() {
                let v_index = (v as u32).wrapping_sub(V_BASE);
                if v_index < V_COUNT {
                    chars.next();
                    let mut s = S_BASE + (l * V_COUNT + v_index) * T_COUNT;
                    if let Some(&t) = chars.peek() {
                        let t_index = (t as u32).wrapping_sub(T_BASE);
                        if t_index > 0 && t_index < T_COUNT { chars.next(); s += t_index; }
                    }
                    out.push(char::from_u32(s).expect("hangul syllable"));
                    continue;
                }
            }
        }
        out.push(c);
    }
    out
}

/// Hangul Compatibility Jamo (U+3131–U+318E): letters typed apart, matching no folder name.
pub fn has_compatibility_jamo(text: &str) -> bool {
    text.chars().any(|c| ('\u{3131}'..='\u{318E}').contains(&c))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn syllables_round_trip_through_both_forms() {
        let composed = "/tmp/한글 프로젝트/값";
        let decomposed = nfd(composed);
        assert_ne!(decomposed, composed);
        // `/tmp/` 5 · 한글 6 · space 1 · 프로젝트 9 · `/` 1 · 값 3
        assert_eq!(decomposed.chars().count(), 5 + 6 + 1 + 9 + 1 + 3);
        assert_eq!(nfc(&decomposed), composed);
        assert_eq!(nfd(&decomposed), decomposed, "decomposing twice changes nothing");
        assert_eq!(nfc(composed), composed, "composing composed text changes nothing");
    }

    #[test]
    fn non_hangul_and_compatibility_jamo_pass_through_untouched() {
        assert_eq!(nfd("ascii/é"), "ascii/é");
        assert_eq!(nfc("ㅎㅏㄴ"), "ㅎㅏㄴ");
        assert!(has_compatibility_jamo("프로젝트ㅅ"));
        assert!(!has_compatibility_jamo(&nfd("프로젝트")));
    }
}
