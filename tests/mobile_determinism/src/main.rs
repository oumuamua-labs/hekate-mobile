// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 Oumuamua Labs <info@oumuamua.dev>

use hekate_core::config::Config;
use hekate_core::proofs::InnerProof;
use hekate_core::trace::TraceBuilder;
use hekate_crypto::DefaultHasher;
use hekate_crypto::transcript::Transcript;
use hekate_math::{Bit, Block32, Block128, TowerField};
use hekate_program::circuit::{Circuit, CircuitProgram};
use hekate_program::digest::program_id;
use hekate_program::{FixedShape, ProgramInstance, ProgramWitness, define_columns};
use hekate_verifier::HekateVerifier;
use std::process::ExitCode;

type F = Block128;
type H = DefaultHasher;

const TRANSCRIPT_LABEL: &[u8] = b"hekate-mobile-determinism-v1";
const SEED: [u8; 32] = [0u8; 32];
const FIXED_VALUE: u32 = 42;
const NUM_VARS: usize = 15;
const NUM_ROWS: usize = 1 << NUM_VARS;

define_columns! {
    Cols {
        VALUE: B32,
        Q: Bit,
    }
}

fn build_program() -> Result<CircuitProgram<F>, String> {
    let mut cx =
        Circuit::<F>::new("HarnessProgram", NUM_ROWS).map_err(|e| format!("circuit: {e}"))?;

    let cols = cx.schema(&Cols::build_layout());

    let cs = cx.cs();
    let q = cs.col(cols.at(Cols::Q).index());
    let value = cs.col(cols.at(Cols::VALUE).index());

    cs.constrain(q * (cs.next(cols.at(Cols::VALUE).index()) + value));

    cx.fix(cols.at(Cols::Q), FixedShape::LastRow);
    cx.publish(cols.at(Cols::VALUE), 0);

    cx.compile().map_err(|e| format!("compile: {e}"))
}

fn instance() -> ProgramInstance<F> {
    ProgramInstance::new(NUM_ROWS, vec![F::from(u128::from(FIXED_VALUE))])
}

fn build_witness() -> Result<ProgramWitness<F>, String> {
    let value = Block32::from(FIXED_VALUE);

    let mut tb =
        TraceBuilder::new(&Cols::build_layout(), NUM_VARS).map_err(|e| format!("trace: {e}"))?;

    for i in 0..NUM_ROWS {
        let selector = match i + 1 < NUM_ROWS {
            true => Bit::ONE,
            false => Bit::ZERO,
        };

        tb.set_b32(Cols::VALUE, i, value)
            .map_err(|e| format!("value[{i}]: {e}"))?;
        tb.set_bit(Cols::Q, i, selector)
            .map_err(|e| format!("q[{i}]: {e}"))?;
    }

    Ok(ProgramWitness::<F>::new(tb.build()))
}

fn verify(air: &CircuitProgram<F>, proof: &InnerProof<F>) -> Result<(), String> {
    let id = program_id::<F, _>(air).map_err(|e| format!("program id: {e}"))?;
    let mut transcript = Transcript::<H>::new(TRANSCRIPT_LABEL);

    let accepted = HekateVerifier::<F, H>::verify(
        &id,
        air,
        &instance(),
        proof,
        &mut transcript,
        &Config::default(),
    )
    .map_err(|e| format!("verify: {e}"))?;

    match accepted {
        true => Ok(()),
        false => Err(String::from("verifier rejected the proof")),
    }
}

fn prove_to(out_path: &str) -> Result<(), String> {
    let air = build_program()?;

    let proof = hekate_prover_sys::prove(
        TRANSCRIPT_LABEL,
        &air,
        &instance(),
        &build_witness()?,
        &Config::default(),
        SEED,
        None,
    )
    .map_err(|e| format!("prove: {e}"))?;

    verify(&air, &proof)?;

    let bytes = hekate_sdk::serialize_proof_bytes(&proof);
    std::fs::write(out_path, &bytes).map_err(|e| format!("write {out_path}: {e}"))?;

    eprintln!("verified; wrote {} bytes to {out_path}", bytes.len());

    Ok(())
}

fn verify_file(proof_path: &str) -> Result<(), String> {
    let bytes = std::fs::read(proof_path).map_err(|e| format!("read {proof_path}: {e}"))?;

    let proof = hekate_sdk::deserialize_proof::<F>(&bytes)
        .map_err(|e| format!("deserialize {proof_path}: {e}"))?;

    verify(&build_program()?, &proof).map_err(|e| format!("{proof_path}: {e}"))?;

    eprintln!("verified {} bytes from {proof_path}", bytes.len());

    Ok(())
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();

    let result = match (args.get(1).map(String::as_str), args.get(2)) {
        (Some("prove"), Some(out_path)) => prove_to(out_path),
        (Some("verify"), Some(proof_path)) => verify_file(proof_path),
        _ => {
            eprintln!("usage: determinism-harness prove <out-path> | verify <proof-path>");
            return ExitCode::from(2);
        }
    };

    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("[fail] {e}");
            ExitCode::from(1)
        }
    }
}
