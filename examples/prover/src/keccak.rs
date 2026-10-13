// SPDX-FileCopyrightText: 2026 Andrei Kochergin <andrei@oumuamua.dev>
// SPDX-FileCopyrightText: 2026 Oumuamua Labs <info@oumuamua.dev>
// SPDX-License-Identifier: AGPL-3.0-only

use hekate_core::errors;
use hekate_core::trace::{ColumnTrace, ColumnType, TraceBuilder};
use hekate_keccak::{
    CpuKeccakColumns, KeccakCall, KeccakChiplet, KeccakSpongeNative, generate_keccak_trace,
};
use hekate_math::{Bit, Block64, TowerField};
use hekate_program::chiplet::ChipletDef;
use hekate_program::circuit::{Circuit, CircuitProgram, Col};
use hekate_program::{FixedShape, ProgramInstance, ProgramWitness};
use std::sync::Arc;
use std::time::{Duration, Instant};
use zeroize::Zeroizing;

use crate::{
    CancelToken, F, MIN_NUM_VARS, Proof, ProveError, VerifyError, ensure_not_cancelled, guard,
    run_prover, run_verifier,
};

const TRANSCRIPT_LABEL: &[u8] = b"hekate-mobile-example-keccak256-v1";

const RATE_BYTES: usize = 136;
const RATE_LANES: usize = RATE_BYTES / 8;
const LANE_BYTES: usize = 8;
const DOMAIN_SEPARATOR: u8 = 0x01;
const STATE_LANES: usize = 25;
const DIGEST_LANES: usize = 4;
const DIGEST_BYTES: usize = DIGEST_LANES * LANE_BYTES;

const OUT_GATE: usize = CpuKeccakColumns::NUM_COLUMNS;
const PAD_SEAM: usize = CpuKeccakColumns::NUM_COLUMNS + 1;

struct Statement {
    program: CircuitProgram<F>,
    num_rows: usize,
}

impl Statement {
    fn new(message_len: usize) -> Result<Self, String> {
        if !message_len.is_multiple_of(LANE_BYTES) {
            return Err(format!(
                "message is {message_len} bytes, the program takes a multiple of {LANE_BYTES}"
            ));
        }

        let num_blocks = message_len / RATE_BYTES + 1;

        let num_rows = (KeccakChiplet::BLOCK_ROWS * num_blocks)
            .next_power_of_two()
            .max(1 << MIN_NUM_VARS);

        let program = build_program(num_rows, num_blocks, message_len)
            .map_err(|e| format!("build statement: {e}"))?;

        Ok(Self { program, num_rows })
    }

    fn instance(&self, lanes: [u64; DIGEST_LANES]) -> ProgramInstance<F> {
        let public = lanes.map(|lane| F::from(Block64::from(lane))).to_vec();

        ProgramInstance::new(self.num_rows, public)
    }
}

#[uniffi::export]
pub fn prove_keccak(
    message: Vec<u8>,
    cancel: Option<Arc<CancelToken>>,
) -> Result<Proof, ProveError> {
    guard(ProveError::Panic, || prove(message, cancel.as_deref()))
}

