use sha2::{Digest,Sha256};
const GUARD_STOPWORDS: &[&str] = &[
    "a", "an", "the", "of", "in", "on", "at", "to", "for", "from", "by", "with", "and", "or",
    "is", "are", "was", "were", "be", "been", "do", "does", "did", "can", "could", "would",
    "should", "will", "what", "which", "who", "whom", "whose", "when", "where", "why", "how",
    "i", "me", "my", "we", "our", "you", "your", "it", "its", "this", "that", "these", "those",
    "please", "tell", "give", "show", "way", "there", "s", "whats", "what's",
];

fn guard_tokens(text: &str) -> Vec<String> {
    text.split(|c: char| !c.is_alphanumeric() && c != '.')
        .map(|t| t.trim_matches('.').to_lowercase())
        .filter(|t| !t.is_empty())
        .collect()
}

/// `true` when two prompts agree on their specifics, the things embeddings
/// are known to blur: every number is the same, in the same order, and the
/// content words they share appear in the same order.
///
/// So "Convert 10 miles to kilometers" and "Convert 10 kilometers to miles"
/// disagree (shared words in a different order), as do "Is 17 prime?" and
/// "Is 71 prime?" (different numbers), while "What is the capital of
/// France?" and "Which city is the capital of France?" agree. It errs on the
/// side of "different": a wrong "different" costs one model call, a wrong
/// "same" returns someone else's answer.
pub fn same_specifics(a: &str, b: &str) -> bool {
    let (ta, tb) = (guard_tokens(a), guard_tokens(b));
    let is_number = |t: &String| t.chars().any(|c| c.is_ascii_digit());
    let numbers_a: Vec<&String> = ta.iter().filter(|t| is_number(t)).collect();
    let numbers_b: Vec<&String> = tb.iter().filter(|t| is_number(t)).collect();
    if numbers_a != numbers_b {
        return false;
    }
    let content = |tokens: &[String]| -> Vec<String> {
        tokens
            .iter()
            .filter(|t| !is_number(t) && !GUARD_STOPWORDS.contains(&t.as_str()))
            .cloned()
            .collect()
    };
    let (ca, cb) = (content(&ta), content(&tb));
    let shared_a: Vec<&String> = ca.iter().filter(|t| cb.contains(t)).collect();
    let shared_b: Vec<&String> = cb.iter().filter(|t| ca.contains(t)).collect();
    // Repeated words make the order ambiguous; compare first occurrences.
    let first = |v: Vec<&String>| -> Vec<String> {
        let mut seen: Vec<String> = Vec::new();
        for t in v {
            if !seen.contains(t) {
                seen.push(t.clone());
            }
        }
        seen
    };
    first(shared_a) == first(shared_b)
}

pub fn dedup_key(
    prompt: &str,
    metadata: &std::collections::HashMap<String, String>,
    session_id: Option<&str>,
) -> String {
    // Build a canonical byte sequence: optional session prefix + prompt + sorted metadata.
    let mut buf = String::new();

    if let Some(sid) = session_id {
        buf.push_str(sid);
        buf.push('\x00');
    }

    buf.push_str(prompt);

    // Include relevant metadata in key (sorted for determinism).
    let mut meta_keys: Vec<_> = metadata.keys().collect();
    meta_keys.sort();
    for key in meta_keys {
        if let Some(value) = metadata.get(key) {
            buf.push('\x00');
            buf.push_str(key);
            buf.push('=');
            buf.push_str(value);
        }
    }

    // SHA-256, truncated to 128 bits: deterministic across restarts and
    // processes, and wide enough that two different prompts never share a
    // cached answer by accident (a 64-bit hash makes that a real risk at scale).
    let digest = Sha256::digest(buf.as_bytes());
    let hash = hex::encode(&digest[..16]);
    match session_id {
        Some(_) => format!("dedup:s:{hash}"),
        None => format!("dedup:g:{hash}"),
    }
}

#[cfg(test)]mod owned_pure{use super::*;use std::collections::HashMap;
#[test]fn metadata_equals_alias(){let a=HashMap::from([("a=b".into(),"c".into())]);let b=HashMap::from([("a".into(),"b=c".into())]);assert_eq!(dedup_key("p",&a,None),dedup_key("p",&b,None));}
#[test]fn signed_number_guard_alias(){assert!(same_specifics("temperature -10 degrees","temperature 10 degrees"));}
#[test]fn no_shared_words_guard_accepts(){assert!(same_specifics("cats sleep","rocks fall"));}
#[test]fn different_number_guard_refuses(){assert!(!same_specifics("value 10","value 11"));}
}
