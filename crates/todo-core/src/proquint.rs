//! Proquint rendering for identifiers: readable, spellable, pronounceable.
//!
//! Follows Wilkerson's proposal: each 16-bit word becomes five characters,
//! alternating consonant, vowel, consonant, vowel, consonant, drawn from sixteen
//! consonants and four vowels. `0x7F00 0x0001`, which is 127.0.0.1, renders as
//! `lusab-babad`.
//!
//! The point here is that a task can be read aloud, typed from memory, and
//! recognized in a list, which a hexadecimal UUID cannot.

/// The sixteen consonants, one per four bits.
const CONSONANTS: [char; 16] = [
    'b', 'd', 'f', 'g', 'h', 'j', 'k', 'l', 'm', 'n', 'p', 'r', 's', 't', 'v', 'z',
];

/// The four vowels, one per two bits.
const VOWELS: [char; 4] = ['a', 'i', 'o', 'u'];

/// Renders one 16-bit word as a five-character quint.
pub fn encode_u16(word: u16) -> String {
    let mut rendered = String::with_capacity(5);
    let pick = |table: &[char], index: usize| table.get(index).copied().unwrap_or('b');
    let first = usize::from(word >> 12) & 0x0f;
    let second = usize::from(word >> 10) & 0x03;
    let third = usize::from(word >> 6) & 0x0f;
    let fourth = usize::from(word >> 4) & 0x03;
    let fifth = usize::from(word) & 0x0f;
    rendered.push(pick(&CONSONANTS, first));
    rendered.push(pick(&VOWELS, second));
    rendered.push(pick(&CONSONANTS, third));
    rendered.push(pick(&VOWELS, fourth));
    rendered.push(pick(&CONSONANTS, fifth));
    rendered
}

/// Reads one five-character quint back into its 16-bit word.
///
/// Returns `None` for anything that is not five characters drawn from the
/// alphabets, so a mistyped identifier fails rather than decoding to a wrong one.
pub fn decode_u16(quint: &str) -> Option<u16> {
    let characters = quint.chars().collect::<Vec<_>>();
    let [first, second, third, fourth, fifth] = characters.as_slice() else {
        return None;
    };
    let consonant = |value: &char| {
        CONSONANTS
            .iter()
            .position(|candidate| candidate == value)
            .and_then(|index| u16::try_from(index).ok())
    };
    let vowel = |value: &char| {
        VOWELS
            .iter()
            .position(|candidate| candidate == value)
            .and_then(|index| u16::try_from(index).ok())
    };
    Some(
        (consonant(first)? << 12)
            | (vowel(second)? << 10)
            | (consonant(third)? << 6)
            | (vowel(fourth)? << 4)
            | consonant(fifth)?,
    )
}

/// Renders bytes as hyphen-separated quints, two bytes at a time.
///
/// A trailing odd byte is padded with zero rather than dropped, so nothing in the
/// input goes unrendered.
pub fn encode_bytes(bytes: &[u8]) -> String {
    bytes
        .chunks(2)
        .map(|pair| {
            let high = pair.first().copied().unwrap_or(0);
            let low = pair.get(1).copied().unwrap_or(0);
            encode_u16(u16::from(high) << 8 | u16::from(low))
        })
        .collect::<Vec<_>>()
        .join("-")
}

/// Renders the last four bytes as two quints, for a short spoken identifier.
///
/// The tail is used rather than the head because a UUIDv7 begins with a timestamp
/// that neighboring tasks share; the tail is what actually distinguishes them, and
/// it is what reference matching already compares.
pub fn short(bytes: &[u8]) -> String {
    let tail = bytes.len().saturating_sub(4);
    encode_bytes(bytes.get(tail..).unwrap_or(bytes))
}

/// Reads hyphenated quints back into the hexadecimal they stand for.
///
/// Returns `None` unless every part is a valid quint, so an ordinary hexadecimal
/// reference is never mistaken for a proquint one. This is what lets an identifier
/// that was printed as `lusab-babad` be typed back in.
pub fn decode_to_hex(text: &str) -> Option<String> {
    let text = text.trim();
    if text.is_empty() {
        return None;
    }
    let mut hex = String::new();
    for part in text.split('-') {
        let word = decode_u16(part)?;
        hex.push_str(&format!("{word:04x}"));
    }
    Some(hex)
}

#[cfg(test)]
mod tests {
    use super::{decode_to_hex, decode_u16, encode_bytes, encode_u16, short};

    #[test]
    fn the_canonical_example_renders_as_published() {
        // 127.0.0.1 from Wilkerson's proposal.
        assert_eq!(encode_bytes(&[0x7f, 0x00, 0x00, 0x01]), "lusab-babad");
    }

    #[test]
    fn a_quint_round_trips() {
        for word in [0_u16, 1, 0x7f00, 0xffff, 0x1234] {
            assert_eq!(decode_u16(&encode_u16(word)), Some(word));
        }
    }

    #[test]
    fn every_quint_is_five_characters() {
        assert!(
            (0..=u16::MAX)
                .step_by(97)
                .all(|word| encode_u16(word).chars().count() == 5)
        );
    }

    #[test]
    fn a_mistyped_quint_decodes_to_nothing() {
        assert_eq!(decode_u16("lusa"), None);
        assert_eq!(decode_u16("lusae"), None);
        assert_eq!(decode_u16("xusab"), None);
    }

    #[test]
    fn a_short_identifier_takes_the_distinguishing_tail() {
        let uuid = [
            0x01, 0x98, 0xf5, 0x8a, 0x1c, 0x00, 0x70, 0x00, 0x80, 0x00, 0x00, 0x00, 0x7f, 0x00,
            0x00, 0x01,
        ];
        assert_eq!(short(&uuid), "lusab-babad");
    }

    #[test]
    fn a_printed_identifier_can_be_typed_back_in() {
        assert_eq!(decode_to_hex("lusab-babad").as_deref(), Some("7f000001"));
    }

    #[test]
    fn hexadecimal_is_not_mistaken_for_a_proquint() {
        assert_eq!(decode_to_hex("7f000001"), None);
        assert_eq!(decode_to_hex("0198f58a"), None);
        assert_eq!(decode_to_hex(""), None);
    }

    #[test]
    fn an_odd_byte_is_still_rendered() {
        assert_eq!(encode_bytes(&[0x7f]).chars().count(), 5);
    }
}
