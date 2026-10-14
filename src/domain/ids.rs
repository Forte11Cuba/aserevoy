use std::fmt;

use rand::RngExt;

/// A Telegram user id.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct UserId(pub i64);

/// Short, human-friendly request id shown in the channel and in chats (e.g. `AB7K`).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RequestId(String);

/// No 0/O or 1/I, so ids can be read aloud and typed without mistakes.
const ALPHABET: &[u8] = b"ABCDEFGHJKLMNPQRSTUVWXYZ23456789";
const LEN: usize = 4;

impl RequestId {
    pub fn generate(rng: &mut impl rand::Rng) -> Self {
        let id = (0..LEN)
            .map(|_| ALPHABET[rng.random_range(0..ALPHABET.len())] as char)
            .collect();
        RequestId(id)
    }

    /// Accepts ids typed by users, case-insensitively.
    pub fn parse(s: &str) -> Option<Self> {
        let s = s.trim().to_ascii_uppercase();
        let valid = s.len() == LEN && s.bytes().all(|b| ALPHABET.contains(&b));
        valid.then_some(RequestId(s))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// One-use code of a driver invitation link (`t.me/AsereVoyBot?start=inv_<code>`).
/// Long enough that it cannot be guessed.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct InviteCode(String);

const INVITE_LEN: usize = 12;

impl InviteCode {
    pub fn generate(rng: &mut impl rand::Rng) -> Self {
        let code = (0..INVITE_LEN)
            .map(|_| ALPHABET[rng.random_range(0..ALPHABET.len())] as char)
            .collect();
        InviteCode(code)
    }

    pub fn parse(s: &str) -> Option<Self> {
        let valid = s.len() == INVITE_LEN && s.bytes().all(|b| ALPHABET.contains(&b));
        valid.then(|| InviteCode(s.to_owned()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for RequestId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_ids_parse_back() {
        let mut rng = rand::rng();
        for _ in 0..1000 {
            let id = RequestId::generate(&mut rng);
            assert_eq!(RequestId::parse(id.as_str()), Some(id));
        }
    }

    #[test]
    fn invite_codes_are_long_and_parse_back() {
        let code = InviteCode::generate(&mut rand::rng());
        assert_eq!(code.as_str().len(), 12);
        assert_eq!(InviteCode::parse(code.as_str()), Some(code));
        assert!(InviteCode::parse("AB7K").is_none());
    }

    #[test]
    fn parse_is_case_insensitive_and_rejects_ambiguous_chars() {
        assert_eq!(RequestId::parse(" ab7k ").unwrap().as_str(), "AB7K");
        assert!(RequestId::parse("AB0K").is_none());
        assert!(RequestId::parse("ABI1").is_none());
        assert!(RequestId::parse("ABC").is_none());
    }
}
