/// Stable identifiers used by graph records and ingested snapshots.
pub fn record_key(input: &str) -> String {
    input.as_bytes().iter().map(|byte| format!("{byte:02x}")).collect()
}

pub fn content_hash(input: &str) -> u64 {
    input.as_bytes().iter().fold(0xcbf29ce484222325u64, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(0x100000001b3)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn record_keys_are_stable() {
        assert_eq!(record_key("Core-Router"), "436f72652d526f75746572");
    }

    #[test]
    fn content_hash_is_deterministic() {
        assert_eq!(content_hash("same"), content_hash("same"));
        assert_ne!(content_hash("same"), content_hash("different"));
    }
}
