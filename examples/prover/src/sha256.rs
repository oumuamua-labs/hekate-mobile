// SPDX-FileCopyrightText: 2026 Andrei Kochergin <andrei@oumuamua.dev>
// SPDX-FileCopyrightText: 2026 Oumuamua Labs <info@oumuamua.dev>
// SPDX-License-Identifier: AGPL-3.0-only

use hekate_core::errors;
use hekate_core::trace::{ColumnTrace, ColumnType, TraceBuilder};
use hekate_math::{Bit, TowerField};
use hekate_program::circuit::{Circuit, CircuitProgram};
use hekate_program::{FixedShape, ProgramInstance, ProgramWitness};
use hekate_sha2::{
    BLOCK_WORDS, CpuSha256Block, IV, ROUNDS, STATE_WORDS, Sha256Call, Sha256Chiplet, pad_message,
};
use std::sync::Arc;
use std::time::{Duration, Instant};
use zeroize::Zeroizing;

use crate::{
    CancelToken, F, MIN_NUM_VARS, Proof, ProveError, VerifyError, ensure_not_cancelled, guard,
    run_prover, run_verifier,
};

const TRANSCRIPT_LABEL: &[u8] = b"hekate-mobile-example-sha256-v1";

const ROUNDS_PER_ROW: usize = 4;
const ROWS_PER_BLOCK: usize = ROUNDS / ROUNDS_PER_ROW;
const BLOCK_BYTES: usize = BLOCK_WORDS * 4;
const WORD_BYTES: usize = 4;
const DIGEST_BYTES: usize = STATE_WORDS * WORD_BYTES;

const CPU_ACTIVE: usize = CpuSha256Block::COLUMNS;
const CPU_CARRY: usize = CPU_ACTIVE + 1;
const CPU_CHAIN: usize = CPU_CARRY + 1;

struct Statement {
    chiplet: Sha256Chiplet<F>,
    program: CircuitProgram<F>,
    block: CpuSha256Block,
}

impl Statement {
    fn new(message_len: usize) -> Result<Self, String> {
        if !message_len.is_multiple_of(BLOCK_BYTES) {
            return Err(format!(
                "message is {message_len} bytes, the program takes a multiple of {BLOCK_BYTES}"
            ));
        }

        let num_blocks = message_len / BLOCK_BYTES + 1;

        let num_rows = (num_blocks * ROWS_PER_BLOCK)
            .next_power_of_two()
            .max(1 << MIN_NUM_VARS);

        let chiplet = Sha256Chiplet::<F>::new(num_rows, num_blocks, ROUNDS_PER_ROW)
            .map_err(|e| format!("build chiplet: {e}"))?;

        let (program, block) = build_program(&chiplet, num_blocks, message_len)
            .map_err(|e| format!("build statement: {e}"))?;

        Ok(Self {
            chiplet,
            program,
            block,
        })
    }

    fn instance(&self, words: [u32; STATE_WORDS]) -> ProgramInstance<F> {
        let public = words.map(|word| F::from(u128::from(word))).to_vec();

        ProgramInstance::new(self.chiplet.num_rows(), public)
    }
}

#[uniffi::export]
pub fn prove_sha256(
    message: Vec<u8>,
    cancel: Option<Arc<CancelToken>>,
) -> Result<Proof, ProveError> {
    guard(ProveError::Panic, || prove(message, cancel.as_deref()))
}

#[uniffi::export]
pub fn verify_sha256(
    program_id: String,
    proof_bytes: Vec<u8>,
    digest: Vec<u8>,
    message_bytes: u64,
) -> Result<Duration, VerifyError> {
    guard(VerifyError::Panic, || {
        verify(&program_id, &proof_bytes, &digest, message_bytes)
    })
}

#[uniffi::export]
pub fn sha256_capacity(log_rows: u8) -> u64 {
    1usize
        .checked_shl(u32::from(log_rows))
        .map(|rows| rows / ROWS_PER_BLOCK)
        .and_then(|blocks| blocks.checked_sub(1))
        .and_then(|blocks| blocks.checked_mul(BLOCK_BYTES))
        .map_or(0, |bytes| bytes as u64)
}

fn prove(message: Vec<u8>, cancel: Option<&CancelToken>) -> Result<Proof, ProveError> {
    ensure_not_cancelled(cancel)?;

    let message = Zeroizing::new(message);
    let statement = Statement::new(message.len()).map_err(ProveError::InvalidInput)?;

    let witness_start = Instant::now();

    let calls = sha256_calls(&message);

    let trace = build_trace(&statement, &calls).map_err(|e| ProveError::Witness(e.to_string()))?;

    let witness_time = witness_start.elapsed();

    run_prover(
        TRANSCRIPT_LABEL,
        &statement.program,
        &statement.instance(digest_words(&calls)),
        &ProgramWitness::new(trace),
        witness_time,
        cancel,
    )
}

