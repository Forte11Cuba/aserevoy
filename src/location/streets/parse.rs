//! Written text → `Address`: the street, its corners or a place, and the hints
//! after commas. Every name comes out normalized for comparison.

#[derive(Debug, Clone, PartialEq)]
pub(super) enum AddressKind {
    Between { street: String, a: String, b: String },
    Corner { street: String, cross: String },
    Place(String),
}

#[derive(Debug, Clone, PartialEq)]
pub(super) struct Address {
    pub(super) kind: AddressKind,
    /// What came after commas: maybe a reparto or municipality.
    pub(super) hints: Vec<String>,
}

/// Splits a written address into its parts, all normalized.
pub(super) fn parse(text: &str) -> Address {
    let lowered = text.to_lowercase();
    // The address is the first part with something useful once cleaned
    // ("edif 12 apto 4, 23 e/ L y M"); the parts after it may be hints.
    let mut parts = lowered.split([',', ';', '\n']).map(clean_words).skip_while(|p| p.trim().is_empty());
    let main = parts.next().unwrap_or_default();
    let hints: Vec<String> = parts.map(|p| normalize(&p)).filter(|h| !h.is_empty()).collect();

    // Streets, without a trailing landmark ("23 e/ L y M por la cafetería");
    // if there are no streets, the whole text may name a place.
    let kind = streets_in(&without_landmark(&main)).unwrap_or_else(|| AddressKind::Place(normalize(&main)));
    Address { kind, hints }
}

/// "X entre A y B" or "X esquina A" in the (cleaned) text, if any.
fn streets_in(text: &str) -> Option<AddressKind> {
    let main = format!(" {} ", unglue_y(text));
    for marker in [" e/ ", " entre ", " e ", " e. "] {
        if let Some((street, rest)) = main.split_once(marker)
            && let Some((a, b)) = rest.split_once(" y ")
        {
            return Some(AddressKind::Between { street: normalize(street), a: normalize(a), b: normalize(b) });
        }
    }
    for marker in [" esquina a ", " esquina ", " esq. ", " esq ", " y "] {
        if let Some((street, cross)) = main.split_once(marker) {
            let (street, cross) = (normalize(street), normalize(cross));
            if !street.is_empty() && !cross.is_empty() {
                return Some(AddressKind::Corner { street, cross });
            }
        }
    }
    None
}

/// Phrases that start a landmark ("por la cafetería el Riki", "frente al
/// parque"): what follows describes the surroundings, not the streets.
const LANDMARK_STARTS: [&str; 12] = [
    " por la ", " por el ", " por los ", " por las ", " frente a", " cerca de", " cerca del", " al lado de",
    " al lado del", " detras de", " detrás de", " junto a",
];

/// The address without a trailing landmark. A text that is only a landmark
/// ("frente al Coppelia") is kept: then the landmark is the place.
fn without_landmark(main: &str) -> String {
    let padded = format!(" {main} ");
    let cut = LANDMARK_STARTS.iter().filter_map(|m| padded.find(m)).filter(|&at| at > 0).min();
    match cut {
        Some(at) => padded[..at].trim().to_owned(),
        None => main.to_owned(),
    }
}

