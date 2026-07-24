use std::hash::{DefaultHasher, Hash as _, Hasher as _};

pub async fn do_hash(s: &[u8]) -> u64 {
    let mut hasher = DefaultHasher::new();
    s.hash(&mut hasher);
    hasher.finish()
}
