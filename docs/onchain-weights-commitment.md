# On-chain commitment to model weights: costs measured on zkIE's own parameters

Status: measurement study + proposal. Every number is tagged **[measured]** (run on this repo at `e897cca`, commands at the end) or
**[estimate]** (built from measured parts, stated error). Nothing here changes the prover.

## 1. The gap

Weights and activations are public, and the verifier holds the weights itself, so a verifier never needs a commitment to them. That is also
why nothing on-chain can say *which* weights a proof used: model parameters are not committed anywhere a contract can read. This note asks what
it costs to close that gap and which commitment shape makes it affordable.

## 2. What one WHIR opening costs today (single weight tensor, `Whir::new`, security 90, folding 5, rate 1/2)

| vars `n` | tensor size | proof bytes **[measured]** | commitment | Poseidon2-16 permutations the verifier runs **[measured]** | commit / open (CPU, 4 cores) |
|---|---|---|---|---|---|
| 10 | 2^10 | 17,886 | 72 B | 319 | 0.00 s / 0.18 s |
| 14 | 2^14 | 49,851 | 72 B | 845 | 0.01 s / 0.01 s |
| 18 | 2^18 | 83,272 | 72 B | 1,371 | 0.07 s / 0.12 s |
| 20 | 2^20 | 97,032 | 72 B | 1,579 | 0.27 s / 0.42 s |
| 22 | 2^22 | 117,821 | 72 B | 1,911 | 1.10 s / 1.80 s |
| 24 | 2^24 | 132,325 | 72 B | 2,133 | 4.24 s / 7.05 s |

Native verification takes 1 to 6 ms. The per-round query counts are 80, 16, 9 and 6 (final), with 2 out-of-domain samples per commitment
(`whir_sizes` prints them for every size). Digests are 8 Goldilocks elements (64 bytes), not 32.

**Parameter limit found while measuring.** `Whir::new(n)` cannot be constructed for `n >= 26`: `PowBitsExceedBudget { required: 17, budget: 16 }`
at n=26, `required: 25` at n=27, `required: 26` at n=28. A single global commitment over a GPT-2-sized weight set (2^27) and a `lm_head` padded to 2^26
therefore need either a larger PoW budget (paid by the prover at *open* time, once per opening) or several commitments of n <= 25.

## 3. Putting the verifier on the EVM

`contracts/poseidon2-goldilocks16/` is a Solidity port of zkIE's exact Poseidon2 instance (constants dumped from the Rust code by
`examples/p2_vectors.rs`). It is **bit-exact** against Rust on three permutation vectors, the sponge hash and the 2-to-1 compression.

| operation | gas **[measured]**, Foundry, optimizer on |
|---|---|
| one Poseidon2-Goldilocks-16 permutation | 52,271 (a first unoptimized port was 65,629) |
| leaf hash of 32 elements (4 permutations) | 214,553 |
| 2-to-1 compression | 53,261 |

So one n=22 opening costs 1,911 x 52,271 = **99.9 M gas** for Merkle hashing alone (n=24: 111.5 M), before calldata (about 115 KB) and field
arithmetic. That is above any single-transaction gas cap and above a whole L1 block. Squeezing the port further does not change the conclusion:
the cost is the width-16 permutation over a 64-bit field emulated in 256-bit words, not an implementation detail. A weight set split into one
commitment per shard would multiply this by the number of shards.

## 4. The same opening with an EVM-native hash

Using the same query and hash counts as the real n=22 verifier but a Keccak-256 Merkle tree with 32-byte digests:

| part | gas |
|---|---|
| 1,343 path compressions (64 B each) | 159,890 **[measured]** |
| 80 base-field leaf hashes (256 B) + 31 extension-field leaf hashes (512 B) | 17,288 **[measured]** |
| folding: ~111 queries x ~32 extension-field multiply-accumulates x 362 gas | ~1.3 M **[estimate]** (362 gas/op measured) |
| calldata, ~79 KB with 32-byte digests (digest bytes halve) | 1.26 M at 16 gas/byte, 3.2 M at the 40 gas/byte floor **[estimate]** |
| sumcheck, final polynomial, PoW check, Keccak transcript | ~0.3 M **[estimate, rough]** |
| **total per opening** | **~3 M to ~5 M** (about 30 times less than 99.9 M; hashing alone 565 times less) |

The estimate is a component sum, not an end-to-end verifier; treat it as +/- 30 %. It assumes the transcript challenger is also Keccak-based. A
Poseidon2 challenger would add its own permutations per observed commitment and sampled query.

## 5. Proposal (three phases, each independently useful)

1. **Registry + attestation (cheap, now).** A `WeightsRegistry`: `modelId -> (weightsRoot[], configHash)`. A verifier that has checked the
   zkIE proof off-chain, including the weights opening against the registered root, signs a verdict over
   `(weightsRoot, modelId, inputHash, outputHash, proofHash)`. On-chain cost is one storage write per model plus about 12 k gas per attested
   inference with a BIP-340 signature check (the `verdict-relay` pattern from our open PR `garyyang-finchip/task-token-standard#2`, which also
   binds chain id, contract, token id and submission id). This does not remove trust in the verifier; it makes *which weights* checkable and
   the verifier's judgment public.
2. **Weights-binding on-chain (3 to 5 M gas).** A second WHIR instance for weights only, with a Keccak Merkle tree and a Keccak challenger
   (weights are static, so prover cost is unaffected). The contract checks `eval(weights, r) == y` for a claimed `(r, y)` against the registered
   root. It does not re-derive `r` from the main transcript; that part stays attested until phase 3.
3. **Trustless inference verification.** Wrap the whole verification in a succinct proof checked on-chain. Out of scope here; the
   verifier-side re-run of the forward pass (zkIE#14) has to go first.

To keep the on-chain work to one opening per commitment, merge all weight claims into one before opening: `same_poly` for claims on one
polynomial and `open_batch_multi` for several.

## 6. Questions for the maintainers

- Is a second, Keccak-based PCS instance for static weights acceptable, or must everything stay on Poseidon2?
- PoW budget for large `n`: raise `DEFAULT_MAX_POW` for the weights instance (cost lands on the prover per opening), or cap commitments at n <= 25
  (13 shards for GPT-2 fit: a layer is about 7.1 M parameters, n=23)?
- One commitment per shard, or a few per model, given the n <= 25 limit?

## 7. Reproduce

```sh
# sizes, round parameters, proof bytes (add sizes that fit in RAM)
cargo run --release -p zkie-core --example whir_sizes -- 10 14 18 20 22 24
# verifier permutation counts: apply the instrumentation patch first, then run the same command
git apply docs/measurements/verifier-perm-counters.patch
# Poseidon2 constants and Rust vectors, then the Solidity port and its gas
cargo run --release -p zkie-core --example p2_vectors > contracts/poseidon2-goldilocks16/p2_vectors.json
cd contracts/poseidon2-goldilocks16 && python3 gen_p2g16.py && forge install foundry-rs/forge-std --no-git && forge test -vv
```
