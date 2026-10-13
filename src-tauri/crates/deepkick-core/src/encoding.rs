//! Repairing text that was stored with the wrong character set.
//!
//! Old Windows taggers wrote Cyrillic (CP1251) bytes into ID3v2 frames marked as Latin-1,
//! so readers show `Êîòÿòà` instead of `Котята`. The same frames sometimes hold UTF-8
//! bytes marked as Latin-1. Both cases are recognisable: every character of the broken
//! text is in the Latin-1 range, and re-decoding the bytes yields proper letters.

/// CP1251 code points 0x80..=0xFF (0x98 is unassigned).
const CP1251_HIGH: [char; 128] = [
	'Ђ', 'Ѓ', '‚', 'ѓ', '„', '…', '†', '‡', '€', '‰', 'Љ', '‹', 'Њ', 'Ќ', 'Ћ', 'Џ', //
	'ђ', '‘', '’', '“', '”', '•', '–', '—', '\0', '™', 'љ', '›', 'њ', 'ќ', 'ћ', 'џ', //
	'\u{a0}', 'Ў', 'ў', 'Ј', '¤', 'Ґ', '¦', '§', 'Ё', '©', 'Є', '«', '¬', '\u{ad}', '®', 'Ї', //
	'°', '±', 'І', 'і', 'ґ', 'µ', '¶', '·', 'ё', '№', 'є', '»', 'ј', 'Ѕ', 'ѕ', 'ї', //
	'А', 'Б', 'В', 'Г', 'Д', 'Е', 'Ж', 'З', 'И', 'Й', 'К', 'Л', 'М', 'Н', 'О', 'П', //
	'Р', 'С', 'Т', 'У', 'Ф', 'Х', 'Ц', 'Ч', 'Ш', 'Щ', 'Ъ', 'Ы', 'Ь', 'Э', 'Ю', 'Я', //
	'а', 'б', 'в', 'г', 'д', 'е', 'ж', 'з', 'и', 'й', 'к', 'л', 'м', 'н', 'о', 'п', //
	'р', 'с', 'т', 'у', 'ф', 'х', 'ц', 'ч', 'ш', 'щ', 'ъ', 'ы', 'ь', 'э', 'ю', 'я',
];

/// How a string was repaired, for reporting.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Repair {
	/// CP1251 bytes that had been read as Latin-1
	Cp1251,
	/// UTF-8 bytes that had been read as Latin-1
	Utf8,
}

impl Repair {
	pub fn label(self) -> &'static str {
		match self {
			Repair::Cp1251 => "CP1251",
			Repair::Utf8 => "UTF-8",
		}
	}
}

fn is_cyrillic(c: char) -> bool {
	matches!(c, '\u{0400}'..='\u{04FF}')
}

/// The original bytes of a string that was decoded as Latin-1, if it was.
fn latin1_bytes(text: &str) -> Option<Vec<u8>> {
	text.chars().map(|c| u8::try_from(u32::from(c)).ok()).collect()
}

/// Returns the repaired text and what was wrong with it, or `None` when the text is fine
/// (or cannot be told apart from genuine Latin-1 text).
pub fn repair(text: &str) -> Option<(String, Repair)> {
	let bytes = latin1_bytes(text)?;
	if bytes.iter().all(|b| b.is_ascii()) {
		return None;
	}

	// UTF-8 read as Latin-1: valid UTF-8 with non-ASCII characters is far too unlikely by accident
	if let Ok(utf8) = std::str::from_utf8(&bytes) {
		if utf8.chars().any(|c| !c.is_ascii() && (c.is_alphabetic() || c.is_ascii_punctuation())) {
			return Some((utf8.to_string(), Repair::Utf8));
		}
	}

	// CP1251 read as Latin-1: the letters must come out Cyrillic
	let mut decoded = String::with_capacity(text.len());
	for &b in &bytes {
		let c = if b < 0x80 { b as char } else { CP1251_HIGH[(b - 0x80) as usize] };
		if c == '\0' {
			return None;
		}
		decoded.push(c);
	}
	let letters: Vec<char> = decoded.chars().filter(|c| c.is_alphabetic()).collect();
	let cyrillic = letters.iter().filter(|c| is_cyrillic(**c)).count();
	let non_ascii_letters = letters.iter().filter(|c| !c.is_ascii()).count();
	// every non-ASCII letter became Cyrillic, and there is a real word's worth of them
	if cyrillic >= 2 && cyrillic == non_ascii_letters {
		return Some((decoded, Repair::Cp1251));
	}
	None
}

/// Repairs the text when needed, keeping the original otherwise.
pub fn fixed(text: &str) -> String {
	repair(text).map(|(t, _)| t).unwrap_or_else(|| text.to_string())
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn repairs_cp1251_read_as_latin1() {
		assert_eq!(repair("Êîòÿòà"), Some(("Котята".into(), Repair::Cp1251)));
		assert_eq!(repair("Äåëüôèí"), Some(("Дельфин".into(), Repair::Cp1251)));
		assert_eq!(
			repair("Õðîíèêè Ïîñëåäíèõ Äíåé").map(|r| r.0),
			Some("Хроники Последних Дней".into())
		);
		// ё and Ё live in the 0x80..0xBF block
		assert_eq!(repair("¨ëêà").map(|r| r.0), Some("Ёлка".into()));
	}

	#[test]
	fn repairs_utf8_read_as_latin1() {
		let broken: String = "Котята".bytes().map(|b| b as char).collect();
		assert_eq!(repair(&broken), Some(("Котята".into(), Repair::Utf8)));
		let broken: String = "Björk".bytes().map(|b| b as char).collect();
		assert_eq!(repair(&broken), Some(("Björk".into(), Repair::Utf8)));
	}

	#[test]
	fn leaves_real_text_alone() {
		assert_eq!(repair("Radiohead"), None);
		assert_eq!(repair("Björk"), None);
		assert_eq!(repair("Motörhead"), None);
		assert_eq!(repair("Café del Mar"), None);
		assert_eq!(repair("Mylène Farmer"), None);
		assert_eq!(repair("Котята"), None);
		assert_eq!(repair("713"), None);
		assert_eq!(repair(""), None);
	}
}
