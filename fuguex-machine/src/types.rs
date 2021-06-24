use fugue::ir::Address;
use fugue::ir::il::pcode::{PCode, PCodeOp};

use fuguex_state::{State, StateValue};

pub enum Branch<'space> {
    Next,
    Local(isize),
    Global(Address<'space>),
}

pub enum Outcome<'space> {
    Halt,
    Branch(Branch<'space>),
}

pub enum StepOutcome<'space> {
    Halt,
    Branch(Address<'space>),
}

pub struct StepState<'space> {
    pcode: PCode<'space>,
    position: usize,
}

pub enum BranchOutcome<'space> {
    Local,
    Global(Address<'space>),
}

impl<'space> From<PCode<'space>> for StepState<'space> {
    fn from(pcode: PCode<'space>) -> Self {
        Self {
            pcode,
            position: 0,
        }
    }
}

impl<'space> StepState<'space> {
    #[inline(always)]
    pub fn address(&self) -> Address<'space> {
        self.pcode.address()
    }

    #[inline(always)]
    pub fn current(&self) -> Option<&PCodeOp<'space>> {
        self.pcode.operations().get(self.position)
    }

    pub fn fallthrough(&self) -> Address<'space> {
        self.address() + self.pcode.length()
    }

    pub fn branch(&mut self, action: &Branch<'space>) -> BranchOutcome<'space> {
        match action {
            Branch::Next => {
                self.position += 1;
            },
            Branch::Local(offset) => {
                if offset.is_negative() {
                    let abs = offset.wrapping_abs() as usize;
                    if abs <= self.position {
                        self.position -= abs;
                    } else {
                        panic!("negative local branch out of range")
                    }
                } else {
                    self.position += *offset as usize;
                }
            },
            Branch::Global(address) => {
                return BranchOutcome::Global(*address)
            }
        }

        if self.position < self.pcode.operations.len() {
            BranchOutcome::Local
        } else {
            BranchOutcome::Global(self.fallthrough())
        }
    }
}
