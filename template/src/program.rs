//! Replace the contents of this file with your
//! domain-specific Air, Program, and witness
//! builder. The default seam proves knowledge
//! of a 32-bit value pinned to the public input,
//! useful only as a shape demonstration.

use hekate_core::trace::{ColumnType, Trace, TraceBuilder};
use hekate_math::{Bit, Block128, Block32, TowerField};
use hekate_program::constraint::builder::ConstraintSystem;
use hekate_program::constraint::{BoundaryConstraint, ConstraintAst};
use hekate_program::define_columns;
use hekate_program::{Air, Program, ProgramInstance, ProgramWitness};

use crate::inputs::MyInputs;
use crate::ProveError;

pub const TRANSCRIPT_LABEL: &[u8] = b"my-prover-v1";

const NUM_VARS: usize = 4;
const NUM_ROWS: usize = 1 << NUM_VARS;

type F = Block128;

define_columns! {
    Cols {
        VALUE: B32,
        Q: Bit,
    }
}

#[derive(Clone)]
pub struct MyAir {
    layout: Vec<ColumnType>,
}

impl MyAir {
    fn new() -> Self {
        Self {
            layout: Cols::build_layout(),
        }
    }
}

impl Air<F> for MyAir {
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

impl Program<F> for MyAir {
    fn num_public_inputs(&self) -> usize {
        1
    }
}

pub fn build_program_state(
    inputs: &MyInputs,
) -> Result<(MyAir, ProgramInstance<F>, ProgramWitness<F>), ProveError> {
    let value = Block32::from(inputs.value);

    let mut tb = TraceBuilder::new(&Cols::build_layout(), NUM_VARS)
        .map_err(|e| ProveError::Witness(format!("trace builder: {e}")))?;

    for i in 0..NUM_ROWS {
        let selector = if i + 1 < NUM_ROWS {
            Bit::ONE
        } else {
            Bit::ZERO
        };

        tb.set_b32(Cols::VALUE, i, value)
            .map_err(|e| ProveError::Witness(format!("set value[{i}]: {e}")))?;
        tb.set_bit(Cols::Q, i, selector)
            .map_err(|e| ProveError::Witness(format!("set q[{i}]: {e}")))?;
    }

    let trace = tb.build();

    let public_input = trace
        .get_element::<F>(Cols::VALUE, 0)
        .map_err(|e| ProveError::Witness(format!("get public_input: {e}")))?
        .to_tower();

    let air = MyAir::new();
    let instance = ProgramInstance::new(NUM_ROWS, vec![public_input]);
    let witness = ProgramWitness::<F>::new(trace);

    Ok((air, instance, witness))
}
