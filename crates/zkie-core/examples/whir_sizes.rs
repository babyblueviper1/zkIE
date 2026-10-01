//! Measures the real WHIR proof size, round parameters and timings at zkIE's own parameters (security 90, folding 5, rate 1/2 start).
use p3_challenger::DuplexChallenger;
use p3_field::extension::BinomialExtensionField;
use p3_goldilocks::{Goldilocks, Poseidon2Goldilocks};
use p3_whir::parameters::{FoldingFactor, ProtocolParameters, SecurityAssumption, WhirConfig, DEFAULT_MAX_POW};
use zkie_core::pcs::whir::Whir;

type F = Goldilocks;
type EF = BinomialExtensionField<F, 2>;
type Perm = Poseidon2Goldilocks<16>;
type Ch = DuplexChallenger<F, Perm, 16, 8>;

fn config(n: usize) -> Result<WhirConfig<EF, F, Ch>, String> {
    let ff = FoldingFactor::Constant(5);
    let (rounds, _) = ff.compute_number_of_rounds(n).expect("schedule");
    let mut rates = Vec::new();
    let mut rate = 1;
    for r in 0..rounds {
        rate += ff.at_round(r) - 1;
        rates.push(rate);
    }
    let params = ProtocolParameters {
        security_level: 90,
        pow_bits: DEFAULT_MAX_POW,
        folding_factor: ff,
        soundness_type: SecurityAssumption::CapacityBound,
        starting_log_inv_rate: 1,
        round_log_inv_rates: rates,
    };
    WhirConfig::<EF, F, Ch>::new(n, params).map_err(|e| format!("{e:?}"))
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    // config-only sizes (no allocation): print for the model-scale weight polynomials
    for n in [10usize, 16, 20, 22, 24, 25, 26, 27, 28] {
        let c = match config(n) { Ok(c) => c, Err(e) => { println!("== n={n}: Whir::new CANNOT BUILD with zkIE parameters (security 90, pow 16): {e}"); continue; } };
        println!("== n={n} vars (2^{n} Goldilocks = {} MB)  commitment_ood={} starting_folding_pow={} final_queries={} final_sumcheck_rounds={}",
            (1u64 << n) * 8 / 1_000_000, c.commitment_ood_samples, c.starting_folding_pow_bits, c.final_queries, c.final_sumcheck_rounds);
        println!("   folding_schedule={:?}", c.folding_schedule);
        for (i, r) in c.round_parameters.iter().enumerate() {
            println!("   round {i}: queries={} ood={} domain=2^{} log_inv_rate={} folding={} num_vars_after={} pow={} fold_pow={}",
                r.num_queries, r.ood_samples, r.domain_size.trailing_zeros(), r.log_inv_rate, r.folding_factor, r.num_variables, r.pow_bits, r.folding_pow_bits);
        }
    }
    // real prove/verify + serialized size for sizes that fit in memory
    let real: Vec<usize> = args.iter().skip(1).filter_map(|a| a.parse().ok()).collect();
    for n in real {
        let whir = Whir::new(n);
        let evals: Vec<Goldilocks> = (0..(1u64 << n)).map(|i| Goldilocks::new((i.wrapping_mul(0x9E3779B97F4A7C15) >> 3) % 0xFFFF_FFFF_0000_0001)).collect();
        let t0 = std::time::Instant::now();
        let (commitment, pd, proto) = whir.commit(&evals);
        let tc = t0.elapsed().as_secs_f64();
        let point: Vec<Goldilocks> = (0..n as u64).map(|i| Goldilocks::new(1_000_003 * (i + 7))).collect();
        let t1 = std::time::Instant::now();
        let (proof, val) = whir.open(pd, &proto, &point);
        let to = t1.elapsed().as_secs_f64();
        let t2 = std::time::Instant::now();
        let v = whir.verify(&commitment, &proof, &proto, &point).expect("verify");
        let tv = t2.elapsed().as_secs_f64();
        assert_eq!(v, val);
        let proof_bytes = bincode::serialize(&proof).unwrap().len();
        let commit_bytes = bincode::serialize(&commitment).unwrap().len();
        println!("REAL n={n}: commit {tc:.2}s open {to:.2}s verify(native) {tv:.3}s | proof {proof_bytes} bytes ({:.1} KB) commitment {commit_bytes} bytes", proof_bytes as f64 / 1024.0);
    }
}
