// SPDX-FileCopyrightText: 2026 Andrei Kochergin <andrei@oumuamua.dev>
// SPDX-FileCopyrightText: 2026 Oumuamua Labs <info@oumuamua.dev>
// SPDX-License-Identifier: AGPL-3.0-only

use hekate_core::errors;
use hekate_core::trace::{ColumnTrace, TraceBuilder};
use hekate_keccak::shake256;
use hekate_math::{Bit, Block32, TowerField};
use hekate_pqc::mldsa::{self, MLDSA_DATA_BUS_ID, MlDsaChiplet, MlDsaInput, MlDsaParams};
use hekate_program::circuit::{Circuit, CircuitProgram};
use hekate_program::{FixedShape, ProgramInstance, ProgramWitness, define_columns};
use std::ops::Range;
use std::sync::Arc;
use std::time::{Duration, Instant};
use zeroize::Zeroizing;

use crate::{
    CancelToken, F, Proof, ProveError, VerifyError, ensure_not_cancelled, guard, run_prover,
    run_verifier,
};

const TRANSCRIPT_LABEL: &[u8] = b"hekate-mobile-example-mldsa65-v1";

const PARAMS: MlDsaParams = MlDsaParams::ML_DSA_65;

const PURE_ML_DSA: u8 = 0;
const EMPTY_CONTEXT_LEN: u8 = 0;
const MESSAGE_BYTES: usize = 32;
const M_PRIME_BYTES: usize = 2 + MESSAGE_BYTES;

const KEY_HASH_BYTES: usize = 64;
const DIGEST_WORDS: usize = KEY_HASH_BYTES / 4;

define_columns! {
    HostColumns {
        WORD: B32,
        SEL: Bit,
    }
}

#[derive(uniffi::Record)]
pub struct MlDsaInputs {
    pub public_key: Vec<u8>,
    pub message: Vec<u8>,
    pub signature: Vec<u8>,
}

struct Statement {
    pipeline: MlDsaChiplet<F>,
    program: CircuitProgram<F>,
    rows: usize,
    words: usize,
    m_prime_rows: Range<usize>,
    key_hash_rows: Range<usize>,
}

#[uniffi::export]
pub fn prove_mldsa(
    inputs: MlDsaInputs,
    cancel: Option<Arc<CancelToken>>,
) -> Result<Proof, ProveError> {
    guard(ProveError::Panic, || prove(inputs, cancel.as_deref()))
}

#[uniffi::export]
pub fn verify_mldsa(
    program_id: String,
    proof_bytes: Vec<u8>,
    message: Vec<u8>,
    key_hash: Vec<u8>,
) -> Result<Duration, VerifyError> {
    guard(VerifyError::Panic, || {
        verify(&program_id, &proof_bytes, &message, &key_hash)
    })
}

#[uniffi::export]
pub fn mldsa_key_hash(public_key: Vec<u8>) -> Vec<u8> {
    shake256(&public_key, KEY_HASH_BYTES).0.to_vec()
}

impl Statement {
    fn new() -> errors::Result<Self> {
        let pipeline = MlDsaChiplet::<F>::new(PARAMS, &[M_PRIME_BYTES])?;

        let public_key_words = PARAMS.pk_bytes().div_ceil(4);
        let m_prime_words = M_PRIME_BYTES.div_ceil(4);
        let signature_words = PARAMS.sig_bytes().div_ceil(4);

        let words = public_key_words + m_prime_words + signature_words + 2 * DIGEST_WORDS;
        let rows = words.next_power_of_two();

        let m_prime_rows = public_key_words..public_key_words + m_prime_words;
        let key_hash_rows = words - 2 * DIGEST_WORDS..words - DIGEST_WORDS;

        let mut cx = Circuit::<F>::new("MlDsaVerify", rows)?;

        let cols = cx.schema(&HostColumns::build_layout());
        let word = cols.at(HostColumns::WORD);
        let sel = cols.at(HostColumns::SEL);

        cx.fix(
            sel,
            FixedShape::Cadence {
                stride: 1,
                count: words,
                origin: 0,
                values: vec![F::ONE],
            },
        );

        let cs = cx.cs();
        let outside_service_rows = cs.one() + cs.col(sel.index());

        cs.constrain(outside_service_rows * cs.col(word.index()));

        cx.call(&mldsa::service(), &[word], sel)?;

        for row in m_prime_rows.clone().chain(key_hash_rows.clone()) {
            cx.publish(word, row);
        }

        cx.attach_namespaced("mldsa", pipeline.defs()?, &[MLDSA_DATA_BUS_ID])?;

        Ok(Self {
            program: cx.compile()?,
            pipeline,
            rows,
            words,
            m_prime_rows,
            key_hash_rows,
        })
    }

