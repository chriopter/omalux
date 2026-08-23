//! Explicit, IO-free inputs that affect deterministic rendering but are not
//! persisted develop parameters.

use std::fmt;

const GRAIN_SEED_DOMAIN: &[u8] = b"io.omacom.grainroom/grain-seed/v1\0";
const FNV_OFFSET_BASIS: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

/// A resolved deterministic grain seed.
///
/// The value is intentionally opaque. Production callers should construct a
/// render context from an already computed source-content digest; tests may
/// opt into a fixed seed explicitly.
#[derive(Clone, Copy, Eq, PartialEq)]
pub struct ResolvedGrainSeed(u64);

impl ResolvedGrainSeed {
    /// Constructs a fixed seed for crate-local deterministic tests.
    #[cfg(test)]
    pub(crate) const fn fixed_for_tests(value: u64) -> Self {
        Self(value)
    }

    pub(crate) const fn value(self) -> u64 {
        self.0
    }
}

impl fmt::Debug for ResolvedGrainSeed {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("ResolvedGrainSeed(..)")
    }
}

/// Opaque non-persisted inputs required by deterministic CPU rendering.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DevelopRenderContext {
    grain_seed: ResolvedGrainSeed,
}

impl DevelopRenderContext {
    /// Resolves grain identity from a trusted 32-byte source-content digest.
    ///
    /// Digest computation is deliberately outside this API: this constructor
    /// performs no path lookup, metadata access, global-default lookup, or IO.
    pub fn from_source_digest(source_digest: [u8; 32]) -> Self {
        let mut state = FNV_OFFSET_BASIS;
        for byte in GRAIN_SEED_DOMAIN.iter().chain(source_digest.iter()) {
            state ^= u64::from(*byte);
            state = state.wrapping_mul(FNV_PRIME);
        }
        // Stable final avalanche. The fixed domain keeps this derivation
        // independent from any future digest-derived render identities.
        state ^= state >> 30;
        state = state.wrapping_mul(0xbf58_476d_1ce4_e5b9);
        state ^= state >> 27;
        state = state.wrapping_mul(0x94d0_49bb_1331_11eb);
        state ^= state >> 31;
        Self {
            grain_seed: ResolvedGrainSeed(state),
        }
    }

    pub(crate) const fn grain_seed(self) -> ResolvedGrainSeed {
        self.grain_seed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn digest_resolution_is_stable_domain_separated_and_content_sensitive() {
        let zero = DevelopRenderContext::from_source_digest([0; 32]);
        let same = DevelopRenderContext::from_source_digest([0; 32]);
        let mut changed_digest = [0; 32];
        changed_digest[31] = 1;
        let changed = DevelopRenderContext::from_source_digest(changed_digest);
        assert_eq!(zero, same);
        assert_ne!(zero, changed);
        assert_eq!(zero.grain_seed().value(), 0x2340_b7de_3925_fb4b);
    }
}
