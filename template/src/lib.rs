mod inputs;
mod program;

use hekate_core::config::Config;
use hekate_program::digest::program_id_hex;
use hekate_prover_sys::{CancelToken as ProverCancelToken, Error as ProverError, ErrorCode};
use std::any::Any;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::Arc;
use thiserror::Error;

use program::TRANSCRIPT_LABEL;

pub use inputs::{MyInputs, MyOutput};

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
}

#[uniffi::export]
impl CancelToken {
    #[uniffi::constructor]
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            inner: ProverCancelToken::new(),
        })
    }

    pub fn request(&self) {
        self.inner.request();
    }
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

#[uniffi::export]
pub fn prove(inputs: MyInputs, cancel: Option<Arc<CancelToken>>) -> Result<MyOutput, ProveError> {
    let result = catch_unwind(AssertUnwindSafe(|| {
        let token = cancel.as_deref().map(|t| &t.inner);
        prove_inner(&inputs, token)
    }));

    match result {
        Ok(r) => r,
        Err(payload) => Err(ProveError::Panic(panic_message(payload))),
    }
}

fn prove_inner(
    inputs: &MyInputs,
    cancel: Option<&ProverCancelToken>,
) -> Result<MyOutput, ProveError> {
    let air = program::build_program()
        .map_err(|e| ProveError::InvalidInput(format!("build program: {e}")))?;

    let instance = program::instance(inputs);
    let witness = program::build_witness(inputs)?;

    let program_id =
        program_id_hex(&air).map_err(|e| ProveError::Prover(format!("program id: {e}")))?;

    let mut seed = [0u8; 32];
    getrandom::fill(&mut seed).map_err(|e| ProveError::Prover(format!("getrandom: {e}")))?;

    let proof = hekate_prover_sys::prove(
        TRANSCRIPT_LABEL,
        &air,
        &instance,
        &witness,
        &Config::default(),
        seed,
        cancel,
    )
    .map_err(map_prover_error)?;

    let proof_bytes = hekate_sdk::serialize_proof_bytes(&proof);

    Ok(MyOutput::from_parts(program_id, proof_bytes, inputs))
}

fn map_prover_error(err: ProverError) -> ProveError {
    match err.code() {
        Some(ErrorCode::Cancelled) => ProveError::Cancelled,
        _ => ProveError::Prover(err.to_string()),
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
