fn hash_pairs<'a>(pairs: impl IntoIterator<Item = (&'a str, &'a str)>) -> String {
    let mut pairs: Vec<_> = pairs.into_iter().collect();
    pairs.sort_unstable_by_key(|(k, _)| *k);

    let mut buffer = Vec::with_capacity(pairs.len() * 64);

    for (k, v) in pairs {
        buffer.extend_from_slice(k.as_bytes());
        buffer.push(0);
        buffer.extend_from_slice(v.as_bytes());
        buffer.push(0);
    }

    let hash = xxhash_rust::xxh3::xxh3_64(&buffer);
    format!("{:016x}", hash)
}

#[test] fn embedded_nul_erases_pair_boundaries(){assert_eq!(hash_pairs([("a","b\0c\0d")]),hash_pairs([("a","b"),("c","d")]));}
#[test] fn order_is_canonical(){assert_eq!(hash_pairs([("a","b"),("c","d")]),hash_pairs([("c","d"),("a","b")]));}
#[test] fn ordinary_distinct_values_differ(){assert_ne!(hash_pairs([("a","b")]),hash_pairs([("a","c")]));}
