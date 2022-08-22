use fugue::ir::{AddressValue, IntoAddress};
use fugue::ir::il::Location;
use fugue::ir::il::pcode::{PCode, PCodeOp};

use crate::traits::Interpreter;
use crate::types::{Bound, Branch, BranchOutcome, OrOutcome, Outcome, StepOutcome, StepState};

#[derive(Clone)]
pub struct Machine<I: Interpreter> {
    interpreter: I,
    step_state: StepState,
    ignore_errors: bool,
}

impl<I> From<I> for Machine<I> where I: Interpreter {
    fn from(interpreter: I) -> Self {
        // NO-OP to avoid wrapping in an option in absence of a Default
        // for PCode
        let step_state = StepState::from(PCode::nop(
            AddressValue::new(interpreter.interpreter_space(), 0),
            0,
        ));

        Self {
            interpreter,
            step_state,
            ignore_errors: false,
        }
    }
}

impl<I> Machine<I> where I: Interpreter {
    #[inline(always)]
    pub fn new(interpreter: I) -> Self {
        Self::from(interpreter)
    }

    #[inline(always)]
    pub fn new_with(interpreter: I, ignore_errors: bool) -> Self {
        let mut machine = Self::new(interpreter);
        machine.ignore_errors = ignore_errors;
        machine
    }

    #[deprecated(since = "2.10", note = "use Machine::ignore_errors")]
    pub fn set_ignore_errors(&mut self, ignore_errors: bool) {
        self.ignore_errors = ignore_errors;
    }

    pub fn ignore_errors(&mut self, ignore_errors: bool) {
        self.ignore_errors = ignore_errors;
    }

