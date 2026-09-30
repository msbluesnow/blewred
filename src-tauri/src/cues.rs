use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum CueStatus {
    Pending,
    Approaching,
    ActiveCensor,
    BlockedByUser,
    Completed,
    Dismissed,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ScheduledCue {
    pub id: String,
    pub start_sec: f64,
    pub end_sec: f64,
    pub duration_sec: f64,
    pub formatted_range: String,
    pub reason: String,
    pub raw_text: String,
    pub status: CueStatus,
}

/// Formats seconds into human-readable MM:SS or HH:MM:SS format
pub fn format_seconds(sec: f64) -> String {
    let total = sec.max(0.0).round() as u64;
    let h = total / 3600;
    let m = (total % 3600) / 60;
    let s = total % 60;

    if h > 0 {
        format!("{:02}:{:02}:{:02}", h, m, s)
    } else {
        format!("{:02}:{:02}", m, s)
    }
}

/// Parses a single time token like "34:69", "1.34", "01:25:30", "45", "12,40"
/// Correctly normalizes seconds overflow (e.g. 34:69 -> 34*60 + 69 = 2109s).
pub fn parse_time_token(token: &str) -> Option<(f64, usize)> {
    let clean: String = token
        .trim()
        .trim_matches(|c: char| !c.is_ascii_digit() && c != ':' && c != '.' && c != ',')
        .replace(',', ".");

    if clean.is_empty() {
        return None;
    }

    // Check if token contains ':' or '.'
    let parts: Vec<&str> = if clean.contains(':') {
        clean.split(':').collect()
    } else if clean.contains('.') {
        clean.split('.').collect()
    } else {
        vec![&clean]
    };

    let part_count = parts.len();
    match part_count {
        1 => {
            // Raw seconds (e.g. "45")
            let s: f64 = parts[0].parse().ok()?;
            Some((s, 1))
        }
        2 => {
            // Minutes:Seconds or Hours:Minutes (e.g. "34:69" or "1.34")
            let p0: f64 = parts[0].parse().ok()?;
            let p1_str = if parts[1].len() == 3 && parts[1].ends_with('1') {
                &parts[1][..2]
            } else {
                parts[1]
            };
            let p1: f64 = p1_str.parse().ok()?;
            let total = p0 * 60.0 + p1;
            Some((total, 2))
        }
        3 => {
            // Hours:Minutes:Seconds (e.g. "01:25:30")
            let h: f64 = parts[0].parse().ok()?;
            let m: f64 = parts[1].parse().ok()?;
            // Trim OCR confusion where a closing parenthesis/bracket was decoded as a trailing '1' (e.g. "01:34:001")
            let s_str = if parts[2].len() == 3 && parts[2].ends_with('1') {
                &parts[2][..2]
            } else {
                parts[2]
            };
            let s: f64 = s_str.parse().ok()?;
            let total = h * 3600.0 + m * 60.0 + s;
            Some((total, 3))
        }
        _ => None,
    }
}

/// Preprocesses and cleans up OCR and human-input whitespace in text:
/// - Strips non-standard unicode whitespace (NBSP, zero-width spaces, tabs).
/// - Removes spaces around colons (e.g. "12 : 30" -> "12:30", "01 : 25 : 30" -> "01:25:30").
/// - Removes spaces around dots/commas between digits (e.g. "12 . 30" -> "12.30", "1 . 34" -> "1.34").
/// - Removes erroneous spaces between digits adjacent to colons/dots (e.g. "1 2:30" -> "12:30", "12:3 0" -> "12:30").
/// - Removes spaces inside brackets around numbers: "[ 12:30 ]" -> "[12:30]".
/// - Standardizes range separators to clean " - ".
pub fn clean_cues_whitespace(raw_text: &str) -> String {
    let mut result_lines = Vec::new();

    for line in raw_text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        // 1. Replace non-standard whitespace characters (NBSP, tabs, zero-width) with standard space
        let s_init: String = trimmed
            .chars()
            .map(|c| if c == '\u{00A0}' || c == '\t' || c == '\u{200B}' || c == '\u{FEFF}' { ' ' } else { c })
            .collect();

        // 2. Remove spaces around colons when adjacent to digits (e.g. "12 : 30" -> "12:30", "12 :30" -> "12:30", "12: 30" -> "12:30")
        let mut cleaned_colons = String::with_capacity(s_init.len());
        let chars: Vec<char> = s_init.chars().collect();
        let mut i = 0;
        while i < chars.len() {
            if chars[i] == ':' {
                let mut prev_idx = cleaned_colons.len();
                while prev_idx > 0 && cleaned_colons.as_bytes()[prev_idx - 1] == b' ' {
                    prev_idx -= 1;
                }
                let prev_is_digit = prev_idx > 0 && (cleaned_colons.as_bytes()[prev_idx - 1] as char).is_ascii_digit();

                let mut next_idx = i + 1;
                while next_idx < chars.len() && chars[next_idx] == ' ' {
                    next_idx += 1;
                }
                let next_is_digit = next_idx < chars.len() && chars[next_idx].is_ascii_digit();

                if prev_is_digit || next_is_digit {
                    if prev_is_digit {
                        cleaned_colons.truncate(prev_idx);
                    }
                    cleaned_colons.push(':');
                    if next_is_digit {
                        i = next_idx;
                        continue;
                    }
                }
            }
            cleaned_colons.push(chars[i]);
            i += 1;
        }
        let mut s = cleaned_colons;

        // 3. Remove spaces around dots or commas between digits (e.g. "12 . 30" -> "12.30", "1 . 34" -> "1.34")
        let mut cleaned_dots = String::with_capacity(s.len());
        let d_chars: Vec<char> = s.chars().collect();
        let mut j = 0;
        while j < d_chars.len() {
            if (d_chars[j] == '.' || d_chars[j] == ',') && j > 0 {
                let mut prev_idx = cleaned_dots.len();
                while prev_idx > 0 && cleaned_dots.as_bytes()[prev_idx - 1] == b' ' {
                    prev_idx -= 1;
                }
                let prev_is_digit = prev_idx > 0 && (cleaned_dots.as_bytes()[prev_idx - 1] as char).is_ascii_digit();

                let mut next_idx = j + 1;
                while next_idx < d_chars.len() && d_chars[next_idx] == ' ' {
                    next_idx += 1;
                }
                let next_is_digit = next_idx < d_chars.len() && d_chars[next_idx].is_ascii_digit();

                if prev_is_digit && next_is_digit {
                    cleaned_dots.truncate(prev_idx);
                    cleaned_dots.push('.');
                    j = next_idx;
                    continue;
                }
            }
            cleaned_dots.push(d_chars[j]);
            j += 1;
        }
        s = cleaned_dots;

        // 4. Remove spaces inside brackets around numbers: "[ 12:30 ]" -> "[12:30]"
        while s.contains("[ ") || s.contains(" ]") || s.contains("( ") || s.contains(" )") {
            s = s.replace("[ ", "[").replace(" ]", "]").replace("( ", "(").replace(" )", ")");
        }

        // 5. Remove accidental spaces between digits when part of a time token (e.g. "1 2:30" -> "12:30", "12:3 0" -> "12:30")
        let mut digit_compact = String::with_capacity(s.len());
        let s_chars: Vec<char> = s.chars().collect();
        let mut k = 0;
        while k < s_chars.len() {
            if s_chars[k] == ' ' && k > 0 && k + 1 < s_chars.len() {
                let prev_c = s_chars[k - 1];
                let next_c = s_chars[k + 1];
                if prev_c.is_ascii_digit() && next_c.is_ascii_digit() {
                    let has_near_sep_before = (k >= 2 && s_chars[k - 2] == ':') || (k >= 3 && s_chars[k - 3] == ':')
                        || (k >= 2 && s_chars[k - 2] == '.') || (k >= 3 && s_chars[k - 3] == '.');
                    let has_near_sep_after = (k + 2 < s_chars.len() && s_chars[k + 2] == ':') || (k + 3 < s_chars.len() && s_chars[k + 3] == ':')
                        || (k + 2 < s_chars.len() && s_chars[k + 2] == '.') || (k + 3 < s_chars.len() && s_chars[k + 3] == '.');
                    if has_near_sep_before || has_near_sep_after {
                        k += 1;
                        continue;
                    }
                }
            }
            digit_compact.push(s_chars[k]);
            k += 1;
        }
        s = digit_compact;

        // 6. Standardize range separators to clean " - "
        s = s.replace('—', "-").replace('–', "-").replace('~', "-");
        let mut range_clean = String::with_capacity(s.len() + 4);
        let r_chars: Vec<char> = s.chars().collect();
        let mut m = 0;
        while m < r_chars.len() {
            if r_chars[m] == '-' {
                let prev_has_time_char = m > 0 && (r_chars[m - 1].is_ascii_digit() || r_chars[m - 1] == ' ' || r_chars[m - 1] == ']' || r_chars[m - 1] == ')');
                let next_has_time_char = m + 1 < r_chars.len() && (r_chars[m + 1].is_ascii_digit() || r_chars[m + 1] == ' ' || r_chars[m + 1] == '[' || r_chars[m + 1] == '(');
                if prev_has_time_char && next_has_time_char {
                    while range_clean.ends_with(' ') {
                        range_clean.pop();
                    }
                    range_clean.push_str(" - ");
                    m += 1;
                    while m < r_chars.len() && r_chars[m] == ' ' {
                        m += 1;
                    }
                    continue;
                }
            }
            range_clean.push(r_chars[m]);
            m += 1;
        }
        s = range_clean;

        // 7. Collapse multiple spaces
        while s.contains("  ") {
            s = s.replace("  ", " ");
        }

        result_lines.push(s.trim().to_string());
    }

    result_lines.join("\n")
}

