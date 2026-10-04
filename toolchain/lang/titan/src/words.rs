//! `words` -- the 25 words of TITAN++, each with the LEVEL that brings it.
//!
//! The list and the levels are `TITAN_MAESTRO` 4.4 and 14.14. The frontend
//! climbs the ladder one level at a time: a word of a level that does not
//! exist yet is not "a syntax error" -- it is T0040, and it says which level
//! brings it.
//!
//! ** THE TITAN GUARDIAN is the test at the bottom: 25 words, a ceiling of 30,
//! no word twice, and the ladder adding up exactly as 14.14 promises. A word
//! that sneaks in without its level, or a 31st, stops `cargo test` here.

/// The level this frontend understands today. Everything above says "not yet".
pub const LEVEL_NOW: u8 = 10;

/// The ceiling (TITAN_MAESTRO 4.4). A new word gets in only if it removes a
/// confusion, and it goes through here.
pub const CEILING: usize = 30;

#[derive(Debug, Clone, Copy)]
pub struct Word {
    pub text: &'static str,
    pub level: u8,
}

const fn w(text: &'static str, level: u8) -> Word {
    Word { text, level }
}

/// In the order of the ladder (14.14).
pub const WORDS: [Word; 25] = [
    w("fn", 0),
    w("let", 1),
    w("mut", 2),
    w("if", 3),
    w("else", 3),
    w("true", 3),
    w("false", 3),
    w("and", 3),
    w("or", 3),
    w("not", 3),
    w("for", 4),
    w("in", 4),
    w("while", 4),
    w("break", 4),
    w("continue", 4),
    w("return", 5),
    w("type", 6),
    w("take", 7),
    w("enum", 8),
    w("match", 8),
    w("mod", 9),
    w("use", 9),
    w("pub", 9),
    w("trait", 10),
    w("gpu", 11),
];

/// What each level lets you write, in one line (14.14), for the T0040 message.
pub fn level_name(level: u8) -> &'static str {
    match level {
        0 => "saludar",
        1 => "calcular",
        2 => "contar",
        3 => "decidir",
        4 => "repetir",
        5 => "funciones con resultado",
        6 => "registros",
        7 => "prestar y entregar",
        8 => "casos con datos",
        9 => "varios ficheros",
        10 => "comportamientos",
        11 => "la 3060",
        _ => "?",
    }
}

pub fn find(text: &str) -> Option<Word> {
    WORDS.iter().copied().find(|w| w.text == text)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// THE TITAN GUARDIAN.
    #[test]
    fn titan_guardian_25_words_under_the_ceiling_and_the_ladder_adds_up() {
        assert_eq!(WORDS.len(), 25);
        assert!(WORDS.len() <= CEILING);
        for (i, a) in WORDS.iter().enumerate() {
            assert!(a.text.bytes().all(|b| b.is_ascii_lowercase()), "{}", a.text);
            assert!(WORDS[i + 1..].iter().all(|b| b.text != a.text), "twice: {}", a.text);
        }
        // The totals of 14.14: with this level, this many words.
        let totals = [(0, 1), (1, 2), (2, 3), (3, 10), (4, 15), (5, 16), (6, 17), (7, 18), (8, 20), (9, 23), (10, 24), (11, 25)];
        for (level, total) in totals {
            assert_eq!(WORDS.iter().filter(|w| w.level <= level).count(), total, "level {}", level);
        }
        // And the ladder only climbs.
        assert!(WORDS.windows(2).all(|p| p[0].level <= p[1].level));
    }

    #[test]
    fn every_level_has_its_line() {
        for w in WORDS {
            assert_ne!(level_name(w.level), "?");
        }
    }
}
