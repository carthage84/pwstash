use rand::Rng;

use crate::error::StashError;

pub const DEFAULT_LENGTH: usize = 20;
pub const MIN_LENGTH: usize = 8;
pub const MAX_LENGTH: usize = 128;

const LOWER: &[u8] = b"abcdefghijklmnopqrstuvwxyz";
const UPPER: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ";
const DIGITS: &[u8] = b"0123456789";
const SYMBOLS: &[u8] = b"!@#$%^&*()-_=+[]{}";

/// Generate a password with lower, upper, digits, and symbols.
pub fn generate(length: usize) -> Result<String, StashError> {
    if !(MIN_LENGTH..=MAX_LENGTH).contains(&length) {
        return Err(StashError::InvalidGenerateLength {
            min: MIN_LENGTH,
            max: MAX_LENGTH,
        });
    }

    let classes: [&[u8]; 4] = [LOWER, UPPER, DIGITS, SYMBOLS];
    let alphabet: Vec<u8> = classes
        .iter()
        .flat_map(|class| class.iter().copied())
        .collect();
    let mut rng = rand::rngs::OsRng;
    let mut bytes = Vec::with_capacity(length);

    for class in classes {
        bytes.push(class[rng.gen_range(0..class.len())]);
    }
    while bytes.len() < length {
        bytes.push(alphabet[rng.gen_range(0..alphabet.len())]);
    }
    for i in (1..bytes.len()).rev() {
        let j = rng.gen_range(0..=i);
        bytes.swap(i, j);
    }

    Ok(String::from_utf8(bytes).expect("generator uses ASCII"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn respects_length() {
        let password = generate(12).unwrap();
        assert_eq!(password.len(), 12);
    }

    #[test]
    fn includes_each_class() {
        let password = generate(20).unwrap();
        assert!(password.chars().any(|c| c.is_ascii_lowercase()));
        assert!(password.chars().any(|c| c.is_ascii_uppercase()));
        assert!(password.chars().any(|c| c.is_ascii_digit()));
        assert!(password.chars().any(|c| SYMBOLS.contains(&(c as u8))));
    }

    #[test]
    fn rejects_too_short() {
        assert!(matches!(
            generate(4),
            Err(StashError::InvalidGenerateLength { .. })
        ));
    }

    #[test]
    fn two_calls_differ() {
        let a = generate(20).unwrap();
        let b = generate(20).unwrap();
        assert_ne!(a, b);
    }
}
