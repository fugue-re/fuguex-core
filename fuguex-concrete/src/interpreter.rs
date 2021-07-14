use std::marker::PhantomData;
use std::sync::Arc;

use fnv::FnvHashMap as Map;
use fugue::db::Database;
use parking_lot::{RwLock, RwLockReadGuard};

use fugue::bytes::traits::ByteCast;
use fugue::bytes::Order;

use fugue::bv::BitVec;
use fugue::fp::{self, float_format_from_size, Float, FloatFormat, FloatFormatOps};

use fugue::ir::disassembly::ContextDatabase;
use fugue::ir::il::pcode::Operand;
use fugue::ir::{self, Address, AddressSpace, AddressValue, IntoAddress, Translator};

use fuguex_hooks::concrete::ClonableHookConcrete;
use fuguex_hooks::types::{HookCBranchAction, HookCallAction};

use fuguex_machine::types::Outcome;
use fuguex_machine::StepState;
use fuguex_machine::{Branch, Interpreter};

use fuguex_state::pcode::{self, PCodeState};
use fuguex_state::pcode::{
    MAX_POINTER_SIZE, POINTER_16_SIZE, POINTER_32_SIZE, POINTER_64_SIZE, POINTER_8_SIZE,
};
use fuguex_state::register::ReturnLocation;
use fuguex_state::traits::State;

