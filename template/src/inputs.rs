//! Replace `MyInputs` and `MyOutput` with your domain types. The `prove`
//! entry point in `lib.rs` consumes `MyInputs` and returns `MyOutput`.

#[derive(uniffi::Record)]
pub struct MyInputs {
    pub value: u32,
}

#[derive(uniffi::Record)]
pub struct MyOutput {
    pub proof_bytes: Vec<u8>,
    pub public_value: u32,
}

impl MyOutput {
    pub(crate) fn from_parts(proof_bytes: Vec<u8>, inputs: &MyInputs) -> Self {
        Self {
            proof_bytes,
            public_value: inputs.value,
        }
    }
}