    fn instance(&self, m_prime: &[u8], key_hash: &[u8; KEY_HASH_BYTES]) -> ProgramInstance<F> {
        let public = public_words(m_prime, key_hash)
            .map(|word| F::from(u128::from(word)))
            .collect();

        ProgramInstance::new(self.rows, public)
    }

    fn host_trace(&self, words: &[u32]) -> errors::Result<ColumnTrace> {
        let mut tb = TraceBuilder::new_secret(
            &HostColumns::build_layout(),
            self.rows.trailing_zeros() as usize,
        )?;

        for (row, &word) in words.iter().enumerate() {
            tb.set_b32(HostColumns::WORD, row, Block32::from(word))?;
            tb.set_bit(HostColumns::SEL, row, Bit::ONE)?;
        }

        Ok(tb.build())
    }

    fn ensure_published(
        &self,
        words: &[u32],
        m_prime: &[u8],
        key_hash: &[u8; KEY_HASH_BYTES],
    ) -> Result<(), ProveError> {
        let published = self
            .m_prime_rows
            .clone()
            .chain(self.key_hash_rows.clone())
            .map(|row| words.get(row).copied());

        let expected = public_words(m_prime, key_hash).map(Some);

        match words.len() == self.words && published.eq(expected) {
            true => Ok(()),
            false => Err(ProveError::Prover(String::from(
                "the pipeline's host words do not hold M' and tr at the published rows",
            ))),
        }
    }
}

fn prove(inputs: MlDsaInputs, cancel: Option<&CancelToken>) -> Result<Proof, ProveError> {
    ensure_not_cancelled(cancel)?;

    let public_key = Zeroizing::new(inputs.public_key);
    let signature = Zeroizing::new(inputs.signature);

    let m_prime = m_prime(&inputs.message).map_err(ProveError::InvalidInput)?;

    ensure_len("public key", &public_key, PARAMS.pk_bytes()).map_err(ProveError::InvalidInput)?;
    ensure_len("signature", &signature, PARAMS.sig_bytes()).map_err(ProveError::InvalidInput)?;

    let statement =
        Statement::new().map_err(|e| ProveError::Prover(format!("build statement: {e}")))?;

    let witness_start = Instant::now();

    let traced = statement
        .pipeline
        .trace(&[MlDsaInput {
            pk: &public_key,
            message: &m_prime,
            signature: &signature,
        }])
        .map_err(|e| ProveError::Witness(e.to_string()))?;

    let host = statement
        .host_trace(&traced.words)
        .map_err(|e| ProveError::Witness(format!("host trace: {e}")))?;

    let witness_time = witness_start.elapsed();

    let key_hash = match traced.outputs.first() {
        Some(output) => output.tr,
        None => {
            return Err(ProveError::Prover(String::from(
                "the pipeline returned no output for the call",
            )));
        }
    };

    statement.ensure_published(&traced.words, &m_prime, &key_hash)?;

    run_prover(
        TRANSCRIPT_LABEL,
        &statement.program,
        &statement.instance(&m_prime, &key_hash),
        &ProgramWitness::new(host).with_chiplets(traced.traces),
        witness_time,
        cancel,
    )
}

fn verify(
    program_id: &str,
    proof_bytes: &[u8],
    message: &[u8],
    key_hash: &[u8],
) -> Result<Duration, VerifyError> {
    let m_prime = m_prime(message).map_err(VerifyError::InvalidInput)?;

    let key_hash: &[u8; KEY_HASH_BYTES] = key_hash.try_into().map_err(|_| {
        VerifyError::InvalidInput(format!(
            "key hash is {} bytes, expected {KEY_HASH_BYTES}",
            key_hash.len()
        ))
    })?;

    let statement =
        Statement::new().map_err(|e| VerifyError::Setup(format!("build statement: {e}")))?;

    run_verifier(
        TRANSCRIPT_LABEL,
        program_id,
        &statement.program,
        &statement.instance(&m_prime, key_hash),
        proof_bytes,
    )
}

fn m_prime(message: &[u8]) -> Result<Vec<u8>, String> {
    ensure_len("message", message, MESSAGE_BYTES)?;

    Ok([&[PURE_ML_DSA, EMPTY_CONTEXT_LEN][..], message].concat())
}

fn ensure_len(what: &str, bytes: &[u8], expected: usize) -> Result<(), String> {
    match bytes.len() == expected {
        true => Ok(()),
        false => Err(format!(
            "{what} is {} bytes, expected {expected}",
            bytes.len()
        )),
    }
}

fn public_words<'a>(
    m_prime: &'a [u8],
    key_hash: &'a [u8; KEY_HASH_BYTES],
) -> impl Iterator<Item = u32> + 'a {
    le_words(m_prime).chain(le_words(key_hash))
}