/// Smart Fuzzy Parser for Timestamps
/// Handles diverse human & OCR inputs:
/// - "34:69 - 1.34 nudity" -> Start: 35:09 (2109s), End: 01:34:00 (5640s)
/// - "12:40 - 13:10 18+" -> Start: 12:40, End: 13:10
/// - "from 05.10 to 05.40 banned word" -> Start: 05:10, End: 05:40
/// - "[01:15:20] jump scare" -> Start: 01:15:20, End: 01:15:30 (+10s default)
pub fn parse_fuzzy_cues(raw_text: &str) -> Vec<ScheduledCue> {
    let preprocessed = clean_cues_whitespace(raw_text);
    let mut cues = Vec::new();
    let mut cue_idx = 1;

    for line in preprocessed.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        // Replace common Russian prepositions & range symbols with a canonical separator " - "
        let mut normalized = trimmed
            .replace("—", " - ")
            .replace("–", " - ")
            .replace("->", " - ")
            .replace("..", " - ")
            .replace(" to ", " - ").replace(" from ", " ")
            .replace(" до ", " - ")
            .replace(" по ", " - ")
            .replace(" до: ", " - ")
            .replace(" по: ", " - ")
            .replace(" с ", " ")
            .replace('~', " - ");

        if let Some(rest) = normalized.strip_prefix("from ").or_else(|| normalized.strip_prefix("From ")) {
            normalized = rest.to_string();
        } else if let Some(rest) = normalized.strip_prefix("с ").or_else(|| normalized.strip_prefix("С ")) {
            normalized = rest.to_string();
        }

        // Split by range delimiter " - " or "-"
        let parts: Vec<&str> = if normalized.contains(" - ") {
            normalized.splitn(2, " - ").collect()
        } else if let Some(dash_idx) = find_range_dash(&normalized) {
            vec![&normalized[..dash_idx], &normalized[dash_idx + 1..]]
        } else {
            vec![&normalized]
        };

        if parts.len() >= 2 {
            let left = parts[0].trim();
            let right = parts[1].trim();

            let left_time = extract_first_time(left);
            let right_time = extract_first_time_and_reason(right);

            if let (Some(t_start), Some((t_end_raw, end_parts_count, reason_extracted))) = (left_time, right_time) {
                let start_sec = t_start.0;
                let mut end_sec = t_end_raw;

                // Contextual Forward-Time Vector Heuristic:
                // If end_sec <= start_sec, the user likely wrote "1.34" meaning 1 hour 34 minutes
                // after a start time like "34:69" (35m 09s = 2109s).
                if end_sec <= start_sec && end_parts_count == 2 {
                    // Extract parts from the right side to re-evaluate as Hours:Minutes
                    if let Some((h, m)) = extract_two_parts(right) {
                        let candidate_end = h * 3600.0 + m * 60.0;
                        if candidate_end > start_sec {
                            end_sec = candidate_end;
                        }
                    }
                }

                // If end_sec is still <= start_sec, assign a default 10s duration
                if end_sec <= start_sec {
                    end_sec = start_sec + 10.0;
                }

                let duration = end_sec - start_sec;
                let reason = if !reason_extracted.is_empty() {
                    reason_extracted
                } else {
                    "Scheduled Block".to_string()
                };

                let formatted_range = format!("{} — {}", format_seconds(start_sec), format_seconds(end_sec));

                cues.push(ScheduledCue {
                    id: format!("cue-{}", cue_idx),
                    start_sec,
                    end_sec,
                    duration_sec: duration,
                    formatted_range,
                    reason,
                    raw_text: trimmed.to_string(),
                    status: CueStatus::Pending,
                });
                cue_idx += 1;
            }
        } else {
            // Single timestamp on line (e.g. "[12:34] dangerous moment")
            if let Some((t_val, reason)) = extract_single_time_and_reason(trimmed) {
                let start_sec = t_val;
                let end_sec = start_sec + 10.0; // 10 seconds default window
                let duration = 10.0;
                let formatted_range = format!("{} — {}", format_seconds(start_sec), format_seconds(end_sec));

                cues.push(ScheduledCue {
                    id: format!("cue-{}", cue_idx),
                    start_sec,
                    end_sec,
                    duration_sec: duration,
                    formatted_range,
                    reason: if !reason.is_empty() { reason } else { "Scheduled Block".to_string() },
                    raw_text: trimmed.to_string(),
                    status: CueStatus::Pending,
                });
                cue_idx += 1;
            }
        }
    }

    // Sort chronologically by start_sec
    cues.sort_by(|a, b| a.start_sec.partial_cmp(&b.start_sec).unwrap_or(std::cmp::Ordering::Equal));
    cues
}

