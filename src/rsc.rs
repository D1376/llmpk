use std::sync::LazyLock;
use std::time::Duration;

use anyhow::{anyhow, Context, Result};
use regex::Regex;
use reqwest::blocking::Client;

const UA: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) \
                  AppleWebKit/537.36 (KHTML, like Gecko) \
                  Chrome/124.0.0.0 Safari/537.36";

static CLIENT: LazyLock<Client> = LazyLock::new(|| {
    Client::builder()
        .user_agent(UA)
        .timeout(Duration::from_secs(20))
        .build()
        .expect("building reqwest client")
});

pub fn fetch_html(url: &str) -> Result<String> {
    CLIENT
        .get(url)
        .send()
        .with_context(|| format!("requesting {url}"))?
        .error_for_status()
        .with_context(|| format!("non-2xx from {url}"))?
        .text()
        .context("reading response body")
}

static RSC_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"self\.__next_f\.push\(\[1,"((?:\\.|[^"\\])*)"\]\)"#).expect("compiling RSC regex")
});

pub fn extract_stream(html: &str) -> Result<String> {
    let mut joined = String::new();
    for cap in RSC_RE.captures_iter(html) {
        decode_into(&cap[1], &mut joined);
    }
    if joined.is_empty() {
        return Err(anyhow!(
            "could not find leaderboard data — the site may have changed its structure"
        ));
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
                    // Handle UTF-16 surrogate pairs
                    if (0xD800..=0xDBFF).contains(&n) {
                        // High surrogate — look for \u low surrogate
                        if chars.next() == Some('\\') && chars.next() == Some('u') {
                            let low_hex: String = chars.by_ref().take(4).collect();
                            if let Ok(low) = u32::from_str_radix(&low_hex, 16) {
                                if (0xDC00..=0xDFFF).contains(&low) {
                                    let cp = 0x10000 + (n - 0xD800) * 0x400 + (low - 0xDC00);
                                    if let Some(ch) = char::from_u32(cp) {
                                        out.push(ch);
                                        continue;
                                    }
                                }
                            }
                        }
                        out.push('\u{FFFD}');
                        continue;
                    }
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

/// Fetch HTML using a fresh client per call (old behavior).
#[cfg(test)]
pub fn fetch_html_new_client(url: &str) -> Result<String> {
    Client::builder()
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

/// Find the first balanced `[...]` array following the literal `"<key>":` in the stream.
pub fn first_array_after<'a>(stream: &'a str, key: &str) -> Option<&'a str> {
    let needle = format!("\"{key}\":");
    let i = stream.find(&needle)?;
    let bytes = stream.as_bytes();
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decode_basic_escapes() {
        let mut out = String::new();
        decode_into(r#"hello\nworld\t!"#, &mut out);
        assert_eq!(out, "hello\nworld\t!");
    }

    #[test]
    fn decode_quote_and_backslash() {
        let mut out = String::new();
        decode_into(r#"say \"hi\" and \\"#, &mut out);
        assert_eq!(out, r#"say "hi" and \"#);
    }

    #[test]
    fn decode_unicode_bmp() {
        let mut out = String::new();
        decode_into(r#"Aé"#, &mut out);
        assert_eq!(out, "Aé");
    }

    #[test]
    fn decode_utf16_surrogate_pair() {
        let mut out = String::new();
        // U+1F600 = 😀
        decode_into(r#"😀"#, &mut out);
        assert_eq!(out, "😀");
    }

    #[test]
    fn decode_high_surrogate_without_low() {
        let mut out = String::new();
        decode_into(r#"\uD800"#, &mut out);
        assert_eq!(out, "\u{FFFD}");
    }

    #[test]
    fn decode_invalid_hex() {
        let mut out = String::new();
        decode_into(r#"\uZZZZ"#, &mut out);
        assert_eq!(out, "\u{FFFD}");
    }

    #[test]
    fn decode_trailing_backslash() {
        let mut out = String::new();
        decode_into("hello\\", &mut out);
        assert_eq!(out, "hello");
    }

    #[test]
    fn innermost_finds_nested_object() {
        let stream = r#"{"outer":{"inner":{"target":1}}}"#;
        let results = innermost_objects_with(stream, "\"target\":");
        assert_eq!(results.len(), 1);
        assert!(results[0].contains("\"target\":1"));
    }

    #[test]
    fn innermost_skips_containing_if_inner_exists() {
        let stream = r#"{"a":{"x":1},"b":{"x":2}}"#;
        let results = innermost_objects_with(stream, "\"x\":");
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn innermost_handles_strings_with_braces() {
        let stream = r#"{"a":"{not a brace}","b":1}"#;
        let results = innermost_objects_with(stream, "\"b\":");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0], "{\"a\":\"{not a brace}\",\"b\":1}");
    }

    #[test]
    fn innermost_no_match() {
        let stream = r#"{"a":1}"#;
        let results = innermost_objects_with(stream, "\"missing\":");
        assert!(results.is_empty());
    }

    #[test]
    fn first_array_finds_key() {
        let stream = r#"garbage"rows":[{"id":1},{"id":2}]"#;
        let result = first_array_after(stream, "rows");
        assert_eq!(result, Some(r#"[{"id":1},{"id":2}]"#));
    }

    #[test]
    fn first_array_skips_whitespace() {
        let stream = r#"key: "rows":   [1, 2, 3] more"#;
        let result = first_array_after(stream, "rows");
        assert_eq!(result, Some("[1, 2, 3]"));
    }

    #[test]
    fn first_array_missing_key() {
        let stream = r#"{"data":[1]}"#;
        let result = first_array_after(stream, "rows");
        assert_eq!(result, None);
    }

    #[test]
    fn first_array_no_bracket_after_key() {
        let stream = r#"key: "rows": null"#;
        let result = first_array_after(stream, "rows");
        assert_eq!(result, None);
    }

    #[test]
    fn first_array_nested_arrays() {
        let stream = r#"data: "rows": [[1,2],[3,4]]"#;
        let result = first_array_after(stream, "rows");
        assert_eq!(result, Some("[[1,2],[3,4]]"));
    }

    #[test]
    fn extract_stream_no_rsc_chunks() {
        let result = extract_stream("<html>no data here</html>");
        assert!(result.is_err());
    }

    #[test]
    fn extract_stream_decodes_chunks() {
        let html = r#"self.__next_f.push([1,"hello\nworld"])"#;
        let result = extract_stream(html).unwrap();
        assert_eq!(result, "hello\nworld");
    }

    const AA_URL: &str = "https://artificialanalysis.ai/";

    /// Benchmark: fresh client per request (old) vs shared client (new).
    /// Run with: LLMPK_BENCH=1 cargo test bench_fetch -- --nocapture
    #[test]
    fn bench_fetch() {
        if std::env::var("LLMPK_BENCH").is_err() {
            return;
        }

        let urls = [AA_URL];
        let rounds = 3;

        println!("\n=== Fetch Benchmark ({rounds} rounds per URL) ===\n");

        for url in &urls {
            // Warm up the shared client
            let _ = fetch_html(url);

            // --- Shared client (new) ---
            let mut shared_times = Vec::new();
            for _ in 0..rounds {
                let t = std::time::Instant::now();
                let _ = fetch_html(url);
                shared_times.push(t.elapsed());
            }

            // --- Fresh client per request (old) ---
            let mut fresh_times = Vec::new();
            for _ in 0..rounds {
                let t = std::time::Instant::now();
                let _ = fetch_html_new_client(url);
                fresh_times.push(t.elapsed());
            }

            let shared_avg: Duration = shared_times.iter().sum::<Duration>() / rounds;
            let fresh_avg: Duration = fresh_times.iter().sum::<Duration>() / rounds;
            let speedup = fresh_avg.as_secs_f64() / shared_avg.as_secs_f64();

            println!("URL: {url}");
            println!("  Fresh client : {fresh_times:?}  avg={fresh_avg:.0?}");
            println!("  Shared client: {shared_times:?}  avg={shared_avg:.0?}");
            println!("  Speedup      : {speedup:.2}x");
            println!();
        }
    }

    /// Benchmark: regex compiled per call (old) vs static regex (new).
    /// Run with: LLMPK_BENCH=1 cargo test bench_regex -- --nocapture
    #[test]
    #[allow(clippy::regex_creation_in_loops)]
    fn bench_regex() {
        if std::env::var("LLMPK_BENCH").is_err() {
            return;
        }

        let html = match std::env::var("LLMPK_HOMEPAGE_FIXTURE") {
            Ok(path) => std::fs::read_to_string(&path).expect("fixture read"),
            Err(_) => {
                println!("skipping bench_regex (set LLMPK_HOMEPAGE_FIXTURE)");
                return;
            }
        };

        let rounds = 10;

        // Static regex (new)
        let mut static_times = Vec::new();
        for _ in 0..rounds {
            let t = std::time::Instant::now();
            let _ = extract_stream(&html);
            static_times.push(t.elapsed());
        }

        // Regex compiled per call (old)
        let re = Regex::new(r#"self\.__next_f\.push\(\[1,"((?:\\.|[^"\\])*)"\]\)"#).unwrap();
        let mut compiled_times = Vec::new();
        for _ in 0..rounds {
            let t = std::time::Instant::now();
            let mut joined = String::new();
            for cap in re.captures_iter(&html) {
                decode_into(&cap[1], &mut joined);
            }
            let _ = joined;
            compiled_times.push(t.elapsed());
        }

        // Regex recompiled per call (old)
        let mut recompile_times = Vec::new();
        for _ in 0..rounds {
            let re = Regex::new(r#"self\.__next_f\.push\(\[1,"((?:\\.|[^"\\])*)"\]\)"#).unwrap();
            let t = std::time::Instant::now();
            let mut joined = String::new();
            for cap in re.captures_iter(&html) {
                decode_into(&cap[1], &mut joined);
            }
            let _ = joined;
            recompile_times.push(t.elapsed());
        }

        let static_avg: Duration = static_times.iter().sum::<Duration>() / rounds;
        let compiled_avg: Duration = compiled_times.iter().sum::<Duration>() / rounds;
        let recompile_avg: Duration = recompile_times.iter().sum::<Duration>() / rounds;

        println!("\n=== Regex Benchmark ({rounds} rounds) ===\n");
        println!("  Recompile each call: {recompile_times:?}  avg={recompile_avg:.0?}");
        println!("  Pre-compiled       : {compiled_times:?}  avg={compiled_avg:.0?}");
        println!("  Static (LazyLock)  : {static_times:?}  avg={static_avg:.0?}");
        println!(
            "  Compile overhead   : {:.2}x vs static",
            recompile_avg.as_secs_f64() / static_avg.as_secs_f64()
        );
    }
}