fn le_words(bytes: &[u8]) -> impl Iterator<Item = u32> + '_ {
    bytes.chunks(4).map(|chunk| {
        let mut word = [0u8; 4];
        word[..chunk.len()].copy_from_slice(chunk);

        u32::from_le_bytes(word)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use ml_dsa::{B32, MlDsa65, SigningKey};

    const MESSAGE: &[u8; MESSAGE_BYTES] = b"hekate-mobile ML-DSA test vector";
    const SEED: [u8; 32] = [7; 32];

    const CRYPTOKIT_SIGNATURE_HEX: &str =
        include_str!("../vectors/cryptokit-mldsa65-signature.hex");

    fn signed(message: &[u8], context: &[u8]) -> MlDsaInputs {
        let key = SigningKey::<MlDsa65>::from_seed(&B32::from(SEED));
        let expanded = key.expanded_key();

        let signature = expanded
            .sign_deterministic(message, context)
            .expect("the context fits");

        MlDsaInputs {
            public_key: expanded.verifying_key().encode().to_vec(),
            message: message.to_vec(),
            signature: signature.encode().to_vec(),
        }
    }

    fn unhex(text: &str) -> Vec<u8> {
        let digits: Vec<u8> = text.bytes().filter(u8::is_ascii_hexdigit).collect();

        digits
            .chunks(2)
            .map(|pair| {
                let pair = std::str::from_utf8(pair).expect("ASCII hex");

                u8::from_str_radix(pair, 16).expect("two hex digits")
            })
            .collect()
    }

    fn refusal(inputs: MlDsaInputs, cancel: Option<Arc<CancelToken>>) -> ProveError {
        match prove_mldsa(inputs, cancel) {
            Ok(_) => panic!("the prover returned a proof"),
            Err(e) => e,
        }
    }

    #[test]
    fn external_interface_signature_proves_and_verifies() {
        let inputs = signed(MESSAGE, b"");
        let key_hash = mldsa_key_hash(inputs.public_key.clone());

        let proof = prove_mldsa(inputs, None).expect("an honest signature proves");

        verify_mldsa(
            proof.program_id,
            proof.proof_bytes,
            MESSAGE.to_vec(),
            key_hash,
        )
        .expect("an honest proof verifies");
    }

    #[test]
    fn forged_signature_has_no_witness() {
        let mut forged = signed(MESSAGE, b"");
        forged.signature[40] ^= 1;

        let err = refusal(forged, None);

        assert!(matches!(err, ProveError::Witness(_)), "{err}");
    }

    #[test]
    fn signature_under_nonempty_context_has_no_witness() {
        let err = refusal(signed(MESSAGE, b"login"), None);

        assert!(matches!(err, ProveError::Witness(_)), "{err}");
    }

    #[test]
    fn message_of_other_length_is_refused() {
        let err = refusal(signed(&MESSAGE[..31], b""), None);

        assert!(matches!(err, ProveError::InvalidInput(_)), "{err}");
    }

    #[test]
    fn truncated_public_key_is_refused() {
        let mut inputs = signed(MESSAGE, b"");
        inputs.public_key.pop();

        let err = refusal(inputs, None);

        assert!(matches!(err, ProveError::InvalidInput(_)), "{err}");
    }

    #[test]
    fn requested_token_cancels_proof() {
        let token = CancelToken::new();
        token.request();

        let err = refusal(signed(MESSAGE, b""), Some(token));

        assert!(matches!(err, ProveError::Cancelled), "{err}");
    }

    #[test]
    fn proof_is_rejected_for_another_message_or_key() {
        let inputs = signed(MESSAGE, b"");
        let key_hash = mldsa_key_hash(inputs.public_key.clone());

        let proof = prove_mldsa(inputs, None).expect("an honest signature proves");

        let mut other_message = MESSAGE.to_vec();
        other_message[0] ^= 1;

        let err = verify_mldsa(
            proof.program_id.clone(),
            proof.proof_bytes.clone(),
            other_message,
            key_hash.clone(),
        )
        .expect_err("the proof is bound to its message");

        assert!(matches!(err, VerifyError::Rejected(_)), "{err}");

        let mut other_key_hash = key_hash;
        other_key_hash[0] ^= 1;

        let err = verify_mldsa(
            proof.program_id,
            proof.proof_bytes,
            MESSAGE.to_vec(),
            other_key_hash,
        )
        .expect_err("the proof is bound to its key hash");

        assert!(matches!(err, VerifyError::Rejected(_)), "{err}");
    }

    #[test]
    fn cryptokit_signature_proves_and_verifies() {
        let mut inputs = signed(MESSAGE, b"");
        inputs.signature = unhex(CRYPTOKIT_SIGNATURE_HEX);

        let key_hash = mldsa_key_hash(inputs.public_key.clone());

        let proof = prove_mldsa(inputs, None).expect("a CryptoKit signature proves");

        verify_mldsa(
            proof.program_id,
            proof.proof_bytes,
            MESSAGE.to_vec(),
            key_hash,
        )
        .expect("the proof of a CryptoKit signature verifies");
    }
}