use rayon::iter::ParallelIterator;
use rayon::slice::ParallelSlice;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("division by zero")]
    DivisionByZero,
    #[error(transparent)]
    Hook(fuguex_hooks::types::Error<pcode::Error>),
    #[error("error lifting instruction at {0}: {1}")]
    Lift(Address, #[source] ir::error::Error),
    #[error(transparent)]
    State(#[from] pcode::Error),
    #[error("incompatible operand sizes of {0} bytes and {1} bytes")]
    IncompatibleOperands(usize, usize),
    #[error("unsupported address size of {} bits", .0 * 8)]
    UnsupportedAddressSize(usize),
    #[error("unsupported branch destination in `{}` space", .0.name())]
    UnsupportedBranchDestination(Arc<AddressSpace>),
    #[error(transparent)]
    UnsupportedFloatFormat(#[from] fp::Error),
    #[error("unsupported operand size of {0} bytes; maximum supported is {1} bytes")]
    UnsupportedOperandSize(usize, usize),
}

pub type ConcreteState<O> = PCodeState<u8, O>;

#[derive(Clone)]
pub struct ConcreteContext<O: Order, R, const OPERAND_SIZE: usize> {
    translator: Arc<Translator>,
    translator_context: ContextDatabase,
    translator_cache: Arc<RwLock<Map<Address, StepState>>>,
    hooks: Vec<
        Box<dyn ClonableHookConcrete<State = ConcreteState<O>, Error = pcode::Error, Outcome = R>>,
    >,
    state: ConcreteState<O>,
    marker: PhantomData<R>,
}

trait ToSignedBytes {
    fn expand_as<O: Order, R, const OPERAND_SIZE: usize>(
        self,
        ctxt: &mut ConcreteContext<O, R, { OPERAND_SIZE }>,
        dest: &Operand,
        signed: bool,
    ) -> Result<(), Error>;
}

impl ToSignedBytes for bool {
    fn expand_as<O: Order, R, const OPERAND_SIZE: usize>(
        self,
        ctxt: &mut ConcreteContext<O, R, { OPERAND_SIZE }>,
        dest: &Operand,
        _signed: bool,
    ) -> Result<(), Error> {
        let mut buf = [0u8; 1];
        self.into_bytes::<O>(&mut buf);

        ctxt.state
            .with_operand_values_mut(dest, |values| values.copy_from_slice(&buf[..]))
            .map_err(Error::State)?;

        for hook in ctxt.hooks.iter_mut() {
            hook.hook_operand_write(&mut ctxt.state, dest, &buf[..])
                .map_err(Error::Hook)?;
        }

        Ok(())
    }
}

impl ToSignedBytes for BitVec {
    fn expand_as<O: Order, R, const OPERAND_SIZE: usize>(
        self,
        ctxt: &mut ConcreteContext<O, R, { OPERAND_SIZE }>,
        dest: &Operand,
        signed: bool,
    ) -> Result<(), Error> {
        let size = dest.size();
        let dbits = size << 3;
        let target = if signed { self.signed() } else { self };
        let target = if target.bits() != dbits {
            target.cast(dbits)
        } else {
            target
        };

        if size > OPERAND_SIZE {
            return Err(Error::UnsupportedOperandSize(size, OPERAND_SIZE));
        }

        let mut buf = [0u8; OPERAND_SIZE];
        target.into_bytes::<O>(&mut buf[..size]);

        ctxt.state
            .with_operand_values_mut(dest, |values| values.copy_from_slice(&buf[..size]))
            .map_err(Error::State)?;

        for hook in ctxt.hooks.iter_mut() {
            hook.hook_operand_write(&mut ctxt.state, dest, &buf[..size])
                .map_err(Error::Hook)?;
        }

        Ok(())
    }
}

impl<O: Order, R: Default, const OPERAND_SIZE: usize> ConcreteContext<O, R, { OPERAND_SIZE }> {
    pub fn new(translator: Translator, state: ConcreteState<O>) -> Self {
        Self {
            translator_context: translator.context_database(),
            translator_cache: Arc::new(RwLock::new(Map::default())),
            translator: Arc::new(translator),
            hooks: Vec::default(),
            state,
            marker: PhantomData,
        }
    }

    pub fn add_hook<H>(&mut self, hook: H)
    where
        H: ClonableHookConcrete<State = ConcreteState<O>, Error = pcode::Error, Outcome = R>
            + 'static,
    {
        self.hooks.push(Box::new(hook));
    }

    pub fn lift_blocks_from(&mut self, database: &Database) -> Result<(), Error> {
        let state = &self.state;
        let trans = self.translator.clone();
        let context = &self.translator_context;
        let cache = self.translator_cache.clone();

        database.functions().par_chunks(256).try_for_each(|fns| {
            let mut translator_context = context.clone();
            let mut current = Map::default();
            for f in fns.iter() {
                // ...with at least one block
                if !f.blocks().is_empty() {
                    // ...where those blocks are not inside an `external` section
                    for b in f.blocks().iter().filter(|b| !b.segment().is_external()) {
                        // ...disassemble the block
                        let mut offset = 0;
                        let address = b.address();

                        let view = state
                            .view_values_from(&address)
                            .map_err(Error::State)?;

                        while offset < b.len() {
                            let address_offset = address + offset as u64;
                            let address_value = trans.address(address_offset.into());

                            let address = Address::from(&address_value);

                            let lifted = trans.lift_pcode(&mut translator_context,
                                            address_value.clone(),
                                            &view[offset..])
                                .map_err(|e| Error::Lift(address, e));

                            if let Err(e) = lifted {
                                if let Ok(dis) = trans.disassemble(&mut translator_context, address_value, &view[offset..]) {
                                        println!("failed on: {} {}", dis.mnemonic(), dis.operands());
                                    }
                                return Err(e)
                            }

                            let lifted = lifted.unwrap();

                            offset += lifted.length();
                            current.insert(address, lifted.into());
                        }
                    }
                }
            }
            cache.write().extend(current.into_iter());
            Ok(())
        })
    }

    pub fn lifted_cache(&self) -> RwLockReadGuard<Map<Address, StepState>> {
        self.translator_cache.read()
    }

    fn lift_int1<CO, COO>(
        &mut self,
        op: CO,
        dest: &Operand,
        rhs: &Operand,
        signed: bool,
    ) -> Result<Outcome<R>, Error>
    where
        CO: FnOnce(BitVec) -> Result<COO, Error>,
        COO: ToSignedBytes,
    {
        let rsize = rhs.size();
        if rsize > OPERAND_SIZE {
            return Err(Error::UnsupportedOperandSize(rsize, OPERAND_SIZE));
        }

        let mut rbuf = [0u8; OPERAND_SIZE];

        self.state
            .with_operand_values(rhs, |values| rbuf[..rsize].copy_from_slice(values))
            .map_err(Error::State)?;

        for hook in self.hooks.iter_mut() {
            hook.hook_operand_read(&mut self.state, rhs, &rbuf[..rsize])
                .map_err(Error::Hook)?;
        }

        op(BitVec::from_bytes::<O>(&rbuf[..rsize], signed))?.expand_as(self, dest, signed)?;

        Ok(Outcome::Branch(Branch::Next))
    }

    fn lift_int2<CO, COO>(
        &mut self,
        op: CO,
        dest: &Operand,
        lhs: &Operand,
        rhs: &Operand,
        signed: bool,
    ) -> Result<Outcome<R>, Error>
    where
        CO: FnOnce(BitVec, BitVec) -> Result<COO, Error>,
        COO: ToSignedBytes,
    {
        let mut lbuf = [0u8; OPERAND_SIZE];
        let mut rbuf = [0u8; OPERAND_SIZE];

        let lsize = lhs.size();
        let rsize = rhs.size();

        if lsize > OPERAND_SIZE {
            return Err(Error::UnsupportedOperandSize(lsize, OPERAND_SIZE));
        }

        if rsize > OPERAND_SIZE {
            return Err(Error::UnsupportedOperandSize(rsize, OPERAND_SIZE));
        }

        self.state
            .with_operand_values(lhs, |values| lbuf[..lsize].copy_from_slice(values))
            .map_err(Error::State)?;

        self.state
            .with_operand_values(rhs, |values| rbuf[..rsize].copy_from_slice(values))
            .map_err(Error::State)?;

        for hook in self.hooks.iter_mut() {
            hook.hook_operand_read(&mut self.state, lhs, &lbuf[..lsize])
                .map_err(Error::Hook)?;
            hook.hook_operand_read(&mut self.state, rhs, &rbuf[..rsize])
                .map_err(Error::Hook)?;
        }

        let lhs_val = BitVec::from_bytes::<O>(&lbuf[..lsize], signed);
        let mut rhs_val = BitVec::from_bytes::<O>(&rbuf[..rsize], signed);

        if lhs.size() != rhs.size() {
            rhs_val = rhs_val.cast(lhs_val.bits());
        }

        op(lhs_val, rhs_val)?.expand_as(self, dest, signed)?;

        Ok(Outcome::Branch(Branch::Next))
    }

    fn lift_bool1<CO>(&mut self, op: CO, dest: &Operand, rhs: &Operand) -> Result<Outcome<R>, Error>
    where
        CO: FnOnce(bool) -> Result<bool, Error>,
    {
        let mut rbuf = [0u8; 1];
        let rsize = rhs.size();

        self.state
            .with_operand_values(rhs, |values| rbuf[..rsize].copy_from_slice(values))
            .map_err(Error::State)?;

        for hook in self.hooks.iter_mut() {
            hook.hook_operand_read(&mut self.state, rhs, &rbuf[..rsize])
                .map_err(Error::Hook)?;
        }

        op(bool::from_bytes::<O>(&rbuf))?.expand_as(self, dest, false)?;

        Ok(Outcome::Branch(Branch::Next))
    }

    fn lift_bool2<CO>(
        &mut self,
        op: CO,
        dest: &Operand,
        lhs: &Operand,
        rhs: &Operand,
    ) -> Result<Outcome<R>, Error>
    where
        CO: FnOnce(bool, bool) -> Result<bool, Error>,
    {
        let mut lbuf = [0u8; 1];
        let mut rbuf = [0u8; 1];

        self.state
            .with_operand_values(lhs, |values| lbuf[..].copy_from_slice(values))
            .map_err(Error::State)?;

        self.state
            .with_operand_values(rhs, |values| rbuf[..].copy_from_slice(values))
            .map_err(Error::State)?;

        for hook in self.hooks.iter_mut() {
            hook.hook_operand_read(&mut self.state, lhs, &lbuf[..])
                .map_err(Error::Hook)?;
            hook.hook_operand_read(&mut self.state, rhs, &rbuf[..])
                .map_err(Error::Hook)?;
        }

        op(
            bool::from_bytes::<O>(&lbuf[..]),
            bool::from_bytes::<O>(&rbuf[..]),
        )?
        .expand_as(self, dest, false)?;

        Ok(Outcome::Branch(Branch::Next))
    }

    fn lift_float1<CO, COO>(
        &mut self,
        op: CO,
        dest: &Operand,
        rhs: &Operand,
    ) -> Result<Outcome<R>, Error>
    where
        CO: FnOnce(Float, &FloatFormat) -> Result<COO, Error>,
        COO: ToSignedBytes,
    {
        let rsize = rhs.size();
        if rsize > OPERAND_SIZE {
            return Err(Error::UnsupportedOperandSize(rsize, OPERAND_SIZE));
        }

        let format = float_format_from_size(rsize)?;
        let mut rbuf = [0u8; OPERAND_SIZE];

        self.state
            .with_operand_values(rhs, |values| rbuf[..rsize].copy_from_slice(values))
            .map_err(Error::State)?;

        for hook in self.hooks.iter_mut() {
            hook.hook_operand_read(&mut self.state, rhs, &rbuf[..rsize])
                .map_err(Error::Hook)?;
        }

        let rhs_val = format.from_bitvec(&BitVec::from_bytes::<O>(&rbuf[..rsize], false));

        op(rhs_val, &format)?.expand_as(self, dest, true)?;

        Ok(Outcome::Branch(Branch::Next))
    }

    fn lift_float2<CO, COO>(
        &mut self,
        op: CO,
        dest: &Operand,
        lhs: &Operand,
        rhs: &Operand,
    ) -> Result<Outcome<R>, Error>
    where
        CO: FnOnce(Float, Float, &FloatFormat) -> Result<COO, Error>,
        COO: ToSignedBytes,
    {
        let mut lbuf = [0u8; OPERAND_SIZE];
        let mut rbuf = [0u8; OPERAND_SIZE];

        let lsize = lhs.size();
        let rsize = rhs.size();

        if lsize > OPERAND_SIZE {
            return Err(Error::UnsupportedOperandSize(lsize, OPERAND_SIZE));
        }

        if rsize > OPERAND_SIZE {
            return Err(Error::UnsupportedOperandSize(rsize, OPERAND_SIZE));
        }

        if lsize != rsize {
            return Err(Error::IncompatibleOperands(lsize, rsize));
        }

        let format = float_format_from_size(rsize)?;

        self.state
            .with_operand_values(lhs, |values| lbuf[..lsize].copy_from_slice(values))
            .map_err(Error::State)?;

        self.state
            .with_operand_values(rhs, |values| rbuf[..rsize].copy_from_slice(values))
            .map_err(Error::State)?;

        for hook in self.hooks.iter_mut() {
            hook.hook_operand_read(&mut self.state, lhs, &lbuf[..lsize])
                .map_err(Error::Hook)?;
            hook.hook_operand_read(&mut self.state, rhs, &rbuf[..rsize])
                .map_err(Error::Hook)?;
        }

        let lhs_val = format.from_bitvec(&BitVec::from_bytes::<O>(&lbuf[..lsize], false));
        let rhs_val = format.from_bitvec(&BitVec::from_bytes::<O>(&rbuf[..rsize], false));

        op(lhs_val, rhs_val, &format)?.expand_as(self, dest, true)?;

        Ok(Outcome::Branch(Branch::Next))
    }

    fn with_return_location<U, F>(&self, f: F) -> Result<U, Error>
    where
        F: FnOnce(&Operand) -> Result<U, Error>,
    {
        match self.state.registers().return_location() {
            ReturnLocation::Register(ref operand) => f(&operand.clone()),
            ReturnLocation::Relative(ref operand, offset) => {
                let address = self.state.get_address(operand).map_err(Error::State)?;
                let operand = Operand::Address {
                    value: AddressValue::new(
                        self.state.memory_space(),
                        u64::from(address + *offset),
                    ),
                    size: self.state.memory_space().address_size(),
                };
                f(&operand)
            }
        }
    }

    fn skip_return(&mut self) -> Result<AddressValue, Error> {
        // NOTE: for x86, etc. we need to clean-up the stack
        // arguments; currently, this is the responsibility of
        // hooks that issue a `HookCallAction::Skip`.
        let address = self.with_return_location(|operand| {
            self.state.get_address(operand).map_err(Error::State)
        })?;

        // Next we pop the return address (if needed)
        let extra_pop = self.state.convention().default_prototype().extra_pop();

        if extra_pop > 0 {
            let stack_pointer = self.state.registers().stack_pointer().clone();
            let address = self
                .state
                .get_address(&stack_pointer)
                .map_err(Error::State)?;

            self.state
                .set_address(&stack_pointer, address + extra_pop)
                .map_err(Error::State)?;
        }

        Ok(AddressValue::new(
            self.state.memory_space(),
            u64::from(address),
        ))
    }

    #[inline]
    fn get_address_value(&mut self, pointer: &Operand) -> Result<u64, Error> {
        let mut buf = [0u8; MAX_POINTER_SIZE];
        let psize = pointer.size();

        let address = if psize == POINTER_64_SIZE {
            self.state
                .with_operand_values(pointer, |values| {
                    buf[..psize].copy_from_slice(values);
                    u64::from_bytes::<O>(values)
                })
                .map_err(Error::State)?
        } else if psize == POINTER_32_SIZE {
            self.state
                .with_operand_values(pointer, |values| {
                    buf[..psize].copy_from_slice(values);
                    u32::from_bytes::<O>(values) as u64
                })
                .map_err(Error::State)?
        } else if psize == POINTER_16_SIZE {
            self.state
                .with_operand_values(pointer, |values| {
                    buf[..psize].copy_from_slice(values);
                    u16::from_bytes::<O>(values) as u64
                })
                .map_err(Error::State)?
        } else if psize == POINTER_8_SIZE {
            self.state
                .with_operand_values(pointer, |values| {
                    buf[..psize].copy_from_slice(values);
                    u8::from_bytes::<O>(values) as u64
                })
                .map_err(Error::State)?
        } else {
            return Err(Error::UnsupportedAddressSize(pointer.size()));
        };

        for hook in self.hooks.iter_mut() {
            hook.hook_operand_read(&mut self.state, pointer, &buf[..psize])
                .map_err(Error::Hook)?;
        }

        Ok(address)
    }

    #[inline]
    fn copy_operand(&mut self, source: &Operand, destination: &Operand) -> Result<(), Error> {
        let size = source.size();
        if size > OPERAND_SIZE {
            return Err(Error::UnsupportedOperandSize(size, OPERAND_SIZE));
        }

        let mut buf = [0u8; OPERAND_SIZE];
        self.state
            .with_operand_values(source, |values| buf[..size].copy_from_slice(values))
            .map_err(Error::State)?;

        for hook in self.hooks.iter_mut() {
            hook.hook_operand_read(&mut self.state, source, &buf[..size])
                .map_err(Error::Hook)?;
        }

        self.state
            .with_operand_values_mut(destination, |values| values.copy_from_slice(&buf[..size]))
            .map_err(Error::State)?;

        for hook in self.hooks.iter_mut() {
            hook.hook_operand_write(&mut self.state, destination, &buf[..size])
                .map_err(Error::Hook)?;
        }

        Ok(())
    }
}

impl<O: Order, R: Default, const OPERAND_SIZE: usize> Interpreter
    for ConcreteContext<O, R, { OPERAND_SIZE }>
{
    type State = PCodeState<u8, O>;
    type Error = Error;
    type Outcome = R;

    fn fork(&self) -> Self {
        Self {
            translator: self.translator.clone(),
            translator_context: self.translator.context_database(),
            translator_cache: self.translator_cache.clone(),
            hooks: self.hooks.clone(),
            state: self.state.fork(),
            marker: self.marker,
        }
    }

    fn restore(&mut self, other: &Self) {
        self.hooks = other.hooks.clone();
        self.state.restore(&other.state);
    }

    fn copy(&mut self, source: &Operand, destination: &Operand) -> Result<Outcome<R>, Error> {
        self.copy_operand(source, destination)?;
        Ok(Outcome::Branch(Branch::Next))
    }

    fn load(
        &mut self,
        source: &Operand,
        destination: &Operand,
        space: Arc<AddressSpace>,
    ) -> Result<Outcome<R>, Error> {
        let space_size = space.address_size();
        let space_word_size = space.word_size() as u64;

        assert_eq!(space_size, source.size());

        let offset = self.get_address_value(source)?;
        let addr_val = offset.wrapping_mul(space_word_size)
            & 1u64
                .checked_shl(space_size.checked_shl(3).unwrap_or(0) as u32)
                .unwrap_or(0)
                .wrapping_sub(1);

        let address = Operand::Address {
            value: AddressValue::new(space, addr_val),
            size: destination.size(),
        };

        self.copy_operand(&address, destination)?;

        Ok(Outcome::Branch(Branch::Next))
    }

    fn store(
        &mut self,
        source: &Operand,
        destination: &Operand,
        space: Arc<AddressSpace>,
    ) -> Result<Outcome<R>, Error> {
        // Same semantics as copy and load, just with different address spaces
        let space_size = space.address_size();
        let space_word_size = space.word_size() as u64;

        assert_eq!(space_size, destination.size());

        let offset = self.get_address_value(destination)?;

        // NOTE:
        // It is possible for the addressable unit of an address space to be
        // bigger than a single byte. If the wordsize attribute of the space
        // given by the ID is bigger than one, the offset into the space
        // obtained from input1 must be multiplied by this value in order to
        // obtain the correct byte offset into the space.

        let addr_val = offset.wrapping_mul(space_word_size)
            & 1u64
                .checked_shl(space_size.checked_shl(3).unwrap_or(0) as u32)
                .unwrap_or(0)
                .wrapping_sub(1);

        let address = Operand::Address {
            value: AddressValue::new(space, addr_val),
            size: source.size(),
        };

        self.copy_operand(source, &address)?;

        Ok(Outcome::Branch(Branch::Next))
    }

    fn branch(&mut self, destination: &Operand) -> Result<Outcome<R>, Error> {
        // destination operand does not store branch target, it is branch target
        match destination {
            Operand::Constant { value, .. } => {
                let action = Branch::Local(*value as isize);
                Ok(Outcome::Branch(action))
            }
            Operand::Address { value, .. } => {
                let action = Branch::Global(value.clone());
                Ok(Outcome::Branch(action))
            }
            Operand::Register { space, .. } | Operand::Variable { space, .. } => {
                return Err(Error::UnsupportedBranchDestination(space.clone()))
            }
        }
    }

    fn cbranch(&mut self, destination: &Operand, condition: &Operand) -> Result<Outcome<R>, Error> {
        assert!(condition.size() == 1);

        let mut flip = false;
        for hook in self.hooks.iter_mut() {
            match hook
                .hook_cbranch(&mut self.state, destination, condition)
                .map_err(Error::Hook)?
                .action
            {
                HookCBranchAction::Pass => (),
                HookCBranchAction::Flip => {
                    flip = true;
                }
                HookCBranchAction::Halt(r) => return Ok(Outcome::Halt(r)),
            }
        }

        let mut buf = [0u8; 1];
        self.state
            .with_operand_values(condition, |values| buf.copy_from_slice(values))
            .map_err(Error::State)?;

        for hook in self.hooks.iter_mut() {
            hook.hook_operand_read(&mut self.state, condition, &buf)
                .map_err(Error::Hook)?;
        }

        if flip {
            if bool::from_bytes::<O>(&buf) {
                self.state.set_operand(condition, false)
            } else {
                self.state.set_operand(condition, true)
            }?;
        }

        self.branch(destination)
    }

    fn ibranch(&mut self, destination: &Operand) -> Result<Outcome<R>, Error> {
        if destination == self.state.registers().program_counter() {
            return self.icall(destination);
        }

        let address = AddressValue::new(
            self.state.memory_space(),
            self.get_address_value(destination)?,
        );
        Ok(Outcome::Branch(Branch::Global(address)))
    }

    fn call(&mut self, destination: &Operand) -> Result<Outcome<R>, Error> {
        match destination {
            Operand::Address { value, .. } => {
                let mut skip = false;
                let address = value.into();
                for hook in self.hooks.iter_mut() {
                    match hook
                        .hook_call(&mut self.state, &address)
                        .map_err(Error::Hook)?
                        .action
                    {
                        HookCallAction::Pass => (),
                        HookCallAction::Skip => {
                            skip = true;
                        }
                        HookCallAction::Halt(r) => return Ok(Outcome::Halt(r)),
                    }
                }

                if skip {
                    Ok(Outcome::Branch(Branch::Global(self.skip_return()?)))
                } else {
                    Ok(Outcome::Branch(Branch::Global(value.clone())))
                }
            }
            Operand::Constant { space, .. }
            | Operand::Register { space, .. }
            | Operand::Variable { space, .. } => {
                Err(Error::UnsupportedBranchDestination(space.clone()))
            }
        }
    }

    fn icall(&mut self, destination: &Operand) -> Result<Outcome<R>, Error> {
        let address_value = AddressValue::new(
            self.state.memory_space(),
            self.get_address_value(destination)?,
        );
        let address = Address::from(&address_value);

        let mut skip = false;
        for hook in self.hooks.iter_mut() {
            match hook
                .hook_call(&mut self.state, &address)
                .map_err(Error::Hook)?
                .action
            {
                HookCallAction::Pass => (),
                HookCallAction::Skip => {
                    skip = true;
                }
                HookCallAction::Halt(r) => return Ok(Outcome::Halt(r)),
            }
        }

        if skip {
            Ok(Outcome::Branch(Branch::Global(self.skip_return()?)))
        } else {
            Ok(Outcome::Branch(Branch::Global(address_value)))
        }
    }

    fn return_(&mut self, destination: &Operand) -> Result<Outcome<R>, Error> {
        self.ibranch(destination)
    }

    fn int_eq(
        &mut self,
        destination: &Operand,
        operand1: &Operand,
        operand2: &Operand,
    ) -> Result<Outcome<R>, Error> {
        self.lift_int2(|u, v| Ok(u == v), destination, operand1, operand2, false)
    }

    fn int_not_eq(
        &mut self,
        destination: &Operand,
        operand1: &Operand,
        operand2: &Operand,
    ) -> Result<Outcome<R>, Self::Error> {
        self.lift_int2(|u, v| Ok(u != v), destination, operand1, operand2, false)
    }

    fn int_less(
        &mut self,
        destination: &Operand,
        operand1: &Operand,
        operand2: &Operand,
    ) -> Result<Outcome<R>, Self::Error> {
        self.lift_int2(|u, v| Ok(u < v), destination, operand1, operand2, false)
    }

    fn int_less_eq(
        &mut self,
        destination: &Operand,
        operand1: &Operand,
        operand2: &Operand,
    ) -> Result<Outcome<R>, Self::Error> {
        self.lift_int2(|u, v| Ok(u <= v), destination, operand1, operand2, false)
    }

    fn int_sless(
        &mut self,
        destination: &Operand,
        operand1: &Operand,
        operand2: &Operand,
    ) -> Result<Outcome<R>, Self::Error> {
        self.lift_int2(|u, v| Ok(u < v), destination, operand1, operand2, true)
    }

    fn int_sless_eq(
        &mut self,
        destination: &Operand,
        operand1: &Operand,
        operand2: &Operand,
    ) -> Result<Outcome<R>, Self::Error> {
        self.lift_int2(|u, v| Ok(u <= v), destination, operand1, operand2, true)
    }

    fn int_zext(
        &mut self,
        destination: &Operand,
        operand: &Operand,
    ) -> Result<Outcome<R>, Self::Error> {
        self.lift_int1(|u| Ok(u), destination, operand, false)
    }

    fn int_sext(
        &mut self,
        destination: &Operand,
        operand: &Operand,
    ) -> Result<Outcome<R>, Self::Error> {
        self.lift_int1(|u| Ok(u), destination, operand, true)
    }

    fn int_add(
        &mut self,
        destination: &Operand,
        operand1: &Operand,
        operand2: &Operand,
    ) -> Result<Outcome<R>, Self::Error> {
        self.lift_int2(|u, v| Ok(u + v), destination, operand1, operand2, false)
    }

    fn int_sub(
        &mut self,
        destination: &Operand,
        operand1: &Operand,
        operand2: &Operand,
    ) -> Result<Outcome<R>, Self::Error> {
        self.lift_int2(|u, v| Ok(u - v), destination, operand1, operand2, false)
    }

    fn int_carry(
        &mut self,
        destination: &Operand,
        operand1: &Operand,
        operand2: &Operand,
    ) -> Result<Outcome<R>, Self::Error> {
        self.lift_int2(
            |u, v| Ok(u.carry(&v)),
            destination,
            operand1,
            operand2,
            false,
        )
    }

    fn int_scarry(
        &mut self,
        destination: &Operand,
        operand1: &Operand,
        operand2: &Operand,
    ) -> Result<Outcome<R>, Self::Error> {
        self.lift_int2(
            |u, v| Ok(u.carry(&v)),
            destination,
            operand1,
            operand2,
            true,
        )
    }

    fn int_sborrow(
        &mut self,
        destination: &Operand,
        operand1: &Operand,
        operand2: &Operand,
    ) -> Result<Outcome<R>, Self::Error> {
        self.lift_int2(
            |u, v| Ok(u.borrow(&v)),
            destination,
            operand1,
            operand2,
            true,
        )
    }

    fn int_neg(
        &mut self,
        destination: &Operand,
        operand: &Operand,
    ) -> Result<Outcome<R>, Self::Error> {
        self.lift_int1(|u| Ok(-u), destination, operand, true)
    }

    fn int_not(
        &mut self,
        destination: &Operand,
        operand: &Operand,
    ) -> Result<Outcome<R>, Self::Error> {
        self.lift_int1(|u| Ok(!u), destination, operand, false)
    }

    fn int_xor(
        &mut self,
        destination: &Operand,
        operand1: &Operand,
        operand2: &Operand,
    ) -> Result<Outcome<R>, Self::Error> {
        self.lift_int2(|u, v| Ok(u ^ v), destination, operand1, operand2, false)
    }

    fn int_and(
        &mut self,
        destination: &Operand,
        operand1: &Operand,
        operand2: &Operand,
    ) -> Result<Outcome<R>, Self::Error> {
        self.lift_int2(|u, v| Ok(u & v), destination, operand1, operand2, false)
    }

    fn int_or(
        &mut self,
        destination: &Operand,
        operand1: &Operand,
        operand2: &Operand,
    ) -> Result<Outcome<R>, Self::Error> {
        self.lift_int2(|u, v| Ok(u | v), destination, operand1, operand2, false)
    }

    fn int_left_shift(
        &mut self,
        destination: &Operand,
        operand1: &Operand,
        operand2: &Operand,
    ) -> Result<Outcome<R>, Self::Error> {
        self.lift_int2(|u, v| Ok(u << v), destination, operand1, operand2, false)
    }

    fn int_right_shift(
        &mut self,
        destination: &Operand,
        operand1: &Operand,
        operand2: &Operand,
    ) -> Result<Outcome<R>, Self::Error> {
        self.lift_int2(|u, v| Ok(u >> v), destination, operand1, operand2, false)
    }

    fn int_sright_shift(
        &mut self,
        destination: &Operand,
        operand1: &Operand,
        operand2: &Operand,
    ) -> Result<Outcome<R>, Self::Error> {
        self.lift_int2(|u, v| Ok(u >> v), destination, operand1, operand2, true)
    }

    fn int_mul(
        &mut self,
        destination: &Operand,
        operand1: &Operand,
        operand2: &Operand,
    ) -> Result<Outcome<R>, Self::Error> {
        self.lift_int2(|u, v| Ok(u * v), destination, operand1, operand2, false)
    }

    fn int_div(
        &mut self,
        destination: &Operand,
        operand1: &Operand,
        operand2: &Operand,
    ) -> Result<Outcome<R>, Self::Error> {
        self.lift_int2(
            |u, v| {
                if v.is_zero() {
                    Err(Error::DivisionByZero)
                } else {
                    Ok(u / v)
                }
            },
            destination,
            operand1,
            operand2,
            false,
        )
    }

    fn int_sdiv(
        &mut self,
        destination: &Operand,
        operand1: &Operand,
        operand2: &Operand,
    ) -> Result<Outcome<R>, Self::Error> {
        self.lift_int2(
            |u, v| {
                if v.is_zero() {
                    Err(Error::DivisionByZero)
                } else {
                    Ok(u / v)
                }
            },
            destination,
            operand1,
            operand2,
            true,
        )
    }

    fn int_rem(
        &mut self,
        destination: &Operand,
        operand1: &Operand,
        operand2: &Operand,
    ) -> Result<Outcome<R>, Self::Error> {
        self.lift_int2(
            |u, v| {
                if v.is_zero() {
                    Err(Error::DivisionByZero)
                } else {
                    Ok(u % v)
                }
            },
            destination,
            operand1,
            operand2,
            false,
        )
    }

    fn int_srem(
        &mut self,
        destination: &Operand,
        operand1: &Operand,
        operand2: &Operand,
    ) -> Result<Outcome<R>, Self::Error> {
        self.lift_int2(
            |u, v| {
                if v.is_zero() {
                    Err(Error::DivisionByZero)
                } else {
                    Ok(u % v)
                }
            },
            destination,
            operand1,
            operand2,
            true,
        )
    }

    fn bool_not(
        &mut self,
        destination: &Operand,
        operand: &Operand,
    ) -> Result<Outcome<R>, Self::Error> {
        self.lift_bool1(|u| Ok(!u), destination, operand)
    }

    fn bool_xor(
        &mut self,
        destination: &Operand,
        operand1: &Operand,
        operand2: &Operand,
    ) -> Result<Outcome<R>, Self::Error> {
        self.lift_bool2(|u, v| Ok(u ^ v), destination, operand1, operand2)
    }

    fn bool_and(
        &mut self,
        destination: &Operand,
        operand1: &Operand,
        operand2: &Operand,
    ) -> Result<Outcome<R>, Self::Error> {
        self.lift_bool2(|u, v| Ok(u & v), destination, operand1, operand2)
    }

    fn bool_or(
        &mut self,
        destination: &Operand,
        operand1: &Operand,
        operand2: &Operand,
    ) -> Result<Outcome<R>, Self::Error> {
        self.lift_bool2(|u, v| Ok(u | v), destination, operand1, operand2)
    }

    fn float_eq(
        &mut self,
        destination: &Operand,
        operand1: &Operand,
        operand2: &Operand,
    ) -> Result<Outcome<R>, Self::Error> {
        self.lift_float2(|u, v, _fmt| Ok(u == v), destination, operand1, operand2)
    }

    fn float_not_eq(
        &mut self,
        destination: &Operand,
        operand1: &Operand,
        operand2: &Operand,
    ) -> Result<Outcome<R>, Self::Error> {
        self.lift_float2(|u, v, _fmt| Ok(u != v), destination, operand1, operand2)
    }

    fn float_less(
        &mut self,
        destination: &Operand,
        operand1: &Operand,
        operand2: &Operand,
    ) -> Result<Outcome<R>, Self::Error> {
        self.lift_float2(|u, v, _fmt| Ok(u < v), destination, operand1, operand2)
    }

    fn float_less_eq(
        &mut self,
        destination: &Operand,
        operand1: &Operand,
        operand2: &Operand,
    ) -> Result<Outcome<R>, Self::Error> {
        self.lift_float2(|u, v, _fmt| Ok(u <= v), destination, operand1, operand2)
    }

    fn float_is_nan(
        &mut self,
        destination: &Operand,
        operand: &Operand,
    ) -> Result<Outcome<R>, Self::Error> {
        self.lift_float1(|u, _fmt| Ok(u.is_nan()), destination, operand)
    }

    fn float_add(
        &mut self,
        destination: &Operand,
        operand1: &Operand,
        operand2: &Operand,
    ) -> Result<Outcome<R>, Self::Error> {
        self.lift_float2(
            |u, v, fmt| Ok(fmt.into_bitvec(u + v, fmt.bits())),
            destination,
            operand1,
            operand2,
        )
    }

    fn float_div(
        &mut self,
        destination: &Operand,
        operand1: &Operand,
        operand2: &Operand,
    ) -> Result<Outcome<R>, Self::Error> {
        self.lift_float2(
            |u, v, fmt| Ok(fmt.into_bitvec(u / v, fmt.bits())),
            destination,
            operand1,
            operand2,
        )
    }

    fn float_mul(
        &mut self,
        destination: &Operand,
        operand1: &Operand,
        operand2: &Operand,
    ) -> Result<Outcome<R>, Self::Error> {
        self.lift_float2(
            |u, v, fmt| Ok(fmt.into_bitvec(u * v, fmt.bits())),
            destination,
            operand1,
            operand2,
        )
    }

    fn float_sub(
        &mut self,
        destination: &Operand,
        operand1: &Operand,
        operand2: &Operand,
    ) -> Result<Outcome<R>, Self::Error> {
        self.lift_float2(
            |u, v, fmt| Ok(fmt.into_bitvec(u - v, fmt.bits())),
            destination,
            operand1,
            operand2,
        )
    }

    fn float_neg(
        &mut self,
        destination: &Operand,
        operand: &Operand,
    ) -> Result<Outcome<R>, Self::Error> {
        self.lift_float1(
            |u, fmt| Ok(fmt.into_bitvec(-u, fmt.bits())),
            destination,
            operand,
        )
    }

    fn float_abs(
        &mut self,
        destination: &Operand,
        operand: &Operand,
    ) -> Result<Outcome<R>, Self::Error> {
        self.lift_float1(
            |u, fmt| Ok(fmt.into_bitvec(u.abs(), fmt.bits())),
            destination,
            operand,
        )
    }

    fn float_sqrt(
        &mut self,
        destination: &Operand,
        operand: &Operand,
    ) -> Result<Outcome<R>, Self::Error> {
        self.lift_float1(
            |u, fmt| Ok(fmt.into_bitvec(u.sqrt(), fmt.bits())),
            destination,
            operand,
        )
    }

    fn float_of_int(
        &mut self,
        destination: &Operand,
        operand: &Operand,
    ) -> Result<Outcome<R>, Self::Error> {
        let fmt = float_format_from_size(destination.size())?;
        self.lift_int1(
            |u| {
                let ival = u.into_bigint();
                let fval = Float::from_bigint(fmt.frac_size, fmt.exp_size, ival);
                Ok(fmt.into_bitvec(fval, fmt.bits()))
            },
            destination,
            operand,
            true,
        )
    }

    fn float_of_float(
        &mut self,
        destination: &Operand,
        operand: &Operand,
    ) -> Result<Outcome<R>, Self::Error> {
        let fmt = float_format_from_size(destination.size())?;
        self.lift_float1(
            |rhs, _fmt| Ok(fmt.into_bitvec(rhs, fmt.bits())),
            destination,
            operand,
        )
    }

    fn float_truncate(
        &mut self,
        destination: &Operand,
        operand: &Operand,
    ) -> Result<Outcome<R>, Self::Error> {
        self.lift_float1(
            |u, _fmt| Ok(u.trunc_into_bitvec(destination.size() * 8)),
            destination,
            operand,
        )
    }

    fn float_ceiling(
        &mut self,
        destination: &Operand,
        operand: &Operand,
    ) -> Result<Outcome<R>, Self::Error> {
        self.lift_float1(
            |u, fmt| Ok(fmt.into_bitvec(u.ceil(), fmt.bits())),
            destination,
            operand,
        )
    }

    fn float_floor(
        &mut self,
        destination: &Operand,
        operand: &Operand,
    ) -> Result<Outcome<R>, Self::Error> {
        self.lift_float1(
            |u, fmt| Ok(fmt.into_bitvec(u.floor(), fmt.bits())),
            destination,
            operand,
        )
    }

    fn float_round(
        &mut self,
        destination: &Operand,
        operand: &Operand,
    ) -> Result<Outcome<R>, Self::Error> {
        self.lift_float1(
            |u, fmt| Ok(fmt.into_bitvec(u.round(), fmt.bits())),
            destination,
            operand,
        )
    }

    fn subpiece(
        &mut self,
        destination: &Operand,
        operand: &Operand,
        amount: &Operand,
    ) -> Result<Outcome<R>, Error> {
        let amount_size = amount.size();
        if amount_size > OPERAND_SIZE {
            return Err(Error::UnsupportedOperandSize(amount_size, OPERAND_SIZE));
        }

        let input_size = operand.size();
        if input_size > OPERAND_SIZE {
            return Err(Error::UnsupportedOperandSize(input_size, OPERAND_SIZE));
        }

        let destination_size = destination.size();
        if destination_size > OPERAND_SIZE {
            return Err(Error::UnsupportedOperandSize(
                destination_size,
                OPERAND_SIZE,
            ));
        }

        let mut buf = [0u8; OPERAND_SIZE];
        self.state
            .with_operand_values(amount, |values| buf[..amount_size].copy_from_slice(values))
            .map_err(Error::State)?;

        let amount = BitVec::from_bytes::<O>(&buf[..amount_size], false)
            .to_usize()
            .expect("subpiece `amount` can be stored within usize");

        let mut input_buf = [0u8; OPERAND_SIZE];
        let input_view = &mut input_buf[..input_size];

        self.state
            .with_operand_values(operand, |values| input_view.copy_from_slice(values))
            .map_err(Error::State)?;

        let mut output_buf = [0u8; OPERAND_SIZE];
        let output_view = &mut output_buf[..destination_size];

        O::subpiece(output_view, input_view, amount);

        self.state
            .with_operand_values_mut(destination, |values| values.copy_from_slice(output_view))
            .map_err(Error::State)?;

        Ok(Outcome::Branch(Branch::Next))
    }

    fn pop_count(
        &mut self,
        destination: &Operand,
        operand: &Operand,
    ) -> Result<Outcome<R>, Self::Error> {
        self.lift_int1(
            |u| Ok(BitVec::from(u.count_ones())),
            destination,
            operand,
            false,
        )
    }

    fn intrinsic(
        &mut self,
        _name: &str,
        _operands: &[Operand],
        _result: Option<&Operand>,
    ) -> Result<Outcome<R>, Error> {
        Ok(Outcome::Halt(Default::default()))
    }

    fn lift<A>(&mut self, address: A) -> Result<StepState, Error>
    where
        A: IntoAddress,
    {
        let address_value = address.into_address_value(self.state.memory_space());
        let address = Address::from(&address_value);

        log::debug!("lifting at {}", address);

        // begin read lock region
        let rlock = self.translator_cache.read();

        let cached = rlock.get(&address)
            .map(|step_state| step_state.clone());

        drop(rlock);
        // end read lock region

        let step_state = if let Some(step_state) = cached {
            step_state.clone()
        } else {
            // NOTE: possible race here, if another thread populates
            // the same address. We don't really care, I suppose.

            let view = self
                .state
                .view_values_from(&address)
                .map_err(Error::State)?;
            let step_state = StepState::from(
                self.translator
                    .lift_pcode(&mut self.translator_context, address_value, view)
                    .map_err(|e| Error::Lift(address, e))?
            );

            self.translator_cache
                .write()
                .insert(address, step_state.clone());

            step_state
        };

        let program_counter = self.state.registers().program_counter().clone();
        self.state
            .set_address(&program_counter, address)
            .map_err(Error::State)?;

        Ok(step_state)
    }

    fn interpreter_space(&self) -> Arc<AddressSpace> {
        self.state.memory_space()
    }
}
