use std::time::Duration;

use anyhow::{anyhow, Context, Result};
use regex::Regex;

const UA: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) \
                  AppleWebKit/537.36 (KHTML, like Gecko) \
                  Chrome/124.0.0.0 Safari/537.36";

pub fn fetch_html(url: &str) -> Result<String> {
    reqwest::blocking::Client::builder()
        .user_agent(UA)
        .timeout(Duration::from_secs(20))
        .build()?
        .get(url)
        .send()
        .with_context(|| format!("requesting {url}"))?
        .error_for_status()
        .with_context(|| format!("non-2xx from {url}"))?
        .text()
        .context("reading response body")
}

pub fn extract_stream(html: &str) -> Result<String> {
    let re = Regex::new(r#"self\.__next_f\.push\(\[1,"((?:\\.|[^"\\])*)"\]\)"#)?;
    let mut joined = String::new();
    for cap in re.captures_iter(html) {
        decode_into(&cap[1], &mut joined);
    }
    if joined.is_empty() {
        return Err(anyhow!("no __next_f.push chunks in HTML"));
    }
    Ok(joined)
}

fn decode_into(src: &str, out: &mut String) {
    let mut chars = src.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('n') => out.push('\n'),
            Some('r') => out.push('\r'),
            Some('t') => out.push('\t'),
            Some('"') => out.push('"'),
            Some('\'') => out.push('\''),
            Some('\\') => out.push('\\'),
            Some('/') => out.push('/'),
            Some('b') => out.push('\u{0008}'),
            Some('f') => out.push('\u{000c}'),
            Some('u') => {
                let hex: String = chars.by_ref().take(4).collect();
                if let Ok(n) = u32::from_str_radix(&hex, 16) {
                    if let Some(ch) = char::from_u32(n) {
                        out.push(ch);
                        continue;
                    }
                }
                out.push('\u{FFFD}');
            }
            Some(other) => out.push(other),
            None => break,
        }
    }
}

/// Return the smallest balanced `{...}` substrings that contain `needle`.
pub fn innermost_objects_with<'a>(stream: &'a str, needle: &str) -> Vec<&'a str> {
    let bytes = stream.as_bytes();
    let mut starts: Vec<usize> = Vec::new();
    let mut closed: Vec<(usize, usize)> = Vec::new();
    let mut in_str = false;
    let mut escape = false;

    for (i, &b) in bytes.iter().enumerate() {
        if in_str {
            if escape {
                escape = false;
            } else if b == b'\\' {
                escape = true;
            } else if b == b'"' {
                in_str = false;
            }
            continue;
        }
        match b {
            b'"' => in_str = true,
            b'{' => starts.push(i),
            b'}' => {
                if let Some(s) = starts.pop() {
                    closed.push((s, i));
                }
            }
            _ => {}
        }
    }

    let mut matches: Vec<(usize, usize)> = closed
        .into_iter()
        .filter(|(s, e)| stream[*s..=*e].contains(needle))
        .collect();
    matches.sort_by_key(|(s, e)| e - s);

    let mut chosen: Vec<(usize, usize)> = Vec::new();
    for (s, e) in matches {
        if chosen.iter().any(|(cs, ce)| *cs >= s && *ce <= e) {
            continue;
        }
        chosen.push((s, e));
    }
    chosen.sort_by_key(|(s, _)| *s);
    chosen.iter().map(|(s, e)| &stream[*s..=*e]).collect()
}

/// Find the first balanced `[...]` array following the literal `"<key>":` in the stream.
pub fn first_array_after<'a>(stream: &'a str, key: &str) -> Option<&'a str> {
    let needle = format!("\"{key}\":");
    let i = stream.find(&needle)?;
    let bytes = stream.as_bytes();
    // Locate the opening `[` after the colon.
    let mut start = i + needle.len();
    while start < bytes.len() && bytes[start].is_ascii_whitespace() {
        start += 1;
    }
    if start >= bytes.len() || bytes[start] != b'[' {
        return None;
    }

    let mut depth: i32 = 0;
    let mut in_str = false;
    let mut escape = false;
    for (j, &b) in bytes.iter().enumerate().skip(start) {
        if in_str {
            if escape {
                escape = false;
            } else if b == b'\\' {
                escape = true;
            } else if b == b'"' {
                in_str = false;
            }
            continue;
        }
        match b {
            b'"' => in_str = true,
            b'[' => depth += 1,
            b']' => {
                depth -= 1;
                if depth == 0 {
                    return Some(&stream[start..=j]);
                }
            }
            _ => {}
        }
    }
    None
}
