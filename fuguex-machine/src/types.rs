use fugue::ir::{AddressSpace, AddressValue, IntoAddress};
use fugue::ir::il::Location;
use fugue::ir::il::pcode::{PCode, PCodeOp};

use std::sync::Arc;

#[derive(Debug, Clone)]
#[derive(serde::Deserialize, serde::Serialize)]
pub enum Bound<A: IntoAddress> {
    Address(A),
    Steps(usize),
    Unbounded,
}

impl<A> Bound<A> where A: IntoAddress {
    pub fn address(address: A) -> Bound<A> {
        Self::Address(address)
    }

    pub fn in_space(self, space: &AddressSpace) -> Bound<AddressValue> {
        match self {
            Self::Address(address) => Bound::Address(address.into_address_value(&*space)),
            Self::Steps(steps) => Bound::Steps(steps),
            Self::Unbounded => Bound::Unbounded,
        }
    }
}

impl Bound<AddressValue> {
    pub fn steps(steps: usize) -> Bound<AddressValue> {
        Self::Steps(steps)
    }

    pub fn unbounded() -> Bound<AddressValue> {
        Self::Unbounded
    }

    // Decrease step count
    // Used for counting down from the specified step count
    pub fn deplete(self) -> Self {
        if let Self::Steps(steps) = self {
            Self::Steps(steps.checked_sub(1).unwrap_or(0))
        } else {
            self
        }
    }

    pub fn reached(&self, address: &AddressValue) -> bool {
        match self {
            Self::Address(ref target) => target == address,
            Self::Steps(steps) => *steps == 0,
            Self::Unbounded => false,
        }
    }
}

#[derive(Debug, Clone)]
#[derive(serde::Deserialize, serde::Serialize)]
pub enum Branch {
    Next,
    Local(isize),
    Global(AddressValue),
}

#[derive(Debug, Clone)]
#[derive(serde::Deserialize, serde::Serialize)]
pub enum Outcome<R> {
    Halt(R),
    Branch(Branch),
}

#[derive(Debug, Clone)]
#[derive(serde::Deserialize, serde::Serialize)]
pub enum OrOutcome<S, R> {
    Halt(R),
    Branch(Location),
    Continue(S),
}

impl<T, R> From<T> for OrOutcome<T, R> {
    fn from(t: T) -> Self {
        Self::Continue(t)
    }
}

#[derive(Debug, Clone)]
#[derive(serde::Deserialize, serde::Serialize)]
pub enum StepOutcome<R> {
    Halt(R),
    Reached,
    Branch(AddressValue),
}

#[derive(Debug, Clone)]
#[derive(serde::Deserialize, serde::Serialize)]
pub enum BranchOutcome {
    Local,
    Global(AddressValue),
}

#[derive(Debug, Clone)]
#[derive(serde::Deserialize, serde::Serialize)]
pub struct StepState {
    pcode: Arc<PCode>,
    position: usize,
}

impl From<PCode> for StepState {
    fn from(pcode: PCode) -> Self {
        Self {
            pcode: Arc::new(pcode),
            position: 0,
        }
    }
}

impl StepState {
    #[inline(always)]
    pub fn address(&self) -> AddressValue {
        self.pcode.address()
    }

    pub fn location(&self) -> Location {
        Location::new(self.pcode.address(), self.position)
    }

    pub fn operations(&self) -> &PCode {
        &*self.pcode
    }

    pub fn with_location(self, location: &Location) -> Self {
        assert_eq!(self.pcode.address, *location.address());
        Self { position: location.position(), ..self }
    }

    #[inline(always)]
    pub fn current(&self) -> Option<&PCodeOp> {
        self.pcode.operations().get(self.position)
    }

    pub fn fallthrough(&self) -> AddressValue {
        self.address() + self.pcode.length()
    }

    pub fn branch(&mut self, action: &Branch) -> BranchOutcome {
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
                return BranchOutcome::Global(address.clone())
            }
        }

        if self.position < self.pcode.operations.len() {
            BranchOutcome::Local
        } else {
            BranchOutcome::Global(self.fallthrough())
        }
    }

    pub fn branch_location(&self, action: Branch) -> Location {
        match action {
            Branch::Next => {
                if self.position + 1 < self.pcode.operations.len() {
                    Location::new(self.address(), self.position + 1)
                } else {
                    Location::from(self.fallthrough())
                }
            },
            Branch::Local(offset) => {
                let npos = if offset.is_negative() {
                    let abs = offset.wrapping_abs() as usize;
                    if abs <= self.position {
                        self.position - abs
                    } else {
                        panic!("negative local branch out of range")
                    }
                } else {
                    self.position + offset as usize
                };

                if npos < self.pcode.operations.len() {
                    Location::new(self.address(), npos)
                } else {
                    Location::from(self.fallthrough())
                }
            },
            Branch::Global(address) => {
                Location::from(address)
            }
        }
    }
}