fn find_range_dash(s: &str) -> Option<usize> {
    for (i, c) in s.char_indices() {
        if c == '-' || c == '—' || c == '–' {
            let prev = s[..i].chars().last();
            let next = s[i + c.len_utf8()..].chars().next();
            // Must have digits or whitespace adjacent to be a range delimiter
            if prev.map(|p| p.is_ascii_digit() || p.is_whitespace()).unwrap_or(false)
                && next.map(|n| n.is_ascii_digit() || n.is_whitespace()).unwrap_or(false)
            {
                return Some(i);
            }
        }
    }
    None
}

fn extract_first_time(s: &str) -> Option<(f64, usize)> {
    for word in s.split_whitespace() {
        let clean = word.trim_matches(|c: char| !c.is_ascii_digit() && c != ':' && c != '.' && c != ',');
        if let Some(res) = parse_time_token(clean) {
            return Some(res);
        }
    }
    None
}

fn extract_first_time_and_reason(s: &str) -> Option<(f64, usize, String)> {
    let mut words = s.split_whitespace();
    let first_word = words.next()?;
    let clean = first_word.trim_matches(|c: char| !c.is_ascii_digit() && c != ':' && c != '.' && c != ',');
    let (t, count) = parse_time_token(clean)?;

    let reason_words: Vec<&str> = words.collect();
    let mut reason = reason_words.join(" ");
    reason = reason.trim_matches(|c: char| c == '[' || c == ']' || c == '(' || c == ')' || c == '-' || c == ':').trim().to_string();

    Some((t, count, reason))
}

