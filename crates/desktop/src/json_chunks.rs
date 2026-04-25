//! Brace-balanced JSON objects from a stream (same idea as `src/lib/stream/ndjson.ts`).

use serde_json::Value;

#[inline]
fn is_whitespace_char(c: char) -> bool {
    if (c as u32) <= 32 {
        return matches!(c as u8, 9 | 10 | 11 | 12 | 13 | 32);
    }
    matches!(
        c,
        '\u{a0}' | '\u{feff}' | '\u{1680}' | '\u{202f}' | '\u{205f}' | '\u{3000}'
            | '\u{2028}'
            | '\u{2029}'
    ) || ('\u{2000}'..='\u{200a}').contains(&c)
}

/// Incrementally parse JSON objects from `chunk`, concatenated after `prior_buffer`.
/// Returns `(tail_buffer, parsed_values)` where `tail_buffer` holds an incomplete object prefix if any.
pub fn feed_json_chunks(prior_buffer: &str, chunk: &str) -> (String, Vec<Value>) {
    let data = if prior_buffer.is_empty() {
        chunk.to_string()
    } else {
        format!("{prior_buffer}{chunk}")
    };
    let bytes = data.as_bytes();
    let mut messages = Vec::new();
    let mut i = 0usize;

    'outer: while i < bytes.len() {
        while i < bytes.len() {
            let ch = data[i..].chars().next().unwrap();
            if !is_whitespace_char(ch) {
                break;
            }
            i += ch.len_utf8();
        }
        if i >= bytes.len() {
            break;
        }
        if bytes[i] != b'{' {
            let ch = data[i..].chars().next().unwrap();
            i += ch.len_utf8();
            continue;
        }

        let start = i;
        let mut depth = 0i32;
        let mut j = i;
        while j < bytes.len() {
            let c = bytes[j];
            if c == b'{' {
                depth += 1;
            } else if c == b'}' {
                depth -= 1;
                if depth == 0 {
                    let raw = &data[start..=j];
                    match serde_json::from_str::<Value>(raw) {
                        Ok(v) => {
                            messages.push(v);
                            i = j + 1;
                            continue 'outer;
                        }
                        Err(_) => {
                            i = start + 1;
                            continue 'outer;
                        }
                    }
                }
            }
            j += 1;
        }

        if depth > 0 {
            return (data[start..].to_string(), messages);
        }
        break;
    }

    (String::new(), messages)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn two_objects() {
        let (buf, msgs) = feed_json_chunks("", r#"{"a":1}{"b":2}"#);
        assert!(buf.is_empty());
        assert_eq!(msgs.len(), 2);
    }

    #[test]
    fn split_chunk() {
        let (b1, m1) = feed_json_chunks("", r#"{"x":1"#);
        assert_eq!(b1, r#"{"x":1"#);
        assert!(m1.is_empty());
        let (b2, m2) = feed_json_chunks(&b1, r#"} {"y":2}"#);
        assert!(b2.is_empty());
        assert_eq!(m2.len(), 2);
    }
}