    pub fn step<L>(&mut self, location: L) -> Result<StepOutcome<I::Outcome>, I::Error>
    where L: Into<Location> {

        let location = location.into();
        let address = location.address();

        match self.interpreter.lift(&*address)? {
            OrOutcome::Branch(location) => return self.step(location),
            OrOutcome::Continue(step_state) => {
                self.step_state = step_state.with_location(&location);
            },
            OrOutcome::Halt(outcome) => return Ok(StepOutcome::Halt(outcome)),
        }

        while let Some(op) = self.step_state.current() {
            match self.interpreter.operation(&self.step_state.location(), op)? {
                OrOutcome::Branch(location) => return self.step(location),
                OrOutcome::Halt(outcome) => return Ok(StepOutcome::Halt(outcome)),
                OrOutcome::Continue(_) => (),
            };

            let action_res = match op {
                PCodeOp::Copy { ref source, ref destination } => {
                    self.interpreter.copy(source, destination)
                },
                PCodeOp::Load { ref source, ref destination, space } => {
                    self.interpreter.load(source, destination, space.clone())
                },
                PCodeOp::Store { ref source, ref destination, space } => {
                    self.interpreter.store(source, destination, space.clone())
                },
                PCodeOp::Branch { ref destination } => {
                    self.interpreter.branch(destination)
                },
                PCodeOp::CBranch { ref destination, ref condition } => {
                    self.interpreter.cbranch(destination, condition)
                },
                PCodeOp::IBranch { ref destination } => {
                    self.interpreter.ibranch(destination)
                },
                PCodeOp::Call { ref destination } => {
                    self.interpreter.call(destination)
                },
                PCodeOp::ICall { ref destination } => {
                    self.interpreter.icall(destination)
                },
                PCodeOp::Intrinsic { name, ref operands, ref result } => {
                    self.interpreter.intrinsic(name, operands.as_ref(), result.as_ref())
                },
                PCodeOp::Return { ref destination } => {
                    self.interpreter.return_(destination)
                },

                PCodeOp::IntEq { ref result, operands: [ref operand1, ref operand2] } => {
                    self.interpreter.int_eq(result, operand1, operand2)
                },
                PCodeOp::IntNotEq { ref result, operands: [ref operand1, ref operand2] } => {
                    self.interpreter.int_not_eq(result, operand1, operand2)
                },
                PCodeOp::IntLess { ref result, operands: [ref operand1, ref operand2] } => {
                    self.interpreter.int_less(result, operand1, operand2)
                },
                PCodeOp::IntLessEq { ref result, operands: [ref operand1, ref operand2] } => {
                    self.interpreter.int_less_eq(result, operand1, operand2)
                },
                PCodeOp::IntSLess { ref result, operands: [ref operand1, ref operand2] } => {
                    self.interpreter.int_sless(result, operand1, operand2)
                },
                PCodeOp::IntSLessEq { ref result, operands: [ref operand1, ref operand2] } => {
                    self.interpreter.int_sless_eq(result, operand1, operand2)
                },

                PCodeOp::IntZExt { ref result, ref operand } => {
                    self.interpreter.int_zext(result, operand)
                },
                PCodeOp::IntSExt { ref result, ref operand } => {
                    self.interpreter.int_sext(result, operand)
                },

                PCodeOp::IntAdd { ref result, operands: [ref operand1, ref operand2] } => {
                    self.interpreter.int_add(result, operand1, operand2)
                },
                PCodeOp::IntSub { ref result, operands: [ref operand1, ref operand2] } => {
                    self.interpreter.int_sub(result, operand1, operand2)
                },
                PCodeOp::IntCarry { ref result, operands: [ref operand1, ref operand2] } => {
                    self.interpreter.int_carry(result, operand1, operand2)
                },
                PCodeOp::IntSCarry { ref result, operands: [ref operand1, ref operand2] } => {
                    self.interpreter.int_scarry(result, operand1, operand2)
                },
                PCodeOp::IntSBorrow { ref result, operands: [ref operand1, ref operand2] } => {
                    self.interpreter.int_sborrow(result, operand1, operand2)
                },

                PCodeOp::IntNeg { ref result, ref operand } => {
                    self.interpreter.int_neg(result, operand)
                },
                PCodeOp::IntNot { ref result, ref operand } => {
                    self.interpreter.int_not(result, operand)
                },

                PCodeOp::IntXor { ref result, operands: [ref operand1, ref operand2] } => {
                    self.interpreter.int_xor(result, operand1, operand2)
                },
                PCodeOp::IntAnd { ref result, operands: [ref operand1, ref operand2] } => {
                    self.interpreter.int_and(result, operand1, operand2)
                },
                PCodeOp::IntOr { ref result, operands: [ref operand1, ref operand2] } => {
                    self.interpreter.int_or(result, operand1, operand2)
                },
                PCodeOp::IntLeftShift { ref result, operands: [ref operand1, ref operand2] } => {
                    self.interpreter.int_left_shift(result, operand1, operand2)
                },
                PCodeOp::IntRightShift { ref result, operands: [ref operand1, ref operand2] } => {
                    self.interpreter.int_right_shift(result, operand1, operand2)
                },
                PCodeOp::IntSRightShift { ref result, operands: [ref operand1, ref operand2] } => {
                    self.interpreter.int_sright_shift(result, operand1, operand2)
                },

                PCodeOp::IntMul { ref result, operands: [ref operand1, ref operand2] } => {
                    self.interpreter.int_mul(result, operand1, operand2)
                },
                PCodeOp::IntDiv { ref result, operands: [ref operand1, ref operand2] } => {
                    self.interpreter.int_div(result, operand1, operand2)
                },
                PCodeOp::IntSDiv { ref result, operands: [ref operand1, ref operand2] } => {
                    self.interpreter.int_sdiv(result, operand1, operand2)
                },
                PCodeOp::IntRem { ref result, operands: [ref operand1, ref operand2] } => {
                    self.interpreter.int_rem(result, operand1, operand2)
                },
                PCodeOp::IntSRem { ref result, operands: [ref operand1, ref operand2] } => {
                    self.interpreter.int_srem(result, operand1, operand2)
                },

                PCodeOp::BoolNot { ref result, ref operand } => {
                    self.interpreter.bool_not(result, operand)
                },
                PCodeOp::BoolXor { ref result, operands: [ref operand1, ref operand2] } => {
                    self.interpreter.bool_xor(result, operand1, operand2)
                },
                PCodeOp::BoolAnd { ref result, operands: [ref operand1, ref operand2] } => {
                    self.interpreter.bool_and(result, operand1, operand2)
                },
                PCodeOp::BoolOr { ref result, operands: [ref operand1, ref operand2] } => {
                    self.interpreter.bool_or(result, operand1, operand2)
                },

                PCodeOp::FloatEq { ref result, operands: [ref operand1, ref operand2] } => {
                    self.interpreter.float_eq(result, operand1, operand2)
                },
                PCodeOp::FloatNotEq { ref result, operands: [ref operand1, ref operand2] } => {
                    self.interpreter.float_not_eq(result, operand1, operand2)
                },
                PCodeOp::FloatLess { ref result, operands: [ref operand1, ref operand2] } => {
                    self.interpreter.float_less(result, operand1, operand2)
                },
                PCodeOp::FloatLessEq { ref result, operands: [ref operand1, ref operand2] } => {
                    self.interpreter.float_less_eq(result, operand1, operand2)
                },

                PCodeOp::FloatIsNaN { ref result, ref operand } => {
                    self.interpreter.float_is_nan(result, operand)
                },

                PCodeOp::FloatAdd { ref result, operands: [ref operand1, ref operand2] } => {
                    self.interpreter.float_add(result, operand1, operand2)
                },
                PCodeOp::FloatDiv { ref result, operands: [ref operand1, ref operand2] } => {
                    self.interpreter.float_div(result, operand1, operand2)
                },
                PCodeOp::FloatMul { ref result, operands: [ref operand1, ref operand2] } => {
                    self.interpreter.float_mul(result, operand1, operand2)
                },
                PCodeOp::FloatSub { ref result, operands: [ref operand1, ref operand2] } => {
                    self.interpreter.float_sub(result, operand1, operand2)
                },

                PCodeOp::FloatNeg { ref result, ref operand } => {
                    self.interpreter.float_neg(result, operand)
                },
                PCodeOp::FloatAbs { ref result, ref operand } => {
                    self.interpreter.float_abs(result, operand)
                },
                PCodeOp::FloatSqrt { ref result, ref operand } => {
                    self.interpreter.float_sqrt(result, operand)
                },

                PCodeOp::FloatOfInt { ref result, ref operand } => {
                    self.interpreter.float_of_int(result, operand)
                },
                PCodeOp::FloatOfFloat { ref result, ref operand } => {
                    self.interpreter.float_of_float(result, operand)
                },

                PCodeOp::FloatTruncate { ref result, ref operand } => {
                    self.interpreter.float_truncate(result, operand)
                },
                PCodeOp::FloatCeiling { ref result, ref operand } => {
                    self.interpreter.float_ceiling(result, operand)
                },
                PCodeOp::FloatFloor { ref result, ref operand } => {
                    self.interpreter.float_floor(result, operand)
                },
                PCodeOp::FloatRound { ref result, ref operand } => {
                    self.interpreter.float_round(result, operand)
                },

                PCodeOp::Subpiece { ref result, ref operand, ref amount } => {
                    self.interpreter.subpiece(result, operand, amount)
                },
                PCodeOp::PopCount { ref result, ref operand } => {
                    self.interpreter.pop_count(result, operand)
                },

                PCodeOp::Skip => {
                    self.interpreter.skip()
                },
            };

            match action_res {
                Err(e) => {
                    if self.ignore_errors {
                        if let BranchOutcome::Global(address) = self.step_state.branch(&Branch::Next) {
                            return Ok(StepOutcome::Branch(address))
                        } else {
                            continue
                        }
                    } else {
                        return Err(e);
                    }
                },
                Ok(action) => {
                    match action {
                        Outcome::Halt(outcome) => {
                            return Ok(StepOutcome::Halt(outcome))
                        },
                        Outcome::Branch(ref branch) => if let BranchOutcome::Global(address) = self.step_state.branch(branch) {
                            return Ok(StepOutcome::Branch(address))
                        } else {
                            continue
                        },
                    }
                },

            }
        }

        Ok(StepOutcome::Branch(self.step_state.fallthrough()))
    }

    pub fn step_until<L, B>(&mut self, location: L, until: Bound<B>) -> Result<(Bound<AddressValue>, StepOutcome<I::Outcome>), I::Error>
        where L: Into<Location>,
              B: IntoAddress {

        let space = self.interpreter.interpreter_space();
        let mut bound = until.in_space(&*space);
        let mut location = location.into();

        // Check if still within bound
        while !bound.reached(&*location.address()) {
            bound = bound.deplete(&*location.address());
            // Execute the instruction at the current location
            match self.step(location)? {
                StepOutcome::Branch(next_address) => {
                    location = Location::from(next_address);
                },
                v => {
                    return Ok((bound, v))
                }
            }
        }

        Ok((bound, StepOutcome::Reached))
    }

    pub fn step_state(&self) -> &StepState {
        &self.step_state
    }

    pub fn step_state_mut(&mut self) -> &mut StepState {
        &mut self.step_state
    }

    pub fn interpreter(&self) -> &I {
        &self.interpreter
    }

    pub fn interpreter_mut(&mut self) -> &mut I {
        &mut self.interpreter
    }
}