#[uniffi::export]
pub fn verify_keccak(
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
pub fn keccak256(message: Vec<u8>) -> Vec<u8> {
    let message = Zeroizing::new(message);

    digest_bytes(digest_lanes(&sponge_calls(&message))).to_vec()
}

#[uniffi::export]
pub fn keccak_capacity(log_rows: u8) -> u64 {
    1usize
        .checked_shl(u32::from(log_rows))
        .map(|rows| rows / KeccakChiplet::BLOCK_ROWS)
        .and_then(|blocks| blocks.checked_mul(RATE_BYTES))
        .and_then(|bytes| bytes.checked_sub(LANE_BYTES))
        .map_or(0, |bytes| bytes as u64)
}

fn prove(message: Vec<u8>, cancel: Option<&CancelToken>) -> Result<Proof, ProveError> {
    ensure_not_cancelled(cancel)?;

    let message = Zeroizing::new(message);
    let statement = Statement::new(message.len()).map_err(ProveError::InvalidInput)?;

    let witness_start = Instant::now();

    let calls = sponge_calls(&message);

    let trace =
        build_trace(&calls, statement.num_rows).map_err(|e| ProveError::Witness(e.to_string()))?;

    let witness_time = witness_start.elapsed();

    run_prover(
        TRANSCRIPT_LABEL,
        &statement.program,
        &statement.instance(digest_lanes(&calls)),
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

    let lanes = core::array::from_fn(|i| {
        let mut lane = [0u8; LANE_BYTES];
        lane.copy_from_slice(&digest[i * LANE_BYTES..(i + 1) * LANE_BYTES]);

        u64::from_le_bytes(lane)
    });

    run_verifier(
        TRANSCRIPT_LABEL,
        program_id,
        &statement.program,
        &statement.instance(lanes),
        proof_bytes,
    )
}

fn build_program(
    num_rows: usize,
    num_blocks: usize,
    message_len: usize,
) -> errors::Result<CircuitProgram<F>> {
    let mut cx = Circuit::<F>::new("Keccak256Preimage", num_rows)?;

    let cpu = cx.schema(&CpuKeccakColumns::build_layout());
    let selector = cpu.at(CpuKeccakColumns::SELECTOR);

    let lane = |i: usize| cpu.at(CpuKeccakColumns::LANES + i);

    let call_values: Vec<Col> = (0..STATE_LANES).map(lane).collect();

    cx.call(&KeccakChiplet::service(), &call_values, selector)?;

    let stride = KeccakChiplet::BLOCK_ROWS;

    cx.fix(
        selector,
        KeccakChiplet::host_selector_shape(stride, num_blocks),
    );

    let emit_output = cx.column(ColumnType::Bit);

    cx.fix(
        emit_output,
        FixedShape::Cadence {
            stride,
            count: num_blocks,
            origin: 0,
            values: (0..stride)
                .map(|offset| match offset == stride - 1 {
                    true => F::ONE,
                    false => F::ZERO,
                })
                .collect(),
        },
    );

    let seam = cx.column(ColumnType::Bit);

    let seam_rows = match num_blocks {
        1 => Vec::new(),
        _ => vec![(pad_seam_row(num_blocks), F::ONE)],
    };

    cx.fix(seam, FixedShape::Sparse(seam_rows));

    cx.mount(ChipletDef::from_air(&KeccakChiplet::new(
        num_rows, num_blocks,
    ))?);

    for i in RATE_LANES..STATE_LANES {
        cx.boundary(lane(i), 0, F::ZERO);
    }

    let cs = cx.cs();
    let carry = cs.col(emit_output.index());

    for i in RATE_LANES..STATE_LANES {
        let col = lane(i).index();
        cs.assert_zero_when(carry, cs.next(col) + cs.col(col));
    }

    match num_blocks {
        1 => {
            for (i, value) in pad_lanes(message_len) {
                cx.boundary(lane(i), 0, F::from(Block64::from(value)));
            }
        }
        _ => {
            let cs = cx.cs();
            let gate = cs.col(seam.index());

            for (i, value) in pad_lanes(message_len) {
                let value = cs.constant(F::from(Block64::from(value)));
                let col = lane(i).index();

                cs.assert_zero_when(gate, cs.next(col) + cs.col(col) + value);
            }
        }
    }

    let last_output_row = KeccakChiplet::BLOCK_ROWS * num_blocks - 1;

    for i in 0..DIGEST_LANES {
        cx.publish(lane(i), last_output_row);
    }

    cx.compile()
}

fn build_trace(calls: &[KeccakCall], num_rows: usize) -> errors::Result<ColumnTrace> {
    let mut layout = CpuKeccakColumns::build_layout();
    layout.extend([ColumnType::Bit; 2]);

    let mut tb = TraceBuilder::new_secret(&layout, num_rows.trailing_zeros() as usize)?;
    let mut row = 0;

    for (input, output) in calls {
        let output_row = row + KeccakChiplet::BLOCK_ROWS - 1;

        for (i, &lane) in input.iter().enumerate() {
            tb.set_b64(i, row, Block64::from(lane))?;
        }

        tb.set_bit(CpuKeccakColumns::SELECTOR, row, Bit::ONE)?;

        for (i, &lane) in output.iter().enumerate() {
            tb.set_b64(i, output_row, Block64::from(lane))?;
        }

        tb.set_bit(CpuKeccakColumns::SELECTOR, output_row, Bit::ONE)?;
        tb.set_bit(OUT_GATE, output_row, Bit::ONE)?;

        row = output_row + 1;
    }

    let last_output = last_state(calls);

    for (lane, &value) in last_output.iter().enumerate().skip(RATE_LANES) {
        tb.set_b64(lane, row, Block64::from(value))?;
    }

    if calls.len() > 1 {
        tb.set_bit(PAD_SEAM, pad_seam_row(calls.len()), Bit::ONE)?;
    }

    let mut trace = tb.build();

    let inputs: Zeroizing<Vec<[Block64; STATE_LANES]>> = Zeroizing::new(
        calls
            .iter()
            .map(|(input, _)| input.map(Block64::from))
            .collect(),
    );

    for col in generate_keccak_trace(&inputs, num_rows)?.into_columns() {
        trace.add_column(col)?;
    }

    Ok(trace)
}

fn sponge_calls(message: &[u8]) -> Zeroizing<Vec<KeccakCall>> {
    let mut sponge = KeccakSpongeNative::new();
    sponge.absorb(message, RATE_BYTES, DOMAIN_SEPARATOR);

    sponge.into_calls()
}

fn last_state(calls: &[KeccakCall]) -> [u64; STATE_LANES] {
    match calls.last() {
        Some((_, state)) => *state,
        None => unreachable!("pad10*1 absorbs at least one block"),
    }
}

fn digest_lanes(calls: &[KeccakCall]) -> [u64; DIGEST_LANES] {
    let state = last_state(calls);

    core::array::from_fn(|i| state[i])
}

fn digest_bytes(lanes: [u64; DIGEST_LANES]) -> [u8; DIGEST_BYTES] {
    let mut out = [0u8; DIGEST_BYTES];
    for (chunk, lane) in out.as_chunks_mut::<LANE_BYTES>().0.iter_mut().zip(lanes) {
        *chunk = lane.to_le_bytes();
    }

    out
}

fn pad_seam_row(num_blocks: usize) -> usize {
    KeccakChiplet::BLOCK_ROWS * (num_blocks - 1) - 1
}

fn pad_lanes(message_len: usize) -> impl Iterator<Item = (usize, u64)> {
    let first = (message_len % RATE_BYTES) / LANE_BYTES;

    (first..RATE_LANES).map(move |lane| {
        let mut value = 0u64;

        if lane == first {
            value |= u64::from(DOMAIN_SEPARATOR);
        }

        if lane == RATE_LANES - 1 {
            value |= 0x80u64 << 56;
        }

        (lane, value)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const KECCAK256_OF_EMPTY: [u8; DIGEST_BYTES] = [
        0xc5, 0xd2, 0x46, 0x01, 0x86, 0xf7, 0x23, 0x3c, 0x92, 0x7e, 0x7d, 0xb2, 0xdc, 0xc7, 0x03,
        0xc0, 0xe5, 0x00, 0xb6, 0x53, 0xca, 0x82, 0x27, 0x3b, 0x7b, 0xfa, 0xd8, 0x04, 0x5d, 0x85,
        0xa4, 0x70,
    ];

    fn message(len: usize) -> Vec<u8> {
        (0..len).map(|i| (i % 251) as u8).collect()
    }

    #[test]
    fn digest_matches_the_reference_vector() {
        assert_eq!(keccak256(Vec::new()), KECCAK256_OF_EMPTY);
    }

    #[test]
    fn one_and_two_block_messages_prove_and_verify() {
        for len in [128, 256] {
            let message = message(len);
            let digest = keccak256(message.clone());

            let proof = prove_keccak(message, None)
                .unwrap_or_else(|e| panic!("{len} bytes do not prove: {e}"));

            verify_keccak(proof.program_id, proof.proof_bytes, digest, len as u64)
                .unwrap_or_else(|e| panic!("{len} bytes do not verify: {e}"));
        }
    }

    #[test]
    fn proof_is_rejected_for_another_digest() {
        let message = message(128);

        let mut digest = keccak256(message.clone());
        digest[0] ^= 1;

        let proof = prove_keccak(message, None).expect("an honest message proves");

        let err = verify_keccak(proof.program_id, proof.proof_bytes, digest, 128)
            .expect_err("the proof is bound to its digest");

        assert!(matches!(err, VerifyError::Rejected(_)), "{err}");
    }

    #[test]
    fn proof_is_rejected_for_another_length() {
        let message = message(128);
        let digest = keccak256(message.clone());

        let proof = prove_keccak(message, None).expect("an honest message proves");

        let err = verify_keccak(proof.program_id, proof.proof_bytes, digest, 256)
            .expect_err("the proof is bound to its message length");

        assert!(matches!(err, VerifyError::Rejected(_)), "{err}");
    }

    #[test]
    fn message_off_lane_grid_is_refused() {
        let err = match prove_keccak(message(127), None) {
            Ok(_) => panic!("the prover returned a proof"),
            Err(e) => e,
        };

        assert!(matches!(err, ProveError::InvalidInput(_)), "{err}");
    }

    #[test]
    fn capacity_fills_trace_it_names() {
        let capacity = keccak_capacity(15) as usize;

        let fits = Statement::new(capacity).expect("the capacity is a valid length");
        let spills = Statement::new(capacity + LANE_BYTES).expect("a valid length");

        assert_eq!(fits.num_rows, 1 << 15);
        assert_eq!(spills.num_rows, 1 << 16);
    }
}
