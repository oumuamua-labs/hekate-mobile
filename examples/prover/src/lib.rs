// SPDX-FileCopyrightText: 2026 Andrei Kochergin <andrei@oumuamua.dev>
// SPDX-FileCopyrightText: 2026 Oumuamua Labs <info@oumuamua.dev>
// SPDX-License-Identifier: AGPL-3.0-only

mod keccak;
mod mldsa;
mod rsa;
mod sha256;

use hekate_core::config::Config;
use hekate_crypto::DefaultHasher;
use hekate_crypto::transcript::Transcript;
use hekate_math::Block128;
use hekate_program::digest::program_id_hex;
use hekate_program::{Program, ProgramInstance, ProgramWitness};
use hekate_prover_sys::{CancelToken as ProverCancelToken, Error as ProverError, ErrorCode};
use hekate_verifier::HekateVerifier;
use std::any::Any;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};
use thiserror::Error;
use zeroize::Zeroizing;

type F = Block128;
type H = DefaultHasher;

const MIN_NUM_VARS: usize = 8;

uniffi::setup_scaffolding!();

#[cfg(all(
    target_os = "android",
    target_arch = "aarch64",
    not(target_feature = "aes")
))]
compile_error!(
    "aarch64-linux-android needs -C target-feature=+aes, set in .cargo/config.toml. \
     Cargo ignores that file's rustflags when RUSTFLAGS is set: add the feature there."
);

#[derive(uniffi::Object)]
pub struct CancelToken {
    inner: ProverCancelToken,
    requested: AtomicBool,
}

#[derive(uniffi::Record)]
pub struct ProverBuild {
    pub version: String,
    pub mode: String,
}

#[derive(uniffi::Record)]
pub struct Proof {
    pub program_id: String,
    pub proof_bytes: Vec<u8>,
    pub witness_time: Duration,
    pub prove_time: Duration,
}

#[derive(Debug, Error, uniffi::Error)]
#[uniffi(flat_error)]
pub enum ProveError {
    #[error("invalid input: {0}")]
    InvalidInput(String),

    #[error("witness build failed: {0}")]
    Witness(String),

    #[error("prover failed: {0}")]
    Prover(String),

    #[error("cancelled by host")]
    Cancelled,

    #[error("internal panic: {0}")]
    Panic(String),
}

#[derive(Debug, Error, uniffi::Error)]
#[uniffi(flat_error)]
pub enum VerifyError {
    #[error("invalid input: {0}")]
    InvalidInput(String),

    #[error("proof rejected: {0}")]
    Rejected(String),

    #[error("verifier setup failed: {0}")]
    Setup(String),

    #[error("internal panic: {0}")]
    Panic(String),
}

#[uniffi::export]
impl CancelToken {
    #[uniffi::constructor]
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            inner: ProverCancelToken::new(),
            requested: AtomicBool::new(false),
        })
    }

    pub fn request(&self) {
        self.requested.store(true, Ordering::Release);
        self.inner.request();
    }
}

#[uniffi::export]
pub fn prover_build() -> ProverBuild {
    let mode = match cfg!(feature = "public") {
        true => "public",
        false => "ct",
    };

    ProverBuild {
        version: hekate_prover_sys::version().to_string(),
        mode: mode.to_string(),
    }
}

fn guard<T, E>(on_panic: fn(String) -> E, f: impl FnOnce() -> Result<T, E>) -> Result<T, E> {
    match catch_unwind(AssertUnwindSafe(f)) {
        Ok(r) => r,
        Err(payload) => Err(on_panic(panic_message(payload))),
    }
}

fn ensure_not_cancelled(cancel: Option<&CancelToken>) -> Result<(), ProveError> {
    match cancel.is_some_and(|token| token.requested.load(Ordering::Acquire)) {
        true => Err(ProveError::Cancelled),
        false => Ok(()),
    }
}

