//! Phone numbers typed or shared by users.

/// Keeps a leading `+` and the digits; accepts 8 to 15 digits (Cuban mobiles
/// are 8 digits, or 10 with the 53 prefix). Spaces, dashes and dots are ignored.
pub fn normalize(input: &str) -> Option<String> {
    let input = input.trim();
    let (plus, rest) = match input.strip_prefix('+') {
        Some(rest) => ("+", rest),
        None => ("", input),
    };
    if !rest.chars().all(|c| c.is_ascii_digit() || matches!(c, ' ' | '-' | '.' | '(' | ')')) {
        return None;
    }
    let digits: String = rest.chars().filter(char::is_ascii_digit).collect();
    (8..=15).contains(&digits.len()).then(|| format!("{plus}{digits}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_common_formats() {
        assert_eq!(normalize("5123 4567").as_deref(), Some("51234567"));
        assert_eq!(normalize("+53 5-123-4567").as_deref(), Some("+5351234567"));
        assert_eq!(normalize("(53) 51234567").as_deref(), Some("5351234567"));
    }

    #[test]
    fn rejects_text_and_wrong_lengths() {
        assert!(normalize("llámame").is_none());
        assert!(normalize("1234").is_none());
        assert!(normalize("+1234567890123456").is_none());
        assert!(normalize("5123x4567").is_none());
    }
}
