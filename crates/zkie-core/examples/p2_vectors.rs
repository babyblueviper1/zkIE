//! Dumps zkIE's exact Poseidon2-Goldilocks-16 constants and test vectors (permutation, sponge hash, 2-to-1 compress) as JSON.
use p3_field::{PrimeCharacteristicRing, PrimeField64};
use p3_goldilocks::{Goldilocks, Poseidon2Goldilocks, MATRIX_DIAG_16_GOLDILOCKS};
use p3_poseidon2::ExternalLayerConstants;
use p3_symmetric::{CryptographicHasher, PaddingFreeSponge, Permutation, PseudoCompressionFunction, TruncatedPermutation};
use rand::distr::{Distribution, StandardUniform};
use rand::rngs::SmallRng;
use rand::{RngExt, SeedableRng};

type Perm = Poseidon2Goldilocks<16>;
fn u(v: &Goldilocks) -> u64 { v.as_canonical_u64() }
fn arr(v: &[Goldilocks]) -> String { format!("[{}]", v.iter().map(|x| u(x).to_string()).collect::<Vec<_>>().join(",")) }
fn main() {
    let mut rng = SmallRng::seed_from_u64(1);
    let external = ExternalLayerConstants::<Goldilocks, 16>::new_from_rng(8, &mut rng);
    let internal: Vec<Goldilocks> = rng.sample_iter(StandardUniform).take(22).collect();
    let init: Vec<Vec<Goldilocks>> = external.get_initial_constants().iter().map(|r| r.to_vec()).collect();
    let term: Vec<Vec<Goldilocks>> = external.get_terminal_constants().iter().map(|r| r.to_vec()).collect();
    let perm = Perm::new_from_rng_128(&mut SmallRng::seed_from_u64(1));
    let mut vecs = Vec::new();
    let inputs: Vec<[Goldilocks; 16]> = vec![
        [Goldilocks::ZERO; 16],
        core::array::from_fn(|i| Goldilocks::new(i as u64)),
        core::array::from_fn(|i| Goldilocks::new(0xFFFF_FFFF_0000_0000u64 - i as u64)),
    ];
    for inp in inputs { let mut s = inp; perm.permute_mut(&mut s); vecs.push(format!("{{\"in\":{},\"out\":{}}}", arr(&inp), arr(&s))); }
    let hash = PaddingFreeSponge::<Perm, 16, 8, 8>::new(perm.clone());
    let comp = TruncatedPermutation::<Perm, 2, 8, 16>::new(perm.clone());
    let msg: Vec<Goldilocks> = (0..32u64).map(|i| Goldilocks::new(1000 + 7 * i)).collect();
    let h: [Goldilocks; 8] = hash.hash_slice(&msg);
    let d0: [Goldilocks; 8] = core::array::from_fn(|i| Goldilocks::new(5 + i as u64));
    let d1: [Goldilocks; 8] = core::array::from_fn(|i| Goldilocks::new(500 + i as u64));
    let c: [Goldilocks; 8] = comp.compress([d0, d1]);
    println!("{{");
    println!("\"rc_init\":[{}],", init.iter().map(|r| arr(r)).collect::<Vec<_>>().join(","));
    println!("\"rc_term\":[{}],", term.iter().map(|r| arr(r)).collect::<Vec<_>>().join(","));
    println!("\"rc_internal\":{},", arr(&internal));
    println!("\"diag\":{},", arr(&MATRIX_DIAG_16_GOLDILOCKS));
    println!("\"perm_vectors\":[{}],", vecs.join(","));
    println!("\"hash32\":{{\"msg\":{},\"out\":{}}},", arr(&msg), arr(&h));
    println!("\"compress\":{{\"d0\":{},\"d1\":{},\"out\":{}}}", arr(&d0), arr(&d1), arr(&c));
    println!("}}");
}