fn verify(
    program_id: &str,
    proof_bytes: &[u8],
    digest: &[u8],
    message_bytes: u64,
) -> Result<Duration, VerifyError> {
    let digest: &[u8; DIGEST_BYTES] = digest.try_into().map_err(|_| {
        VerifyError::InvalidInput(format!(
            "digest is {} bytes, expected {DIGEST_BYTES}",
            digest.len()
        ))
    })?;

    let message_len = usize::try_from(message_bytes).map_err(|_| {
        VerifyError::InvalidInput(format!("message length {message_bytes} does not fit usize"))
    })?;

    let statement = Statement::new(message_len).map_err(VerifyError::InvalidInput)?;

    let words = core::array::from_fn(|i| {
        let mut word = [0u8; WORD_BYTES];
        word.copy_from_slice(&digest[i * WORD_BYTES..(i + 1) * WORD_BYTES]);

        u32::from_be_bytes(word)
    });

    run_verifier(
        TRANSCRIPT_LABEL,
        program_id,
        &statement.program,
        &statement.instance(words),
        proof_bytes,
    )
}

fn build_program(
    chiplet: &Sha256Chiplet<F>,
    num_blocks: usize,
    message_len: usize,
) -> errors::Result<(CircuitProgram<F>, CpuSha256Block)> {
    let mut cx = Circuit::<F>::new("Sha256Preimage", chiplet.num_rows())?;

    let block = CpuSha256Block::declare(&mut cx, 0);

    let active = cx.column(ColumnType::Bit);
    let carry = cx.column(ColumnType::Bit);
    let chain = cx.column(ColumnType::Bit);

    block.connect(&mut cx, active)?;

    let cadence = |count: usize, set: &dyn Fn(usize) -> bool| FixedShape::Cadence {
        stride: ROWS_PER_BLOCK,
        count,
        origin: 0,
        values: (0..ROWS_PER_BLOCK)
            .map(|offset| match set(offset) {
                true => F::ONE,
                false => F::ZERO,
            })
            .collect(),
    };

    cx.fix(active, cadence(num_blocks, &|offset| offset == 0));
    cx.fix(
        carry,
        cadence(num_blocks, &|offset| offset + 1 < ROWS_PER_BLOCK),
    );
    cx.fix(
        chain,
        cadence(num_blocks - 1, &|offset| offset + 1 == ROWS_PER_BLOCK),
    );

    for (i, &word) in IV.iter().enumerate() {
        cx.boundary(block.h_in_words.at(i), 0, F::from(u128::from(word)));
    }

    {
        let cs = cx.cs();

        let carry = cs.col(carry.index());
        let chain = cs.col(chain.index());

        for i in 0..STATE_WORDS {
            let h_out = cs.col(block.h_out_words.at(i).index());

            cs.assert_zero_when(carry, cs.next(block.h_out_words.at(i).index()) + h_out);
            cs.assert_zero_when(chain, cs.next(block.h_in_words.at(i).index()) + h_out);
        }
    }

    let final_row = (num_blocks - 1) * ROWS_PER_BLOCK;

    for (i, &word) in padding_block(message_len).iter().enumerate() {
        cx.boundary(block.msg.at(i), final_row, F::from(u128::from(word)));
    }

    for i in 0..STATE_WORDS {
        cx.publish(block.h_out_words.at(i), final_row);
    }

    cx.mount(chiplet.def()?);

    Ok((cx.compile()?, block))
}

fn build_trace(statement: &Statement, calls: &[Sha256Call]) -> errors::Result<ColumnTrace> {
    let mut layout = CpuSha256Block::layout().to_vec();
    layout.extend([ColumnType::Bit; 3]);

    let num_vars = statement.chiplet.num_rows().trailing_zeros() as usize;
    let last = calls.len() - 1;

    let mut tb = TraceBuilder::new_secret(&layout, num_vars)?;

    for (b, call) in calls.iter().enumerate() {
        let first_row = b * ROWS_PER_BLOCK;

        for row in first_row..first_row + ROWS_PER_BLOCK {
            statement.block.write(&mut tb, row, call)?;
        }

        tb.set_bit(CPU_ACTIVE, first_row, Bit::ONE)?;

        for row in first_row..first_row + ROWS_PER_BLOCK - 1 {
            tb.set_bit(CPU_CARRY, row, Bit::ONE)?;
        }

        if b < last {
            tb.set_bit(CPU_CHAIN, first_row + ROWS_PER_BLOCK - 1, Bit::ONE)?;
        }
    }

    let mut trace = tb.build();

    for col in statement.chiplet.trace(calls)?.into_columns() {
        trace.add_column(col)?;
    }

    Ok(trace)
}

