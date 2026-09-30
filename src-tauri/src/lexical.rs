use std::collections::HashSet;
use regex::Regex;
use serde::{Deserialize, Serialize};
use unicode_normalization::UnicodeNormalization;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MatchResult {
    pub matched: bool,
    pub word: String,
    pub rule: String,
    pub category: String,
    pub start_char: usize,
    pub end_char: usize,
    pub severity: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuleEntry {
    pub pattern: String,
    pub category: String,
    pub severity: String,
    pub is_regex: bool,
}

pub struct LexicalEngine {
    pub rules: Vec<RuleEntry>,
    compiled_regexes: Vec<(Regex, String, String, String)>,
    raw_words: HashSet<String>,
}

impl LexicalEngine {
    pub fn new() -> Self {
        Self {
            rules: Vec::new(),
            compiled_regexes: Vec::new(),
            raw_words: HashSet::new(),
        }
    }

    /// Basic normalization: NFKC, lowercase, katakana -> hiragana, kanji replacement
    pub fn base_normalize(text: &str) -> String {
        let nfkc: String = text.nfkc().collect();
        let lower = nfkc.to_lowercase();
        
        let mut res = String::with_capacity(lower.len());
        for c in lower.chars() {
            let u = c as u32;
            // Katakana (0x30A1 - 0x30F6) -> Hiragana (0x3041 - 0x3096)
            if (0x30A1..=0x30F6).contains(&u) {
                if let Some(hira) = char::from_u32(u - 0x60) {
                    res.push(hira);
                    continue;
                }
            }
            res.push(c);
        }

        // Japanese Kanji substitutions
        let replacements = [
            ("死ね", "しね"),
            ("死", "し"),
            ("糞", "くそ"),
            ("馬鹿", "ばか"),
            ("貴様", "きさま"),
            ("変態", "へんたい"),
        ];

        let mut final_text = res;
        for (kanji, hira) in replacements {
            if final_text.contains(kanji) {
                final_text = final_text.replace(kanji, hira);
            }
        }

        final_text
    }

    /// Canonical Cyrillic normalization: maps Latin leet and homoglyphs to Cyrillic
    pub fn to_cyrillic_canonical(text: &str) -> String {
        let base = Self::base_normalize(text);
        let mut out = String::with_capacity(base.len());

        for c in base.chars() {
            let cyr = match c {
                'a' => 'а',
                'b' | '6' => 'б',
                'v' | 'w' | '8' => 'в',
                'g' => 'г',
                'd' => 'д',
                'e' | '3' | 'ё' => 'е',
                'z' => 'з',
                'i' | '1' | 'j' | 'u' => 'и',
                'k' => 'к',
                'l' => 'л',
                'm' => 'м',
                'n' | 'h' => 'х',
                'o' | '0' => 'о',
                'p' => 'р',
                'r' => 'р',
                's' | '5' | 'c' => 'с',
                't' | '7' => 'т',
                'y' => 'у',
                'f' => 'ф',
                'x' => 'х',
                _ => c,
            };
            out.push(cyr);
        }
        out
    }

    /// Canonical Latin normalization: maps Cyrillic homoglyphs and leet to Latin
    pub fn to_latin_canonical(text: &str) -> String {
        let base = Self::base_normalize(text);
        let mut out = String::with_capacity(base.len());

        for c in base.chars() {
            let lat = match c {
                '0' => 'o',
                '1' => 'i',
                '3' => 'e',
                '4' => 'a',
                '5' => 's',
                '7' => 't',
                'а' => 'a',
                'в' => 'b',
                'с' => 'c',
                'е' => 'e',
                'н' => 'h',
                'к' => 'k',
                'м' => 'm',
                'о' => 'o',
                'р' => 'p',
                'т' => 't',
                'х' => 'x',
                'у' => 'y',
                _ => c,
            };
            out.push(lat);
        }
        out
    }

    /// Canonical Japanese normalization: Romaji to Hiragana
    pub fn to_japanese_canonical(text: &str) -> String {
        let base = Self::base_normalize(text);
        let romaji_pairs = [
            ("shine", "しね"),
            ("kuso", "くそ"),
            ("baka", "ばか"),
            ("kutabare", "くたばれ"),
            ("kisama", "きさま"),
            ("temee", "てめえ"),
            ("yarou", "やろう"),
            ("chikusho", "ちくしょう"),
        ];

        let mut out = base;
        for (rom, hira) in romaji_pairs {
            if out.contains(rom) {
                out = out.replace(rom, hira);
            }
        }
        out
    }

    pub fn add_rule(&mut self, pattern: &str, category: &str, severity: &str) {
        let pattern = pattern.trim();
        if pattern.is_empty() || pattern.starts_with('#') {
            return;
        }

        let raw_pattern = pattern.to_string();
        let is_regex;
        let regex_str: String;

        if pattern.starts_with('/') && pattern.ends_with('/') && pattern.len() > 2 {
            is_regex = true;
            regex_str = pattern[1..pattern.len() - 1].to_string();
        } else {
            is_regex = false;
            let norm_pattern = Self::base_normalize(pattern);
            let mut escaped = String::new();

            for c in norm_pattern.chars() {
                match c {
                    '*' => escaped.push_str(r"[\w\s\.\-_]*?"),
                    '[' | ']' | '(' | ')' | '{' | '}' | '+' | '?' | '^' | '$' | '|' | '\\' | '.' => {
                        escaped.push('\\');
                        escaped.push(c);
                    }
                    _ => escaped.push(c),
                }
            }

            let has_cjk = norm_pattern.chars().any(|c| {
                let u = c as u32;
                (0x4e00..=0x9fff).contains(&u) || (0x3040..=0x30ff).contains(&u)
            });

            if has_cjk {
                regex_str = escaped;
            } else {
                regex_str = format!(r"\b{}\b", escaped);
            }
        }

        match Regex::new(&format!("(?i){}", regex_str)) {
            Ok(re) => {
                self.compiled_regexes.push((
                    re,
                    raw_pattern.clone(),
                    category.to_string(),
                    severity.to_string(),
                ));
                self.raw_words.insert(raw_pattern.clone());
                self.rules.push(RuleEntry {
                    pattern: raw_pattern,
                    category: category.to_string(),
                    severity: severity.to_string(),
                    is_regex,
                });
            }
            Err(e) => {
                eprintln!("[LexicalEngine] Regex error for pattern '{}': {}", pattern, e);
            }
        }
    }

    pub fn load_words_from_text(&mut self, text: &str) {
        let mut current_category = "custom".to_string();

        for line in text.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }
            if trimmed.starts_with('[') && trimmed.ends_with(']') {
                current_category = trimmed[1..trimmed.len() - 1].trim().to_lowercase();
                continue;
            }
            self.add_rule(trimmed, &current_category, "high");
        }
    }

    pub fn export_words_to_text(&self) -> String {
        let mut lines = vec![
            "# blewred Stop-words Dictionary (Rust Native Engine)".to_string(),
            "# One word or pattern per line".to_string(),
            "".to_string(),
        ];
        let mut current_cat: Option<String> = None;

        for r in &self.rules {
            if Some(&r.category) != current_cat.as_ref() {
                lines.push(format!("\n[{}]", r.category));
                current_cat = Some(r.category.clone());
            }
            lines.push(r.pattern.clone());
        }

        lines.join("\n")
    }

    pub fn clear(&mut self) {
        self.rules.clear();
        self.compiled_regexes.clear();
        self.raw_words.clear();
    }

    pub fn check_text(&self, text: &str) -> Vec<MatchResult> {
        if text.is_empty() {
            return Vec::new();
        }

        let mut results = Vec::new();
        let mut seen_spans = HashSet::new();

        let variants = [
            text.to_string(),
            Self::base_normalize(text),
            Self::to_cyrillic_canonical(text),
            Self::to_latin_canonical(text),
            Self::to_japanese_canonical(text),
        ];

        for variant in &variants {
            for (re, raw_pat, category, severity) in &self.compiled_regexes {
                for m in re.find_iter(variant) {
                    let span = (m.start(), m.end());
                    if seen_spans.contains(&span) {
                        continue;
                    }
                    seen_spans.insert(span);

                    results.push(MatchResult {
                        matched: true,
                        word: m.as_str().to_string(),
                        rule: raw_pat.clone(),
                        category: category.clone(),
                        start_char: m.start(),
                        end_char: m.end(),
                        severity: severity.clone(),
                    });
                }
            }
        }

        results
    }

    pub fn is_banned(&self, text: &str) -> bool {
        !self.check_text(text).is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_multilingual_compliance() {
        let mut engine = LexicalEngine::new();
        engine.add_rule("пидор", "hate_speech", "high");
        engine.add_rule("bitch", "profanity", "high");
        engine.add_rule("死ね", "japanese_ban", "high");
        engine.add_rule("くたばれ", "japanese_ban", "high");

        // 1. Russian with leetspeak & homoglyphs
        let res_ru = engine.check_text("Ты просто п1dор!");
        assert!(!res_ru.is_empty(), "RU homoglyph failed");
        assert_eq!(res_ru[0].rule, "пидор");

        // 2. English with leet
        let res_en = engine.check_text("You are a b1tch now");
        assert!(!res_en.is_empty(), "EN leet failed");
        assert_eq!(res_en[0].rule, "bitch");

        // 3. Japanese Kanji
        let res_ja_kanji = engine.check_text("お前、早く死ねよ！");
        assert!(!res_ja_kanji.is_empty(), "JA Kanji failed");

        // 4. Japanese Katakana
        let res_ja_kata = engine.check_text("あいつ、クタバレ！");
        assert!(!res_ja_kata.is_empty(), "JA Katakana failed");
    }
}