fn run_prover<P: Program<F>>(
    transcript_label: &'static [u8],
    program: &P,
    instance: &ProgramInstance<F>,
    witness: &ProgramWitness<F>,
    witness_time: Duration,
    cancel: Option<&CancelToken>,
) -> Result<Proof, ProveError> {
    ensure_not_cancelled(cancel)?;

    let program_id =
        program_id_hex(program).map_err(|e| ProveError::Prover(format!("program id: {e}")))?;

    let mut seed = Zeroizing::new([0u8; 32]);
    getrandom::fill(&mut *seed).map_err(|e| ProveError::Prover(format!("getrandom: {e}")))?;

    let prove_start = Instant::now();

    let proof = hekate_prover_sys::prove(
        transcript_label,
        program,
        instance,
        witness,
        &Config::default(),
        *seed,
        cancel.map(|token| &token.inner),
    )
    .map_err(map_prover_error)?;

    let prove_time = prove_start.elapsed();

    Ok(Proof {
        program_id,
        proof_bytes: hekate_sdk::serialize_proof_bytes(&proof),
        witness_time,
        prove_time,
    })
}

fn run_verifier<P: Program<F> + Sync>(
    transcript_label: &'static [u8],
    program_id: &str,
    program: &P,
    instance: &ProgramInstance<F>,
    proof_bytes: &[u8],
) -> Result<Duration, VerifyError> {
    let id = decode_program_id(program_id)?;

    let proof = hekate_sdk::deserialize_proof::<F>(proof_bytes)
        .map_err(|e| VerifyError::InvalidInput(format!("proof: {e}")))?;

    let mut transcript = Transcript::<H>::new(transcript_label);

    let verify_start = Instant::now();

    let verdict = HekateVerifier::<F, H>::verify(
        &id,
        program,
        instance,
        &proof,
        &mut transcript,
        &Config::default(),
    );

    let verify_time = verify_start.elapsed();

    match verdict {
        Ok(true) => Ok(verify_time),
        Ok(false) => Err(VerifyError::Rejected(String::from(
            "the verifier returned false",
        ))),
        Err(e) => Err(VerifyError::Rejected(e.to_string())),
    }
}

fn map_prover_error(err: ProverError) -> ProveError {
    match err.code() {
        Some(ErrorCode::Cancelled) => ProveError::Cancelled,
        _ => ProveError::Prover(err.to_string()),
    }
}

fn decode_program_id(hex: &str) -> Result<[u8; 32], VerifyError> {
    let digits = hex.as_bytes();

    if digits.len() != 64 {
        return Err(VerifyError::InvalidInput(format!(
            "program id is {} characters, expected 64 hex digits",
            digits.len()
        )));
    }

    let mut id = [0u8; 32];

    for (i, (byte, pair)) in id.iter_mut().zip(digits.as_chunks::<2>().0).enumerate() {
        *byte = match (hex_value(pair[0]), hex_value(pair[1])) {
            (Some(high), Some(low)) => high << 4 | low,
            _ => {
                return Err(VerifyError::InvalidInput(format!(
                    "program id byte {i} is not lowercase hex"
                )));
            }
        };
    }

    Ok(id)
}

fn hex_value(digit: u8) -> Option<u8> {
    match digit {
        b'0'..=b'9' => Some(digit - b'0'),
        b'a'..=b'f' => Some(digit - b'a' + 10),
        _ => None,
    }
}

fn panic_message(payload: Box<dyn Any + Send>) -> String {
    let payload = match payload.downcast::<&'static str>() {
        Ok(s) => return (*s).to_string(),
        Err(p) => p,
    };

    match payload.downcast::<String>() {
        Ok(s) => *s,
        Err(_) => "panic with non-string payload".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn malformed_program_id_is_refused() {
        for id in [String::new(), "+f".repeat(32), "AB".repeat(32)] {
            let err = decode_program_id(&id).expect_err("not 64 lowercase hex digits");

            assert!(matches!(err, VerifyError::InvalidInput(_)), "{id}: {err}");
        }
    }
}