fn sha256_calls(message: &[u8]) -> Zeroizing<Vec<Sha256Call>> {
    let blocks = pad_message(message);

    let mut calls = Zeroizing::new(Vec::with_capacity(blocks.len()));
    let mut h = IV;

    for block in blocks.iter() {
        let call = Sha256Call {
            h_in: h,
            block: *block,
        };

        h = call.h_out();

        calls.push(call);
    }

    calls
}

fn digest_words(calls: &[Sha256Call]) -> [u32; STATE_WORDS] {
    match calls.last() {
        Some(call) => call.h_out(),
        None => unreachable!("padding yields at least one block"),
    }
}

fn padding_block(message_len: usize) -> [u32; BLOCK_WORDS] {
    let bits = message_len as u64 * 8;

    let mut words = [0u32; BLOCK_WORDS];

    words[0] = 0x8000_0000;
    words[BLOCK_WORDS - 2] = (bits >> 32) as u32;
    words[BLOCK_WORDS - 1] = bits as u32;

    words
}

#[cfg(test)]
mod tests {
    use super::*;
    use hekate_sha2::digest_bytes;

    fn message(len: usize) -> Vec<u8> {
        (0..len).map(|i| (i % 251) as u8).collect()
    }

    fn digest(message: &[u8]) -> Vec<u8> {
        digest_bytes(&digest_words(&sha256_calls(message))).to_vec()
    }

    #[test]
    fn digest_matches_reference_vector() {
        const SHA256_OF_EMPTY: [u8; DIGEST_BYTES] = [
            0xe3, 0xb0, 0xc4, 0x42, 0x98, 0xfc, 0x1c, 0x14, 0x9a, 0xfb, 0xf4, 0xc8, 0x99, 0x6f,
            0xb9, 0x24, 0x27, 0xae, 0x41, 0xe4, 0x64, 0x9b, 0x93, 0x4c, 0xa4, 0x95, 0x99, 0x1b,
            0x78, 0x52, 0xb8, 0x55,
        ];

        assert_eq!(digest(&[]), SHA256_OF_EMPTY);
    }

    #[test]
    fn two_and_three_block_messages_prove_and_verify() {
        for len in [128, 192] {
            let message = message(len);
            let digest = digest(&message);

            let proof = prove_sha256(message, None)
                .unwrap_or_else(|e| panic!("{len} bytes do not prove: {e}"));

            verify_sha256(proof.program_id, proof.proof_bytes, digest, len as u64)
                .unwrap_or_else(|e| panic!("{len} bytes do not verify: {e}"));
        }
    }

    #[test]
    fn proof_is_rejected_for_another_digest() {
        let message = message(128);

        let mut digest = digest(&message);
        digest[0] ^= 1;

        let proof = prove_sha256(message, None).expect("an honest message proves");

        let err = verify_sha256(proof.program_id, proof.proof_bytes, digest, 128)
            .expect_err("the proof is bound to its digest");

        assert!(matches!(err, VerifyError::Rejected(_)), "{err}");
    }

    #[test]
    fn proof_is_rejected_for_another_length() {
        let message = message(128);
        let digest = digest(&message);

        let proof = prove_sha256(message, None).expect("an honest message proves");

        let err = verify_sha256(proof.program_id, proof.proof_bytes, digest, 256)
            .expect_err("the proof is bound to its message length");

        assert!(matches!(err, VerifyError::Rejected(_)), "{err}");
    }

    #[test]
    fn message_off_block_grid_is_refused() {
        let err = match prove_sha256(message(100), None) {
            Ok(_) => panic!("the prover returned a proof"),
            Err(e) => e,
        };

        assert!(matches!(err, ProveError::InvalidInput(_)), "{err}");
    }

    #[test]
    fn capacity_fills_trace_it_names() {
        let capacity = sha256_capacity(15) as usize;

        let fits = Statement::new(capacity).expect("the capacity is a valid length");
        let spills = Statement::new(capacity + BLOCK_BYTES).expect("a valid length");

        assert_eq!(fits.chiplet.num_rows(), 1 << 15);
        assert_eq!(spills.chiplet.num_rows(), 1 << 16);
    }
}