/// "3ra ay 3b": a street letter typed together with "y" ("ay" for "a y").
/// Only single letters a to h, the ones Cuban streets use ("3ra A").
fn unglue_y(text: &str) -> String {
    text.split(' ')
        .map(|w| match w.as_bytes() {
            [l @ b'a'..=b'h', b'y'] => format!("{} y", *l as char),
            _ => w.to_owned(),
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Words that bring the next one with them and say nothing about where:
/// house numbers, apartments, floors, buildings ("#1052", "apto 4", "piso 2").
const DROP_WITH_NEXT: [&str; 20] = [
    "#", "no", "no.", "nro", "nro.", "num", "num.", "número", "numero", "apto", "apto.", "apt", "apt.",
    "apartamento", "piso", "edif", "edif.", "edificio", "local", "interior",
];
/// Words that say nothing about where, on their own.
const DROP_ALONE: [&str; 2] = ["altos", "bajos"];

/// Normalizes the separators and drops what does not locate the place:
/// "%" and "e/" mean "entre", "&" means "y", "esq/" means "esquina", and
/// house numbers, apartments and floors go.
fn clean_words(text: &str) -> String {
    let spaced = text
        .replace('%', " entre ")
        .replace('&', " y ")
        .replace("esq/", " esq ")
        .replace("e/", " e/ ")
        .replace(['°', 'º'], " ")
        .replace('.', ". ");
    let mut words = spaced.split_whitespace().peekable();
    let mut kept = Vec::new();
    while let Some(word) = words.next() {
        if DROP_WITH_NEXT.contains(&word) {
            // The number (or "4b") that goes with it.
            if words.peek().is_some_and(|next| next.chars().next().is_some_and(|c| c.is_ascii_digit())) {
                words.next();
            }
        } else if word.starts_with('#') || DROP_ALONE.contains(&word) {
            continue;
        } else {
            kept.push(word);
        }
    }
    kept.join(" ")
}

/// Words that do not tell streets apart.
const GENERIC: [&str; 18] = [
    "calle", "avenida", "ave", "av", "calzada", "calz", "de", "del", "la", "el", "los", "las", "frente", "al", "a",
    "cerca", "reparto", "rpto",
];

/// Lowercase, no accents or punctuation, without generic words, and with
/// ordinals as numbers: "Calle 23" → "23", "5ta Avenida" → "5",
/// "Calzada de Infanta" → "infanta", "Quinta" → "5".
pub fn normalize(text: &str) -> String {
    single_letters(&normalize_words(text))
}

/// A doubled letter counts once: "Copelia" is "Coppelia" (digits stay: "11").
fn single_letters(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        if !(c.is_alphabetic() && out.ends_with(c)) {
            out.push(c);
        }
    }
    out
}

fn normalize_words(text: &str) -> String {
    // "Calzada (7)": the name before the brackets is all generic words, but
    // it is what people say ("5ta y Calzada"); keep it after the number, so
    // both "Calzada" and "7" match.
    if let Some((head, rest)) = text.split_once('(') {
        let head = words_of(head);
        if !head.is_empty() && head.iter().all(|w| GENERIC.contains(&w.as_str())) {
            let rest = normalize_words(rest);
            return if rest.is_empty() { head.join(" ") } else { format!("{rest} {}", head.join(" ")) };
        }
    }
    let words = words_of(text);
    let meaningful: Vec<&String> = words.iter().filter(|w| !GENERIC.contains(&w.as_str())).collect();
    // "Paseo" or "La Rampa" alone are names: keep the words if all are generic.
    if meaningful.is_empty() {
        return words.join(" ");
    }
    meaningful.iter().map(|w| w.as_str()).collect::<Vec<_>>().join(" ")
}

/// Lowercase words without accents or punctuation, ordinals as numbers.
fn words_of(text: &str) -> Vec<String> {
    let plain: String = text
        .to_lowercase()
        .chars()
        .map(|ch| match ch {
            'á' | 'à' | 'ä' => 'a',
            'é' | 'è' | 'ë' => 'e',
            'í' | 'ì' | 'ï' => 'i',
            'ó' | 'ò' | 'ö' => 'o',
            'ú' | 'ù' | 'ü' => 'u',
            'ñ' => 'n',
            c if c.is_alphanumeric() => c,
            _ => ' ',
        })
        .collect();
    join_letter_suffixes(plain.split_whitespace().map(ordinal).collect())
}

/// A number followed by a single letter is one name, however it is written:
/// "160 C" and "160C" → "160c"; "7ma C" → "7c".
fn join_letter_suffixes(words: Vec<String>) -> Vec<String> {
    let mut joined: Vec<String> = Vec::with_capacity(words.len());
    for word in words {
        let is_letter = word.len() == 1 && word.chars().all(|c| c.is_ascii_lowercase());
        match joined.last_mut() {
            Some(prev) if is_letter && prev.chars().all(|c| c.is_ascii_digit()) => prev.push_str(&word),
            _ => joined.push(word),
        }
    }
    joined
}

/// "5ta" → "5", "1ra" → "1", "quinta" → "5".
fn ordinal(word: &str) -> String {
    const WORDS: [(&str, &str); 10] = [
        ("primera", "1"),
        ("segunda", "2"),
        ("tercera", "3"),
        ("cuarta", "4"),
        ("quinta", "5"),
        ("sexta", "6"),
        ("septima", "7"),
        ("octava", "8"),
        ("novena", "9"),
        ("decima", "10"),
    ];
    if let Some((_, n)) = WORDS.iter().find(|(w, _)| *w == word) {
        return (*n).to_owned();
    }
    let digits: String = word.chars().take_while(char::is_ascii_digit).collect();
    let suffix = &word[digits.len()..];
    // "3ra", "3era", "1ro", "1ero", "2do", "5ta", "9na"… (and "3a", "1o" for 3ª, 1º).
    const SUFFIXES: [&str; 17] =
        ["ra", "da", "ta", "na", "ma", "va", "o", "a", "er", "era", "ero", "ro", "do", "to", "no", "mo", "vo"];
    if !digits.is_empty() && SUFFIXES.contains(&suffix) {
        return digits;
    }
    word.to_owned()
}

/// A written name matches an OSM name if they are equal; if all the written
/// words appear in it ("lazaro" ⊂ "san lazaro"); or, for a number, if the
/// OSM name starts with it ("23" ⊂ "23 rampa", but not "230").
pub(super) fn name_matches(osm: &str, written: &str) -> bool {
    if written.is_empty() {
        return false;
    }
    if osm == written {
        return true;
    }
    let osm_words: Vec<&str> = osm.split(' ').collect();
    if written.chars().all(|c| c.is_ascii_digit()) {
        return osm_words.first() == Some(&written);
    }
    written.split(' ').all(|w| osm_words.contains(&w))
}

/// Two OSM names of the same street, as where a street changes name.
pub(super) fn same_street(a: &str, b: &str) -> bool {
    name_matches(a, b) || name_matches(b, a)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_street_names() {
        assert_eq!(normalize("Calle 23"), "23");
        assert_eq!(normalize("5ta Avenida"), "5");
        assert_eq!(normalize("Quinta Avenida"), "5");
        assert_eq!(normalize("Calzada (7)"), "7 calzada");
        assert_eq!(normalize("Avenida de los Presidentes (Calle G)"), "presidentes g");
        assert_eq!(normalize("Calzada de Infanta"), "infanta");
        assert_eq!(normalize("Calle San Lázaro"), "san lazaro");
        assert_eq!(normalize("Paseo"), "paseo");
        assert_eq!(normalize("Calle L"), "l");
        assert_eq!(normalize("1ra"), "1");
        assert_eq!(normalize("3era"), "3");
        assert_eq!(normalize("1ero"), "1");
        assert_eq!(normalize("2do"), "2");
        assert_eq!(normalize("160 C"), "160c");
        assert_eq!(normalize("160C"), "160c");
        assert_eq!(normalize("7ma C"), "7c");
        assert_eq!(normalize("Avenida Séptima"), "7");
    }

    #[test]
    fn parses_cuban_address_forms() {
        let between = AddressKind::Between { street: "23".into(), a: "l".into(), b: "m".into() };
        assert_eq!(parse("23 e/ L y M").kind, between);
        assert_eq!(parse("Calle 23 entre L y M, Vedado").kind, between);
        assert_eq!(parse("Calle 23 #1052 e/ L y M").kind, between);
        assert_eq!(parse("Calle 23 entre L y M, Vedado").hints, vec!["vedado"]);
        let corner = AddressKind::Corner { street: "infanta".into(), cross: "san lazaro".into() };
        assert_eq!(parse("Infanta esq. San Lázaro").kind, corner);
        assert_eq!(parse("infanta y san lazaro").kind, corner);
        assert_eq!(parse("frente al Coppelia").kind, AddressKind::Place("copelia".into()));
        // Cuban shorthands: "%" for "entre", "e/" glued, "&" for "y", "esq/".
        assert_eq!(parse("23 % L y M").kind, between);
        assert_eq!(parse("23e/L y M").kind, between);
        assert_eq!(parse("Infanta esq/ San Lázaro").kind, corner);
        assert_eq!(parse("infanta & san lazaro").kind, corner);
        // House numbers, apartments and floors say nothing about where.
        assert_eq!(parse("Calle 23 #1052 apto 4 e/ L y M").kind, between);
        assert_eq!(parse("23 nro 1052 piso 2 altos e/ L y M").kind, between);
        assert_eq!(parse("edif 12 apt 4b, 23 e/ L y M").kind, between);
        // A landmark after the streets is not part of them.
        assert_eq!(parse("23 e/ L y M por la cafeteria el riki").kind, between);
        assert_eq!(parse("infanta y san lazaro frente al parque").kind, corner);
        assert_eq!(parse("frente al Coppelia").kind, AddressKind::Place("copelia".into()));
        let glued = AddressKind::Between { street: "160e".into(), a: "3a".into(), b: "3b".into() };
        assert_eq!(parse("160e entre 3era ay 3b").kind, glued);
    }

    #[test]
    fn names_match_by_words() {
        assert!(name_matches("san lazaro", "lazaro"));
        assert!(name_matches("23", "23"));
        assert!(!name_matches("230", "23")); // numbers match whole
        assert!(name_matches("23 rampa", "23"));
        assert!(same_street("23", "23 rampa"));
        assert!(!same_street("23", "l"));
        assert!(!same_street("san lazaro", "san rafael"));
        assert!(!name_matches("infanta", ""));
    }
}
