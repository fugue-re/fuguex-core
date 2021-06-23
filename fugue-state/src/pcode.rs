use fugue::ir::il::pcode::Operand;
use fugue::ir::{IntoAddress, Translator};

use thiserror::Error;

use crate::paged::{self, PagedState};
use crate::register::{self, RegisterState};
use crate::unique::{self, UniqueState};

use crate::traits::{State, StateValue};
use crate::traits::{FromStateValues, IntoStateValues};

#[derive(Debug, Error)]
pub enum Error<'space> {
    #[error(transparent)]
    Memory(paged::Error<'space>),
    #[error(transparent)]
    Register(register::Error<'space>),
    #[error(transparent)]
    Temporary(unique::Error<'space>),
}

#[derive(Debug, Clone)]
pub struct PCodeState<'space, T: StateValue> {
    memory: PagedState<'space, T>,
    registers: RegisterState<'space, T>,
    temporaries: UniqueState<'space, T>,
}

impl<'space, T: StateValue> AsRef<Self> for PCodeState<'space, T> {
    fn as_ref(&self) -> &Self {
        self
    }
}

impl<'space, T: StateValue> AsMut<Self> for PCodeState<'space, T> {
    fn as_mut(&mut self) -> &mut Self {
        self
    }
}

impl<'space, T: StateValue> PCodeState<'space, T> {
    pub fn new(memory: PagedState<'space, T>, translator: &'space Translator) -> Self {
        Self {
            memory,
            registers: RegisterState::new(translator),
            temporaries: UniqueState::new(translator),
        }
    }

    pub fn memory(&self) -> &PagedState<'space, T> {
        &self.memory
    }

    pub fn memory_mut(&mut self) -> &mut PagedState<'space, T> {
        &mut self.memory
    }

    pub fn registers(&self) -> &RegisterState<'space, T> {
        &self.registers
    }

    pub fn registers_mut(&mut self) -> &mut RegisterState<'space, T> {
        &mut self.registers
    }

    pub fn temporaries(&self) -> &UniqueState<'space, T> {
        &self.temporaries
    }

    pub fn temporaries_mut(&mut self) -> &mut UniqueState<'space, T> {
        &mut self.temporaries
    }

    pub fn get_operand<V: FromStateValues<T>>(&self, operand: &Operand<'space>) -> Result<V, Error<'space>> {
        match operand {
            Operand::Address { value, size } => {
                assert_eq!(*size, std::mem::size_of::<V>());

                self.memory()
                    .view_values(*value, *size)
                    .map_err(Error::Memory)
                    .map(V::from_values)
            },
            Operand::Constant { value, size, .. } => {
                assert_eq!(*size, std::mem::size_of::<V>());

                // max size of value
                let mut values: [T; 8] = Default::default();

                assert_eq!(std::mem::size_of_val(&values), std::mem::size_of_val(value));

                let bytes = value.to_ne_bytes();

                for (d, s) in values[..*size].iter_mut().zip(bytes[..*size].iter()) {
                    *d = T::from_byte(*s);
                }

                Ok(V::from_values(&values[..*size]))
            },
            Operand::Register { offset, size, .. } => {
                assert_eq!(*size, std::mem::size_of::<V>());

                self.registers()
                    .view_values(*offset, *size)
                    .map_err(Error::Register)
                    .map(V::from_values)
            },
            Operand::Variable { offset, size, .. } => {
                assert_eq!(*size, std::mem::size_of::<V>());

                self.temporaries()
                    .view_values(*offset, *size)
                    .map_err(Error::Temporary)
                    .map(V::from_values)
            },
        }
    }

    pub fn set_operand<V: IntoStateValues<T>>(&mut self, operand: &Operand<'space>, value: V) -> Result<(), Error<'space>> {
        match operand {
            Operand::Address { value: address, size } => {
                assert_eq!(*size, std::mem::size_of::<V>());
                let view = self.memory_mut()
                    .view_values_mut(*address, *size)
                    .map_err(Error::Memory)?;
                value.into_values(view);
            },
            Operand::Register { offset, size, .. } => {
                assert_eq!(*size, std::mem::size_of::<V>());
                let view = self.registers_mut()
                    .view_values_mut(*offset, *size)
                    .map_err(Error::Register)?;
                value.into_values(view);
            },
            Operand::Variable { offset, size, .. } => {
                assert_eq!(*size, std::mem::size_of::<V>());
                let view = self.temporaries_mut()
                    .view_values_mut(*offset, *size)
                    .map_err(Error::Temporary)?;
                value.into_values(view);
            },
            Operand::Constant { .. } => {
                panic!("cannot assign to Operand::Constant");
            }
        }
        Ok(())
    }
}

impl<'space, V: StateValue> State for PCodeState<'space, V> {
    type Error = Error<'space>;
    type Value = V;

    fn fork(&self) -> Self {
        Self {
            registers: self.registers.fork(),
            temporaries: self.temporaries.fork(),
            memory: self.memory.fork(),
        }
    }

    fn restore(&mut self, other: &Self) {
        self.registers.restore(&other.registers);
        self.temporaries.restore(&other.temporaries);
        self.memory.restore(&other.memory);
    }

    #[inline(always)]
    fn copy_values<F, T>(&mut self, from: F, to: T, size: usize) -> Result<(), Self::Error>
    where F: IntoAddress,
          T: IntoAddress {
        self.memory.copy_values(from, to, size)
            .map_err(Error::Memory)
    }

    #[inline(always)]
    fn get_values<A>(&self, address: A, values: &mut [Self::Value]) -> Result<(), Self::Error>
    where A: IntoAddress {
        self.memory.get_values(address, values)
            .map_err(Error::Memory)
    }

    #[inline(always)]
    fn view_values<A>(&self, address: A, size: usize) -> Result<&[Self::Value], Self::Error>
    where A: IntoAddress {
        self.memory.view_values(address, size)
            .map_err(Error::Memory)
    }

    #[inline(always)]
    fn view_values_mut<A>(&mut self, address: A, size: usize) -> Result<&mut [Self::Value], Self::Error>
    where A: IntoAddress {
        self.memory.view_values_mut(address, size)
            .map_err(Error::Memory)
    }

    #[inline(always)]
    fn set_values<A>(&mut self, address: A, values: &[Self::Value]) -> Result<(), Self::Error>
    where A: IntoAddress {
        self.memory.set_values(address, values)
            .map_err(Error::Memory)
    }

    #[inline(always)]
    fn len(&self) -> usize {
        self.memory.len()
    }
}
