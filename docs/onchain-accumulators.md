# On-chain weights commitment: do accumulators beat a Merkle tree? (follow-up to #15)

Question from the maintainer (Telegram, 2026-10-02): evaluate RSA and pairing-based accumulators for batch membership of the opened
`(index, weight)` pairs, as a cheaper alternative to per-element Merkle proofs. This note gives measured precompile costs, a component
estimate for each option against the Keccak baseline in `onchain-weights-commitment.md` section 4, and the one structural problem that decides it.
Measurements: `contracts/poseidon2-goldilocks16/test/AccumulatorCost.t.sol` (`forge test --match-contract AccumulatorCost -vv`, Foundry, cancun).

## 1. Measured EVM costs

| operation | gas | tag |
|---|---|---|
| BN254 `ecAdd` | 294 | measured |
| BN254 `ecMul` | 6,144 | measured |
| BN254 `ecPairing`, 2 pairs (one KZG opening check) | 113,144 | measured |
| BN254 `ecPairing`, 4 pairs | 181,144 | measured |
| `modexp`, 2048-bit modulus, 128-bit exponent (one Wesolowski PoE check) | 43,493 | measured |
| `modexp`, 2048-bit modulus, 256-bit exponent | 87,184 | measured |
| `modexp`, 3072-bit modulus, 128-bit exponent | 97,680 | measured |
| `modexp`, 2048-bit modulus, 2048-bit exponent | 698,853 | measured |
| `modexp`, 2048-bit modulus, 12,800-bit exponent (100 primes with no PoE) | 4,368,869 | measured |
| `modexp`, 128-bit modulus, 128-bit exponent (one Miller-Rabin round) | 344 | measured |

## 2. What an accumulator can and cannot save

From section 4 of the main note, the n=22 Keccak opening is about 3 to 5 M gas: hashing is only ~0.18 M (path and leaf hashes), folding
arithmetic ~1.3 M, calldata 1.3 to 3.2 M (about half of it the 32-byte path digests), the rest transcript and sumcheck.
Membership proofs touch the hashing and the path-digest calldata only. Replacing them entirely with a free membership check would save
roughly 0.18 M + 0.6 to 1.6 M, i.e. **at most about 15 to 40 % of the opening**. The folding checks stay, because they verify the WHIR
reduction itself and not membership. So a batch-membership accumulator, however cheap, cannot deliver the "drastic" reduction by itself.

## 3. RSA accumulator (batch membership with a Wesolowski proof of exponentiation)

Verifier work: a prime representative for each element, then one PoE (two 128-bit-exponent `modexp` calls at 2048 bits, ~87 k) and
`x mod l = prod(p_i) mod l` (a mulmod per element).
- Hash-to-prime on-chain: the prover supplies a nonce, the contract checks primality with Miller-Rabin. One round on a 128-bit candidate is 344 gas
  measured, so ~7 k per element at ~20 rounds **[estimate]**.
- If the accumulated elements are the 80 opened leaves (not the ~2,560 individual weights): 80 x ~7 k + ~87 k + transcript ~= **0.7 M [estimate]**,
  against about 0.2 M hashing + 0.6 to 1.6 M path calldata for Merkle paths. Net change is roughly zero to -1 M gas on a 3 to 5 M opening.
- If the elements are individual weights (2,560): ~18 M **[estimate]**, worse than the Merkle baseline.
- Assumptions that cost more than the gas: an unknown-order group needs a trusted RSA modulus (class groups have no EVM-priced verifier), 2048-bit gives
  ~112-bit security against the 128-bit Keccak baseline, and the commitment is a different object from the WHIR codeword the proof already opens.

**Verdict: not worth it.** It trades about the same gas for a trusted setup and weaker security.

## 4. Pairing-based (KZG-style) commitment

A KZG opening of the weight polynomial at one point is two pairings: **113 k gas measured**, plus ~6.1 k per extra `ecMul` for batching claims across layers.
For a model with 32 matmul claims batched by a random linear combination: ~0.11 M + 32 x 6.1 k + 0.3 k x ~64 ~= **0.3 to 0.4 M [estimate]**, independent of model size, with
~200 bytes of calldata. That is roughly 10 times below the Keccak opening and it removes the folding and path calldata, not only the hashing.

**But the premise "pairing accumulators are aligned with WHIR's algebra" does not hold for the field zkIE uses.** WHIR here runs over Goldilocks
(p = 2^64 - 2^32 + 1) with an extension field, and the sumcheck challenge `r` lives in that extension. KZG on a pairing-friendly curve commits over the
scalar field of that curve (BN254 Fr, 254 bits). An evaluation of the weight multilinear at a Goldilocks-extension point `r` is not a statement
over BN254 Fr, so the opening above cannot discharge the claim `eval(weights, r) = y` that the reduction ends with. There is no pairing-friendly curve whose
scalar field is Goldilocks. KZG is aligned with proof systems that already run over Fr (Plonk, Halo2-KZG); it is not aligned with WHIR over a 64-bit field.
Making it work needs one of:
1. a field-bridging consistency argument between a Goldilocks commitment and an Fr commitment of the same weights (a real research item, with its own
   prover cost, and I have not found a published construction I would trust at this size), or
2. running the weight-claim part of the protocol over Fr (changes the proof system, loses the small-field speed that makes WHIR attractive), or
3. a succinct wrapper (phase 3 of the main note): verify the WHIR proof inside a Groth16/Plonk circuit over BN254, keep the Poseidon2 weights root as a public
   input, pay ~0.25 to 0.3 M on-chain for any model size (a Groth16 check is 4 pairings = 181 k measured plus the input MSM). Prover cost of the wrapper is
   **not measured here**; the verifier-side cost to wrap is ~1,900 Poseidon2-16 permutations at n=22 per opening (measured in `verifier-perm-counters.patch`),
   and a Goldilocks permutation inside a BN254 circuit uses non-native arithmetic, so it is the quantity to measure first.

## 5. Recommendation

1. Keep the Keccak weights-only WHIR instance (phase 2 of the main note) as the shippable baseline: 3 to 5 M gas, no new assumptions, no new setup.
2. Do not build an RSA accumulator: gas is a wash and it adds a trusted modulus and a weaker security level.
3. Treat the pairing route as the long-term cheap path only if the field problem has an answer. The honest first step is to decide between option 1 (bridge),
   2 (Fr protocol) and 3 (wrapper) from section 4, and for option 3 to measure the in-circuit cost of one Poseidon2-Goldilocks permutation.
4. A cheap, certain improvement available now: a Merkle multi-proof (deduplicated shared siblings across the ~111 queries; the top ~7 levels are shared) cuts the
   path-digest bytes by roughly 30 % **[estimate]**, i.e. ~0.2 to 0.5 M calldata gas, for no new assumption.

Questions for the maintainers:
- Is option 2 (a Fr-native weight claim) or option 3 (a wrapper) closer to where zkIE is going? That decides whether a pairing commitment is on the table at all.
- Is a ~30 % calldata saving from a Merkle multi-proof worth doing before any of the above?