fn extract_two_parts(s: &str) -> Option<(f64, f64)> {
    for word in s.split_whitespace() {
        let clean = word.trim_matches(|c: char| !c.is_ascii_digit() && c != ':' && c != '.' && c != ',');
        let parts: Vec<&str> = if clean.contains(':') {
            clean.split(':').collect()
        } else if clean.contains('.') {
            clean.split('.').collect()
        } else {
            continue;
        };

        if parts.len() == 2 {
            let p0: f64 = parts[0].parse().ok()?;
            let p1: f64 = parts[1].parse().ok()?;
            return Some((p0, p1));
        }
    }
    None
}

fn extract_single_time_and_reason(s: &str) -> Option<(f64, String)> {
    let words: Vec<&str> = s.split_whitespace().collect();
    let mut found_time = None;
    let mut reason_parts = Vec::new();

    for word in words {
        let clean = word.trim_matches(|c: char| !c.is_ascii_digit() && c != ':' && c != '.' && c != ',');
        if found_time.is_none() {
            if let Some((t, _)) = parse_time_token(clean) {
                found_time = Some(t);
                continue;
            }
        }
        let r_word = word.trim_matches(|c: char| c == '[' || c == ']' || c == '(' || c == ')');
        if !r_word.is_empty() {
            reason_parts.push(r_word);
        }
    }

    let t = found_time?;
    let reason = reason_parts.join(" ").trim().to_string();
    Some((t, reason))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_overflow_seconds_and_hours_heuristic() {
        // "34:69-1.34" user example
        let text = "34:69-1.34";
        let cues = parse_fuzzy_cues(text);
        assert_eq!(cues.len(), 1);
        // 34m 69s = 34 * 60 + 69 = 2109s = 35:09
        assert_eq!(cues[0].start_sec, 2109.0);
        // 1.34 after 2109s is 1h 34m = 5640s = 01:34:00
        assert_eq!(cues[0].end_sec, 5640.0);
        assert_eq!(cues[0].formatted_range, "35:09 — 01:34:00");
    }

    #[test]
    fn test_parse_standard_ranges_with_descriptions() {
        let text = r#"
        12:40 - 13:10 shower nudity
        from 05.10 to 05.40 banned word
        01:15:00 - 01:20:00 final scene
        "#;
        let cues = parse_fuzzy_cues(text);
        assert_eq!(cues.len(), 3);

        assert_eq!(cues[0].start_sec, 310.0); // 05:10
        assert_eq!(cues[0].end_sec, 340.0);   // 05:40
        assert!(cues[0].reason.contains("banned word"));

        assert_eq!(cues[1].start_sec, 760.0); // 12:40
        assert_eq!(cues[1].end_sec, 790.0);   // 13:10
        assert!(cues[1].reason.contains("shower nudity"));

        assert_eq!(cues[2].start_sec, 4500.0); // 01:15:00
        assert_eq!(cues[2].end_sec, 4800.0);   // 01:20:00
        assert!(cues[2].reason.contains("final scene"));
    }

    #[test]
    fn test_parse_irregular_separators_and_points() {
        let text = "from 02.15 to 03.45 Dangerous talk\n45:10 - 46:00";
        let cues = parse_fuzzy_cues(text);
        assert_eq!(cues.len(), 2);
        assert_eq!(cues[0].start_sec, 135.0); // 2m 15s
        assert_eq!(cues[0].end_sec, 225.0);   // 3m 45s
        assert_eq!(cues[1].start_sec, 2710.0); // 45m 10s
        assert_eq!(cues[1].end_sec, 2760.0);   // 46m 00s
    }

    #[test]
    fn test_parse_cues_with_ocr_spaces() {
        let text = r#"
        12 : 30 - 14 : 15 Dangerous scene
        34 : 69 - 1 . 34
        01 : 25 : 30 - 01 : 30 : 00
        [ 05 : 10 ] banned word
        1 2 : 4 0 - 1 3 : 1 0 Nudity
        "#;
        let cues = parse_fuzzy_cues(text);
        assert_eq!(cues.len(), 5);
        // 05:10 -> 310s (sorted chronologically)
        assert_eq!(cues[0].start_sec, 310.0);
        assert!(cues[0].reason.contains("banned word"));

        // 12:30 -> 750s, 14:15 -> 855s
        assert_eq!(cues[1].start_sec, 750.0);
        assert_eq!(cues[1].end_sec, 855.0);
        assert!(cues[1].reason.contains("Dangerous scene"));

        // 12:40 -> 760s, 13:10 -> 790s
        assert_eq!(cues[2].start_sec, 760.0);
        assert_eq!(cues[2].end_sec, 790.0);
        assert!(cues[2].reason.contains("Nudity"));

        // 34:69 -> 2109s, 1.34 -> 5640s
        assert_eq!(cues[3].start_sec, 2109.0);
        assert_eq!(cues[3].end_sec, 5640.0);

        // 01:25:30 -> 5130s, 01:30:00 -> 5400s
        assert_eq!(cues[4].start_sec, 5130.0);
        assert_eq!(cues[4].end_sec, 5400.0);
    }
}

