use std::collections::hash_map::RandomState;
use std::hash::{BuildHasher, Hasher};

/// Random seed generator
/// From <https://blog.orhun.dev/zero-deps-random-in-rust/>
fn random_seed() -> u64 {
    RandomState::new().build_hasher().finish()
}

/// Pseudorandom number generator from the "Xorshift RNGs" paper by George Marsaglia.
///
/// <https://github.com/rust-lang/rust/blob/1.55.0/library/core/src/slice/sort.rs#L559-L573>
/// From <https://blog.orhun.dev/zero-deps-random-in-rust/>
fn random_numbers() -> impl Iterator<Item = u32> {
    // let mut random = 92u32;
    let mut random = random_seed() as u32;
    std::iter::repeat_with(move || {
        random ^= random << 13;
        random ^= random >> 17;
        random ^= random << 5;
        random
    })
}

pub fn random_alpha_chars() -> impl Iterator<Item = char> {
    random_numbers()
        .map(|r| (r & 255) as u8 as char)
        .filter(|c| c.is_ascii_alphabetic())
}
