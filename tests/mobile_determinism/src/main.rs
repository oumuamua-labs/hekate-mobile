// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 Oumuamua Labs <info@oumuamua.dev>

use hekate_core::config::Config;
use hekate_core::trace::{ColumnType, Trace, TraceBuilder};
use hekate_math::{Bit, Block32, Block128, TowerField};
use hekate_program::constraint::builder::ConstraintSystem;
use hekate_program::constraint::{BoundaryConstraint, ConstraintAst};
use hekate_program::{Air, Program, ProgramInstance, ProgramWitness, define_columns};
use std::process::ExitCode;

type F = Block128;

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

#[derive(Clone)]
struct HarnessAir {
    layout: Vec<ColumnType>,
}

impl HarnessAir {
    fn new() -> Self {
        Self {
            layout: Cols::build_layout(),
        }
    }
}

impl Air<F> for HarnessAir {
    fn num_columns(&self) -> usize {
        Cols::NUM_COLUMNS
    }

    fn column_layout(&self) -> &[ColumnType] {
        &self.layout
    }

    fn boundary_constraints(&self) -> Vec<BoundaryConstraint<F>> {
        vec![BoundaryConstraint::with_public_input(Cols::VALUE, 0, 0)]
    }

    fn constraint_ast(&self) -> ConstraintAst<F> {
        let cs = ConstraintSystem::<F>::new();
        
        let [value, q] = [cs.col(Cols::VALUE), cs.col(Cols::Q)];
        let next_value = cs.next(Cols::VALUE);
        
        cs.constrain(q * (next_value + value));
        
        cs.build()
    }
}

impl Program<F> for HarnessAir {
    fn num_public_inputs(&self) -> usize {
        1
    }
}

fn build_state() -> Result<(HarnessAir, ProgramInstance<F>, ProgramWitness<F>), String> {
    let value = Block32::from(FIXED_VALUE);

    let mut tb =
        TraceBuilder::new(&Cols::build_layout(), NUM_VARS).map_err(|e| format!("trace: {e}"))?;

    for i in 0..NUM_ROWS {
        let selector = if i + 1 < NUM_ROWS {
            Bit::ONE
        } else {
            Bit::ZERO
        };

        tb.set_b32(Cols::VALUE, i, value)
            .map_err(|e| format!("value[{i}]: {e}"))?;
        tb.set_bit(Cols::Q, i, selector)
            .map_err(|e| format!("q[{i}]: {e}"))?;
    }

    let trace = tb.build();
    let public_input = trace
        .get_element::<F>(Cols::VALUE, 0)
        .map_err(|e| format!("public input: {e}"))?
        .to_tower();

    Ok((
        HarnessAir::new(),
        ProgramInstance::new(NUM_ROWS, vec![public_input]),
        ProgramWitness::<F>::new(trace),
    ))
}

fn run(out_path: &str) -> Result<(), String> {
    let (air, instance, witness) = build_state()?;
    let config = Config::default();

    let proof = hekate_prover_sys::prove(
        TRANSCRIPT_LABEL,
        &air,
        &instance,
        &witness,
        &config,
        SEED,
        None,
    )
    .map_err(|e| format!("prove: {e}"))?;

    let bytes = hekate_sdk::serialize_proof_bytes(&proof);
    std::fs::write(out_path, &bytes).map_err(|e| format!("write {out_path}: {e}"))?;

    eprintln!("wrote {} bytes to {out_path}", bytes.len());

    Ok(())
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();
    let out_path = match args.get(1) {
        Some(p) => p.as_str(),
        None => {
            eprintln!("usage: determinism-harness <out-path>");
            return ExitCode::from(2);
        }
    };

    match run(out_path) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("[fail] {e}");
            ExitCode::from(1)
        }
    }
}