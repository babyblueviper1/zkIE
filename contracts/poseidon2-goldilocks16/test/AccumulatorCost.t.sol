// SPDX-License-Identifier: CC0-1.0
pragma solidity ^0.8.24;
import "forge-std/Test.sol";
// Gas of the EVM precompiles that decide the accumulator question (zkIE weights commitment, PR #15 follow-up):
//  * BN254 ecAdd / ecMul / ecPairing (EIP-196/197, repriced by EIP-1108): what a KZG-style (pairing) batch opening would cost to verify.
//  * modexp (EIP-198, EIP-2565 pricing): what an RSA accumulator (Boneh-Bunz-Fisch batch membership + Wesolowski PoE) would cost.
contract AccumulatorCost is Test {
    uint256 constant P = 21888242871839275222246405745257275088696311157297823662689037894645226208583;
    // BN254 G2 generator, EVM word order: x_im, x_re, y_im, y_re
    uint256 constant G2XI = 11559732032986387107991004021392285783925812861821192530917403151452391805634;
    uint256 constant G2XR = 10857046999023057135944570762232829481370756359578518086990519993285655852781;
    uint256 constant G2YI = 4082367875863433681332203403145435568316851327593401208105741076214120093531;
    uint256 constant G2YR = 8495653923123431417604973247489272438418190587263600148770280649306958101930;

    function _call(uint256 addr, bytes memory input, uint256 outLen) internal view returns (bool ok, uint256 gasUsed, bytes memory out) {
        out = new bytes(outLen);
        uint256 g = gasleft();
        assembly { ok := staticcall(gas(), addr, add(input, 0x20), mload(input), add(out, 0x20), outLen) }
        gasUsed = g - gasleft();
    }

    function testBn254Precompiles() public {
        (bool ok, uint256 gAdd,) = _call(6, abi.encodePacked(uint256(1), uint256(2), uint256(1), uint256(2)), 64);
        assertTrue(ok); emit log_named_uint("GAS ecAdd (call incl. overhead)", gAdd);
        (ok, gAdd,) = _call(7, abi.encodePacked(uint256(1), uint256(2), uint256(0xdeadbeefcafebabe1234567890abcdef1234567890abcdef1234567890abcdef)), 64);
        assertTrue(ok); emit log_named_uint("GAS ecMul (call incl. overhead)", gAdd);
        // e(G1, G2) * e(-G1, G2) == 1 : the shape of a KZG single-opening check with k = 2 pairs; k = 4 for a batched / multi-opening check
        bytes memory in2 = abi.encodePacked(uint256(1), uint256(2), G2XI, G2XR, G2YI, G2YR, uint256(1), P - 2, G2XI, G2XR, G2YI, G2YR);
        uint256 g2; (ok, g2, ) = _call(8, in2, 32); assertTrue(ok); emit log_named_uint("GAS ecPairing, 2 pairs (KZG single/batched opening)", g2);
        bytes memory in4 = abi.encodePacked(in2, in2);
        uint256 g4; bytes memory o; (ok, g4, o) = _call(8, in4, 32); assertTrue(ok); assertEq(uint256(bytes32(o)), 1);
        emit log_named_uint("GAS ecPairing, 4 pairs", g4);
    }

    function _modexpGas(uint256 bLen, uint256 eLen, uint256 mLen) internal returns (uint256 gasUsed) {
        bytes memory b = new bytes(bLen); bytes memory e = new bytes(eLen); bytes memory m = new bytes(mLen);
        for (uint256 i = 0; i < bLen; i++) b[i] = bytes1(uint8(0x5a ^ i)); for (uint256 i = 0; i < eLen; i++) e[i] = bytes1(uint8(0xc3 ^ i));
        for (uint256 i = 0; i < mLen; i++) m[i] = bytes1(uint8(0xf1 ^ (i * 7))); m[mLen - 1] = bytes1(uint8(0x01 | uint8(m[mLen - 1])));
        m[0] = bytes1(uint8(0x80 | uint8(m[0]))); b[0] = bytes1(uint8(0x7f & uint8(b[0])));
        bool ok; (ok, gasUsed, ) = _call(5, abi.encodePacked(bLen, eLen, mLen, b, e, m), mLen); assertTrue(ok);
    }

    function testModexpRsa() public {
        emit log_named_uint("GAS modexp 2048-bit modulus, 128-bit exp  (Wesolowski PoE check, x2 per batch proof)", _modexpGas(256, 16, 256));
        emit log_named_uint("GAS modexp 2048-bit modulus, 256-bit exp", _modexpGas(256, 32, 256));
        emit log_named_uint("GAS modexp 2048-bit modulus, 2048-bit exp (one full-size exponent)", _modexpGas(256, 256, 256));
        emit log_named_uint("GAS modexp 2048-bit modulus, 128*100 = 12800-bit exp (membership of 100 primes, no PoE)", _modexpGas(256, 1600, 256));
        emit log_named_uint("GAS modexp 128-bit modulus, 128-bit exp   (one Miller-Rabin round for hash-to-prime on a 128-bit candidate)", _modexpGas(16, 16, 16));
        emit log_named_uint("GAS modexp 3072-bit modulus, 128-bit exp  (3072-bit RSA modulus, PoE check)", _modexpGas(384, 16, 384));
    }
}
