// SPDX-FileCopyrightText: 2026 Andrei Kochergin <andrei@oumuamua.dev>
// SPDX-FileCopyrightText: 2026 Oumuamua Labs <info@oumuamua.dev>
// SPDX-License-Identifier: AGPL-3.0-only

use hekate_core::errors;
use hekate_gadgets::Modexp;
use hekate_gadgets::chiplets::bignum::modexp::LIMBS32;
use hekate_rsa::{Pkcs1Statement, encode_sha256};
use hekate_sha2::{pad_message, sha256_words};
use std::sync::Arc;
use std::time::{Duration, Instant};
use zeroize::Zeroizing;

use crate::{
    CancelToken, Proof, ProveError, VerifyError, ensure_not_cancelled, guard, run_prover,
    run_verifier,
};

const TRANSCRIPT_LABEL: &[u8] = b"hekate-mobile-example-rsa-pkcs1-v1";

const MODULUS_BYTES: usize = LIMBS32 * 4;
const MESSAGE_BLOCKS: usize = 1;
const ROUNDS_PER_ROW: usize = 2;
const SHA_ROWS: usize = 512;
const CPU_ROWS: usize = 512;

#[derive(uniffi::Record)]
pub struct RsaInputs {
    pub modulus: Vec<u8>,
    pub message: Vec<u8>,
    pub signature: Vec<u8>,
}

#[uniffi::export]
pub fn prove_rsa(inputs: RsaInputs, cancel: Option<Arc<CancelToken>>) -> Result<Proof, ProveError> {
    guard(ProveError::Panic, || prove(inputs, cancel.as_deref()))
}

#[uniffi::export]
pub fn verify_rsa(
    program_id: String,
    proof_bytes: Vec<u8>,
    modulus: Vec<u8>,
) -> Result<Duration, VerifyError> {
    guard(VerifyError::Panic, || {
        verify(&program_id, &proof_bytes, &modulus)
    })
}

fn prove(inputs: RsaInputs, cancel: Option<&CancelToken>) -> Result<Proof, ProveError> {
    ensure_not_cancelled(cancel)?;

    let message = Zeroizing::new(inputs.message);
    let signature_bytes = Zeroizing::new(inputs.signature);

    let blocks = pad_message(&message).len();

    if blocks != MESSAGE_BLOCKS {
        return Err(ProveError::InvalidInput(format!(
            "message is {} bytes and pads to {blocks} SHA-256 blocks, \
             the program hashes {MESSAGE_BLOCKS}",
            message.len()
        )));
    }

    let modulus =
        limbs(&inputs.modulus).map_err(|e| ProveError::InvalidInput(format!("modulus is {e}")))?;

    let signature = Zeroizing::new(
        limbs(&signature_bytes)
            .map_err(|e| ProveError::InvalidInput(format!("signature is {e}")))?,
    );

    let statement = statement().map_err(|e| ProveError::Prover(format!("build statement: {e}")))?;

    check_signature(&message, &modulus, &signature)?;

    let witness_start = Instant::now();

    let witness = statement
        .witness(&message, &modulus, &signature)
        .map_err(|e| ProveError::Witness(e.to_string()))?;

    let witness_time = witness_start.elapsed();

    run_prover(
        TRANSCRIPT_LABEL,
        statement.program(),
        &statement.instance(&modulus),
        &witness,
        witness_time,
        cancel,
    )
}

fn verify(program_id: &str, proof_bytes: &[u8], modulus: &[u8]) -> Result<Duration, VerifyError> {
    let modulus =
        limbs(modulus).map_err(|e| VerifyError::InvalidInput(format!("modulus is {e}")))?;

    let statement = statement().map_err(|e| VerifyError::Setup(format!("build statement: {e}")))?;

    run_verifier(
        TRANSCRIPT_LABEL,
        program_id,
        statement.program(),
        &statement.instance(&modulus),
        proof_bytes,
    )
}

fn statement() -> errors::Result<Pkcs1Statement> {
    Pkcs1Statement::new(MESSAGE_BLOCKS, ROUNDS_PER_ROW, SHA_ROWS, CPU_ROWS)
}

