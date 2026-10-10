//! Replace `MyInputs` and `MyOutput` with your domain types.
//! The `prove` entry point in `lib.rs` consumes `MyInputs`
//! and returns `MyOutput`.

#[derive(uniffi::Record)]
pub struct MyInputs {
    pub value: u32,
}

#[derive(uniffi::Record)]
pub struct MyOutput {
    /// The verifier takes this as its first argument and
    /// rejects on mismatch with the id recomputed from its
    /// own program. It never travels inside the proof.
    pub program_id: String,

    pub proof_bytes: Vec<u8>,
    pub public_value: u32,
}

impl MyOutput {
    pub(crate) fn from_parts(program_id: String, proof_bytes: Vec<u8>, inputs: &MyInputs) -> Self {
        Self {
            program_id,
            proof_bytes,
            public_value: inputs.value,
        }
    }
}
