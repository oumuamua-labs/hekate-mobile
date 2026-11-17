//! Replace with your domain circuit and witness
//! builder. The default seam proves knowledge of
//! a 32-bit value pinned to the public input.

use hekate_core::errors;
use hekate_core::trace::TraceBuilder;
use hekate_math::{Bit, Block32, Block256, TowerField};
use hekate_program::circuit::{Circuit, CircuitProgram};
use hekate_program::define_columns;
use hekate_program::{FixedShape, ProgramInstance, ProgramWitness};

use crate::ProveError;
use crate::inputs::MyInputs;

pub const TRANSCRIPT_LABEL: &[u8] = b"my-prover-v1";

/// Production `Config` sets no row floor: the
/// commitment hides its openings at every height.
const NUM_VARS: usize = 12;
const NUM_ROWS: usize = 1 << NUM_VARS;

type F = Block256;

define_columns! {
    Cols {
        VALUE: B32,
        Q: Bit,
    }
}

pub fn build_program() -> errors::Result<CircuitProgram<F>> {
    let mut cx = Circuit::<F>::new("MyProgram", NUM_ROWS)?;

    let cols = cx.schema(&Cols::build_layout());

    let cs = cx.cs();

    let q = cs.col(cols.at(Cols::Q).index());
    let value = cs.col(cols.at(Cols::VALUE).index());

    cs.constrain(q * (cs.next(cols.at(Cols::VALUE).index()) + value));

    cx.fix(cols.at(Cols::Q), FixedShape::LastRow);
    cx.publish(cols.at(Cols::VALUE), 0);

    cx.compile()
}

pub fn build_witness(inputs: &MyInputs) -> Result<ProgramWitness<F>, ProveError> {
    let value = Block32::from(inputs.value);

    let mut tb = TraceBuilder::new_secret(&Cols::build_layout(), NUM_VARS)
        .map_err(|e| ProveError::Witness(format!("trace builder: {e}")))?;

    for i in 0..NUM_ROWS {
        let selector = match i + 1 < NUM_ROWS {
            true => Bit::ONE,
            false => Bit::ZERO,
        };

        tb.set_b32(Cols::VALUE, i, value)
            .map_err(|e| ProveError::Witness(format!("set value[{i}]: {e}")))?;
        tb.set_bit(Cols::Q, i, selector)
            .map_err(|e| ProveError::Witness(format!("set q[{i}]: {e}")))?;
    }

    Ok(ProgramWitness::<F>::new(tb.build()))
}

pub fn instance(inputs: &MyInputs) -> ProgramInstance<F> {
    ProgramInstance::new(NUM_ROWS, vec![F::from(u128::from(inputs.value))])
}
