// SPDX-License-Identifier: CC0-1.0
pragma solidity ^0.8.24;
import "forge-std/Test.sol";
// Gas of the EVM-native alternative for the weights tree: keccak256 Merkle (32-byte digests) with the SAME query/hash counts as the real zkIE verifier at n=22.
contract KeccakCost is Test {
    // measured verifier work for one n=22 opening: 1343 path compressions + leaf hashes (80 base-field leaves of 32 elems, 16+9+6 extension-field leaves of 32 EF elems)
    function _compress(bytes32 a, bytes32 b) internal pure returns (bytes32 r) { assembly { mstore(0x00, a) mstore(0x20, b) r := keccak256(0x00, 0x40) } }
    function testKeccakMerklePath() public {
        bytes32 d = bytes32(uint256(1)); uint256 g = gasleft();
        for (uint256 i = 0; i < 1343; i++) { d = _compress(d, bytes32(i)); }
        emit log_named_uint("GAS 1343 keccak path compressions (n=22 opening)", g - gasleft());
        bytes memory leafB = new bytes(256); bytes memory leafE = new bytes(512);
        g = gasleft(); bytes32 h; for (uint256 i = 0; i < 80; i++) { h = keccak256(leafB); } emit log_named_uint("GAS 80 base-field leaf hashes (256 B)", g - gasleft());
        g = gasleft(); for (uint256 i = 0; i < 31; i++) { h = keccak256(leafE); } emit log_named_uint("GAS 31 extension-field leaf hashes (512 B)", g - gasleft());
        // calldata cost of the paths (32-byte digests): reading is calldataload (3 gas/word) -- negligible next to the 16/40 gas per byte paid up front
        // extension-field (x^2 - 7) multiply-accumulate, the unit of the folding checks: a0*b0 + 7*a1*b1 , a0*b1 + a1*b0
        uint256 P = 0xFFFFFFFF00000001; uint256 a0 = 123456789; uint256 a1 = 987654321; uint256 b0 = 555555555; uint256 b1 = 777777777; uint256 acc0; uint256 acc1;
        g = gasleft();
        for (uint256 i = 0; i < 1000; i++) {
            uint256 c0 = addmod(mulmod(a0, b0, P), mulmod(7, mulmod(a1, b1, P), P), P);
            uint256 c1 = addmod(mulmod(a0, b1, P), mulmod(a1, b0, P), P);
            acc0 = addmod(acc0, c0, P); acc1 = addmod(acc1, c1, P);
        }
        emit log_named_uint("GAS 1000 EF multiply-accumulates (per op = /1000)", g - gasleft());
        assertTrue(acc0 != 0 || acc1 != 0);
    }
}