fn check_signature(
    message: &[u8],
    modulus: &[u32; LIMBS32],
    signature: &[u32; LIMBS32],
) -> Result<(), ProveError> {
    let recovered = Modexp::new(modulus, signature)
        .map_err(|e| ProveError::InvalidInput(format!("signature: {e}")))?;

    let expected = Zeroizing::new(encode_sha256(&sha256_words(message)));

    match recovered.result() == &*expected {
        true => Ok(()),
        false => Err(ProveError::Witness(String::from(
            "the signature is not RSA PKCS#1 v1.5 with SHA-256 and e = 65537 \
             over this message under this modulus",
        ))),
    }
}

fn limbs(big_endian: &[u8]) -> Result<[u32; LIMBS32], String> {
    let bytes: &[u8; MODULUS_BYTES] = big_endian
        .try_into()
        .map_err(|_| format!("{} bytes, expected {MODULUS_BYTES}", big_endian.len()))?;

    Ok(core::array::from_fn(|j| {
        let at = MODULUS_BYTES - 4 * (j + 1);

        u32::from_be_bytes([bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]])
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[rustfmt::skip]
    const MODULUS: [u8; 256] = [
        0xce, 0x62, 0x4a, 0xaf, 0x0b, 0x31, 0xd8, 0x7e, 0xb9, 0xe0, 0xbe, 0x0e, 0xb6, 0x75, 0xac, 0xcc,
        0x7d, 0x58, 0x0f, 0x3b, 0xdc, 0xeb, 0x64, 0x21, 0x77, 0x1c, 0xa6, 0x0a, 0x7d, 0x68, 0xe2, 0x45,
        0xdd, 0xc1, 0x3e, 0xf1, 0x37, 0xe4, 0x3d, 0x66, 0x17, 0xc8, 0x9a, 0xcd, 0xa9, 0x5d, 0x5f, 0xcf,
        0x92, 0x55, 0x51, 0x77, 0x69, 0x20, 0xbb, 0xe9, 0x29, 0x33, 0x8d, 0xf3, 0x16, 0x74, 0xde, 0x99,
        0xeb, 0x0c, 0x48, 0x86, 0x38, 0x68, 0x6a, 0x93, 0x98, 0x68, 0xe6, 0x88, 0x33, 0x02, 0x41, 0xe8,
        0x74, 0x77, 0xfb, 0xde, 0x14, 0xf8, 0xd1, 0x49, 0x52, 0xcd, 0x4a, 0x64, 0x50, 0x3f, 0x3f, 0x06,
        0xe7, 0xf1, 0xc9, 0x80, 0xce, 0xa9, 0x92, 0xd9, 0xe7, 0x00, 0x1d, 0x84, 0x57, 0x69, 0x9c, 0x11,
        0x50, 0xe2, 0x7a, 0x6c, 0xf1, 0xfc, 0xa8, 0xed, 0x6a, 0x22, 0x1a, 0x00, 0xf2, 0xd3, 0x59, 0x8c,
        0x54, 0xd9, 0xe7, 0xcb, 0x5a, 0x8d, 0x2b, 0x9d, 0xd1, 0x9e, 0x63, 0xb4, 0x24, 0x75, 0x9e, 0xea,
        0xd8, 0x94, 0x21, 0x88, 0xfd, 0x96, 0xd2, 0xff, 0x09, 0x7b, 0x46, 0x71, 0x25, 0x17, 0xa6, 0xc4,
        0xf9, 0x2b, 0xbb, 0xf5, 0x51, 0xd7, 0xf2, 0x34, 0x21, 0x21, 0xc9, 0xb3, 0x47, 0xba, 0x69, 0xa1,
        0x81, 0x95, 0xa7, 0x99, 0x1f, 0xf6, 0x12, 0xba, 0xe9, 0xfc, 0x9d, 0x87, 0x74, 0x6d, 0x2a, 0x02,
        0xdc, 0xcf, 0xe5, 0x60, 0x92, 0x26, 0x11, 0x94, 0xf0, 0x16, 0xf6, 0x4e, 0x02, 0x29, 0xb9, 0xfb,
        0x71, 0x87, 0xfa, 0xb2, 0x0d, 0x43, 0x8c, 0x35, 0xc5, 0x62, 0x55, 0x2a, 0xd6, 0xc2, 0xc6, 0xfa,
        0xa7, 0xdd, 0x6e, 0x47, 0xea, 0x2b, 0x52, 0x58, 0xb6, 0x10, 0xea, 0x08, 0x03, 0xc2, 0x10, 0x1c,
        0x52, 0xfa, 0x1e, 0x0b, 0x45, 0x6c, 0x21, 0x0a, 0x03, 0x06, 0xe9, 0xee, 0x9c, 0x48, 0xf3, 0x49,
    ];

    #[rustfmt::skip]
    const MESSAGE: [u8; 32] = [
        0x68, 0x65, 0x6b, 0x61, 0x74, 0x65, 0x2d, 0x6d, 0x6f, 0x62, 0x69, 0x6c, 0x65, 0x20, 0x52, 0x53,
        0x41, 0x20, 0x65, 0x78, 0x61, 0x6d, 0x70, 0x6c, 0x65, 0x20, 0x76, 0x65, 0x63, 0x74, 0x6f, 0x72,
    ];

    #[rustfmt::skip]
    const SIGNATURE: [u8; 256] = [
        0xbd, 0x01, 0xc6, 0x0a, 0x76, 0x2f, 0x2a, 0xe2, 0xb2, 0x91, 0xf1, 0x15, 0xe4, 0xc2, 0x4e, 0x3d,
        0xe9, 0xf1, 0x3b, 0x70, 0x5b, 0x93, 0x8a, 0x48, 0x79, 0xb8, 0xc4, 0x56, 0xd8, 0x7a, 0xdd, 0x02,
        0xff, 0xee, 0x32, 0xaa, 0x3f, 0xd5, 0x82, 0xa7, 0x83, 0x88, 0xf8, 0xe5, 0xd5, 0x3a, 0xa3, 0x45,
        0x0e, 0x82, 0xa0, 0x52, 0x5d, 0x49, 0x39, 0xe5, 0x28, 0x36, 0xd3, 0x5b, 0xa7, 0xb7, 0xfa, 0x43,
        0xf7, 0xd9, 0x5c, 0xa5, 0x15, 0xbd, 0xf1, 0x46, 0x3a, 0xfe, 0x6c, 0xb1, 0x5d, 0xf0, 0x51, 0x13,
        0xd4, 0x8c, 0xc9, 0x5b, 0x98, 0x2e, 0x0f, 0xf8, 0xd0, 0x1f, 0x6b, 0x2e, 0x7f, 0x03, 0x6f, 0x69,
        0xc9, 0x33, 0xd1, 0xc6, 0x82, 0x43, 0x74, 0xf1, 0x82, 0x05, 0x35, 0x56, 0x15, 0x50, 0x77, 0xb4,
        0xc7, 0x0d, 0xa2, 0xc6, 0xd6, 0x8b, 0x74, 0xea, 0xa6, 0xec, 0x3e, 0xb7, 0x26, 0x31, 0x11, 0x6d,
        0x1d, 0x85, 0xc4, 0xa2, 0xe2, 0xbb, 0x62, 0x06, 0x63, 0x35, 0x10, 0x9d, 0x9b, 0xcc, 0x0a, 0x00,
        0x32, 0xdc, 0x2c, 0x1c, 0xa1, 0xe3, 0xc6, 0xfc, 0x89, 0xbf, 0xf2, 0x8e, 0x78, 0xc9, 0xe8, 0xe9,
        0x39, 0x0b, 0x16, 0xb7, 0x5c, 0xad, 0x5f, 0x80, 0x72, 0x57, 0x53, 0xec, 0xee, 0x21, 0x03, 0x39,
        0xa5, 0xf7, 0x2e, 0x0b, 0xfe, 0x2a, 0x68, 0xd7, 0xba, 0x1b, 0xbb, 0x4f, 0x93, 0x06, 0xbb, 0xa7,
        0x40, 0x78, 0x29, 0x46, 0x15, 0xa1, 0x17, 0x7d, 0x68, 0x12, 0xcc, 0xfb, 0x83, 0x8e, 0xc8, 0xee,
        0x09, 0xb1, 0x49, 0x6e, 0x15, 0xd6, 0xbb, 0x42, 0xf1, 0xd7, 0x47, 0x85, 0x76, 0x15, 0x85, 0x3e,
        0xb1, 0xbe, 0x21, 0x82, 0x13, 0x61, 0x39, 0x62, 0xbb, 0xf1, 0x0e, 0xe6, 0x21, 0xff, 0x05, 0xab,
        0x58, 0xa3, 0x65, 0x61, 0xf4, 0xf2, 0x0d, 0x87, 0xb5, 0x47, 0x47, 0xcc, 0x4a, 0xe1, 0x0d, 0x92,
    ];

    fn inputs() -> RsaInputs {
        RsaInputs {
            modulus: MODULUS.to_vec(),
            message: MESSAGE.to_vec(),
            signature: SIGNATURE.to_vec(),
        }
    }

    fn honest_proof() -> Proof {
        prove_rsa(inputs(), None).expect("an honest signature proves")
    }

    fn refusal(inputs: RsaInputs, cancel: Option<Arc<CancelToken>>) -> ProveError {
        match prove_rsa(inputs, cancel) {
            Ok(_) => panic!("the prover returned a proof"),
            Err(e) => e,
        }
    }

    #[test]
    fn security_framework_signature_proves_and_verifies() {
        let proof = honest_proof();

        verify_rsa(proof.program_id, proof.proof_bytes, MODULUS.to_vec())
            .expect("an honest proof verifies");
    }

    #[test]
    fn forged_signature_is_refused_before_proving() {
        let mut forged = inputs();
        forged.signature[255] ^= 1;

        let err = refusal(forged, None);

        assert!(matches!(err, ProveError::Witness(_)), "{err}");
    }

    #[test]
    fn altered_message_is_refused_before_proving() {
        let mut altered = inputs();
        altered.message[0] ^= 1;

        let err = refusal(altered, None);

        assert!(matches!(err, ProveError::Witness(_)), "{err}");
    }

    #[test]
    fn message_past_one_block_is_refused() {
        let mut long = inputs();
        long.message = vec![0u8; 56];

        let err = refusal(long, None);

        assert!(matches!(err, ProveError::InvalidInput(_)), "{err}");
    }

    #[test]
    fn modulus_with_sign_octet_is_refused() {
        let mut padded = inputs();
        padded.modulus.insert(0, 0);

        let err = refusal(padded, None);

        assert!(matches!(err, ProveError::InvalidInput(_)), "{err}");
    }

    #[test]
    fn requested_token_cancels_proof() {
        let token = CancelToken::new();
        token.request();

        let err = refusal(inputs(), Some(token));

        assert!(matches!(err, ProveError::Cancelled), "{err}");
    }

    #[test]
    fn requested_token_stops_before_witness() {
        let token = CancelToken::new();
        token.request();

        let mut forged = inputs();
        forged.signature[255] ^= 1;

        let err = refusal(forged, Some(token));

        assert!(matches!(err, ProveError::Cancelled), "{err}");
    }

    #[test]
    fn proof_is_rejected_under_another_modulus() {
        let proof = honest_proof();

        let mut other = MODULUS.to_vec();
        other[255] ^= 2;

        let err = verify_rsa(proof.program_id, proof.proof_bytes, other)
            .expect_err("the proof is bound to its modulus");

        assert!(matches!(err, VerifyError::Rejected(_)), "{err}");
    }

    #[test]
    fn tampered_proof_is_not_accepted() {
        let proof = honest_proof();

        for at in [proof.proof_bytes.len() / 3, proof.proof_bytes.len() - 9] {
            let mut tampered = proof.proof_bytes.clone();
            tampered[at] ^= 0x01;

            let err = verify_rsa(proof.program_id.clone(), tampered, MODULUS.to_vec())
                .expect_err("a flipped bit is not accepted");

            assert!(
                matches!(err, VerifyError::InvalidInput(_) | VerifyError::Rejected(_)),
                "byte {at}: {err}"
            );
        }
    }

    #[test]
    fn foreign_program_id_is_rejected() {
        let proof = honest_proof();

        let err = verify_rsa("00".repeat(32), proof.proof_bytes, MODULUS.to_vec())
            .expect_err("the id of another program is rejected");

        assert!(matches!(err, VerifyError::Rejected(_)), "{err}");
    }
}
