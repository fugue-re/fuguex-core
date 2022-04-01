use std::marker::PhantomData;
use std::sync::Arc;

use fnv::FnvHashMap as Map;
use parking_lot::{RwLock, RwLockReadGuard};

use fugue::bytes::traits::ByteCast;
use fugue::bytes::Order;

use fugue::bv::BitVec;
use fugue::db::Database;
use fugue::fp::{self, float_format_from_size, Float, FloatFormat, FloatFormatOps};

use fugue::ir::disassembly::ContextDatabase;
use fugue::ir::il::pcode::{Operand, PCodeOp};
use fugue::ir::il::Location;
use fugue::ir::{
    self, Address, AddressSpace, AddressSpaceId, AddressValue, IntoAddress, Translator,
};

use crate::hooks::ClonableHookConcrete;
use fuguex_hooks::types::{HookCBranchAction, HookCallAction};

use fuguex_intrinsics::{IntrinsicAction, IntrinsicHandler};

use fuguex_loader::LoaderMapping;

use fuguex_machine::types::{Branch, OrOutcome, Outcome, StepState};
use fuguex_machine::Interpreter;

use fuguex_microx::ViolationSource;

use fuguex_state::pcode::{self, PCodeState};
use fuguex_state::pcode::{
    MAX_POINTER_SIZE, POINTER_16_SIZE, POINTER_32_SIZE, POINTER_64_SIZE, POINTER_8_SIZE,
};
use fuguex_state::register::ReturnLocation;
use fuguex_state::traits::State;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("division by zero")]
    DivisionByZero,
    #[error(transparent)]
    Hook(fuguex_hooks::types::Error<pcode::Error>),
    #[error(transparent)]
    Intrinsic(fuguex_intrinsics::Error<pcode::Error>),
    #[error("error lifting instruction at {0}: {1}")]
    Lift(Address, #[source] ir::error::Error),
    #[error(transparent)]
    State(#[from] pcode::Error),
    #[error("incompatible operand sizes of {0} bytes and {1} bytes")]
    IncompatibleOperands(usize, usize),
    #[error("unsupported address size of {} bits", .0 * 8)]
    UnsupportedAddressSize(usize),
    #[error("unsupported branch destination in space `{}`", .0.index())]
    UnsupportedBranchDestination(AddressSpaceId),
    #[error(transparent)]
    UnsupportedFloatFormat(#[from] fp::Error),
    #[error("unsupported operand size of {0} bytes; maximum supported is {1} bytes")]
    UnsupportedOperandSize(usize, usize),
}

pub type ConcreteState<O> = PCodeState<u8, O>;

#[derive(Clone)]
pub struct ConcreteContext<O: Order, R, const OPERAND_SIZE: usize> {
    database: Option<Arc<Database>>,
    translator: Arc<Translator>,
    translator_context: ContextDatabase,
    translator_cache: Arc<RwLock<Map<Address, StepState>>>,
    hook_names: Map<String, usize>,
    hooks: Vec<
        Box<dyn ClonableHookConcrete<State = ConcreteState<O>, Error = pcode::Error, Outcome = R>>,
    >,
    intrinsics: IntrinsicHandler<R, ConcreteState<O>>,
    state: ConcreteState<O>,
    marker: PhantomData<R>,
}

trait ToSignedBytes {
    fn expand_as<O: Order, R: Clone + Default + 'static, const OPERAND_SIZE: usize>(
        self,
        ctxt: &mut ConcreteContext<O, R, { OPERAND_SIZE }>,
        dest: &Operand,
        signed: bool,
    ) -> Result<(), Error>;
}

impl ToSignedBytes for bool {
    fn expand_as<O: Order, R: Clone + Default + 'static, const OPERAND_SIZE: usize>(
        self,
        ctxt: &mut ConcreteContext<O, R, { OPERAND_SIZE }>,
        dest: &Operand,
        _signed: bool,
    ) -> Result<(), Error> {
        let mut buf = [0u8; 1];
        self.into_bytes::<O>(&mut buf);

        ctxt.write_operand(dest, &buf[..])?;

        Ok(())
    }
}

impl ToSignedBytes for BitVec {
    fn expand_as<O: Order, R: Clone + Default + 'static, const OPERAND_SIZE: usize>(
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

        ctxt.write_operand(dest, &buf[..size])?;

        Ok(())
    }
}

impl<O: Order, R: Clone + Default + 'static, const OPERAND_SIZE: usize>
    ConcreteContext<O, R, { OPERAND_SIZE }>
{
    pub fn new(translator: Translator, state: ConcreteState<O>) -> Self {
        Self {
            database: None,
            translator_context: translator.context_database(),
            translator_cache: Arc::new(RwLock::new(Map::default())),
            translator: Arc::new(translator),
            hook_names: Map::default(),
            hooks: Vec::default(),
            intrinsics: IntrinsicHandler::default(),
            state,
            marker: PhantomData,
        }
    }

    pub fn from_loader(loader: impl LoaderMapping<ConcreteState<O>>) -> Self {
        let database = loader.database();
        let translator = loader.translator();
        let state = loader.into_state();

        Self {
            database,
            translator_context: translator.context_database(),
            translator_cache: Arc::new(RwLock::new(Map::default())),
            translator,
            hook_names: Map::default(),
            hooks: Vec::default(),
            intrinsics: IntrinsicHandler::default(),
            state,
            marker: PhantomData,
        }
    }

    pub fn add_hook<S, H>(&mut self, name: S, hook: H)
    where
        S: AsRef<str>,
        H: ClonableHookConcrete<State = ConcreteState<O>, Error = pcode::Error, Outcome = R>
            + 'static,
    {
        let index = self.hooks.len();
        self.hook_names.insert(name.as_ref().into(), index);
        self.hooks.push(Box::new(hook));
    }

    pub fn find_hook<S, H>(&self, name: S) -> Option<&H>
    where
        S: AsRef<str>,
        H: ClonableHookConcrete<State = ConcreteState<O>, Error = pcode::Error, Outcome = R>
            + 'static,
    {
        self.hook_names
            .get(name.as_ref())
            .copied()
            .and_then(|i| self.hooks[i].downcast_ref::<H>())
    }

    pub fn find_hook_mut<S, H>(&mut self, name: S) -> Option<&mut H>
    where
        S: AsRef<str>,
        H: ClonableHookConcrete<State = ConcreteState<O>, Error = pcode::Error, Outcome = R>
            + 'static,
    {
        self.hook_names
            .get(name.as_ref())
            .copied()
            .and_then(move |i| self.hooks[i].downcast_mut::<H>())
    }

    pub fn database(&self) -> Option<&Database> {
        self.database.as_deref()
    }

    pub fn state(&self) -> &ConcreteState<O> {
        &self.state
    }

    pub fn state_mut(&mut self) -> &mut ConcreteState<O> {
        &mut self.state
    }

    pub fn lifted_cache(&self) -> RwLockReadGuard<Map<Address, StepState>> {
        self.translator_cache.read()
    }

    fn read_operand_with<U, F>(
        &mut self,
        operand: &Operand,
        buf: &mut [u8],
        kind: ViolationSource,
        f: F,
    ) -> Result<U, Error>
    where
        F: Fn(&mut [u8]) -> U,
    {
        for hook in self.hooks.iter_mut() {
            hook.hook_operand_read(&mut self.state, operand)
                .map_err(Error::Hook)?;
        }

        let res = self.state.with_operand_values(operand, |values| {
            buf.copy_from_slice(values);
            f(buf)
        });

        if let Err(pcode::Error::Memory(ref e)) = res {
            let mut state_change = None;
            let (address, size) = e.access();

            debug_assert_eq!(size, buf.len());

            for hook in self.hooks.iter_mut() {
                let result = hook
                    .hook_invalid_memory_access(&mut self.state, &address, size, kind)
                    .map_err(Error::Hook)?;
                if result.state_changed || result.action.is_value() {
                    state_change = Some(result);
                }
            }

            if let Some(state_change) = state_change {
                if let Some(values) = state_change.action.into_value() {
                    for (d, s) in buf.iter_mut().zip(values.into_iter()) {
                        *d = s;
                    }
                    Ok(f(buf))
                } else {
                    self.state.with_operand_values(operand, |values| {
                        buf.copy_from_slice(values);
                        f(buf)
                    })
                }
            } else {
                // no state change, redo error
                res
            }
            .map_err(Error::State)
        } else {
            res.map_err(Error::State)
        }
    }

    fn read_operand(
        &mut self,
        operand: &Operand,
        buf: &mut [u8],
        kind: ViolationSource,
    ) -> Result<(), Error> {
        self.read_operand_with(operand, buf, kind, |_| ())
    }

    fn write_operand(&mut self, operand: &Operand, buf: &[u8]) -> Result<(), Error> {
        let res = self
            .state
            .with_operand_values_mut(operand, |values| values.copy_from_slice(&buf));

        if let Err(pcode::Error::Memory(ref e)) = res {
            let mut state_changed = false;
            let mut skipped = false;
            let (address, size) = e.access();

            debug_assert_eq!(size, buf.len());

            for hook in self.hooks.iter_mut() {
                let res = hook
                    .hook_invalid_memory_access(
                        &mut self.state,
                        &address,
                        size,
                        ViolationSource::Write,
                    )
                    .map_err(Error::Hook)?;
                state_changed |= res.state_changed;
                skipped |= res.action.is_skip();
            }

            if state_changed {
                let res = self.state
                    .with_operand_values_mut(operand, |values| values.copy_from_slice(&buf));
                if res.is_err() && skipped {
                    Ok(())
                } else {
                    res
                }
            } else if skipped {
                Ok(())
            } else {
                // no state change, redo error
                res
            }
            .map_err(Error::State)?
        } else {
            res.map_err(Error::State)?
        }

        for hook in self.hooks.iter_mut() {
            hook.hook_operand_write(&mut self.state, operand, &buf)
                .map_err(Error::Hook)?;
        }

        Ok(())
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

        self.read_operand(rhs, &mut rbuf[..rsize], ViolationSource::Read)?;

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

        self.read_operand(lhs, &mut lbuf[..lsize], ViolationSource::Read)?;
        self.read_operand(rhs, &mut rbuf[..rsize], ViolationSource::Read)?;

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

        self.read_operand(rhs, &mut rbuf, ViolationSource::Read)?;

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

        self.read_operand(lhs, &mut lbuf, ViolationSource::Read)?;
        self.read_operand(rhs, &mut rbuf, ViolationSource::Read)?;

        op(bool::from_bytes::<O>(&lbuf), bool::from_bytes::<O>(&rbuf))?
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

        self.read_operand(rhs, &mut rbuf[..rsize], ViolationSource::Read)?;

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

        self.read_operand(lhs, &mut lbuf[..lsize], ViolationSource::Read)?;
        self.read_operand(rhs, &mut rbuf[..rsize], ViolationSource::Read)?;

        let lhs_val = format.from_bitvec(&BitVec::from_bytes::<O>(&lbuf[..lsize], false));
        let rhs_val = format.from_bitvec(&BitVec::from_bytes::<O>(&rbuf[..rsize], false));

        op(lhs_val, rhs_val, &format)?.expand_as(self, dest, true)?;

        Ok(Outcome::Branch(Branch::Next))
    }

    fn with_return_location<U, F>(&self, f: F) -> Result<U, Error>
    where
        F: FnOnce(&Operand) -> Result<U, Error>,
    {
        match &*self.state.registers().return_location() {
            ReturnLocation::Register(ref operand) => f(&operand.clone()),
            ReturnLocation::Relative(ref operand, offset) => {
                let address = self.state.get_address(operand).map_err(Error::State)?;
                let operand = Operand::Address {
                    value: Address::new(&*self.state.memory_space(), u64::from(address + *offset)),
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

            self.set_address_value(&stack_pointer, address + extra_pop)?;
        }

        Ok(AddressValue::new(
            self.state.memory_space(),
            u64::from(address),
        ))
    }

    #[inline]
    fn get_address_value(
        &mut self,
        pointer: &Operand,
        source: ViolationSource,
    ) -> Result<u64, Error> {
        let mut buf = [0u8; MAX_POINTER_SIZE];
        let psize = pointer.size();

        for hook in self.hooks.iter_mut() {
            hook.hook_operand_read(&mut self.state, pointer)
                .map_err(Error::Hook)?;
        }

        let address = if psize == POINTER_64_SIZE {
            self.read_operand_with(pointer, &mut buf[..psize], source, |buf| {
                u64::from_bytes::<O>(buf)
            })?
        } else if psize == POINTER_32_SIZE {
            self.read_operand_with(pointer, &mut buf[..psize], source, |buf| {
                u32::from_bytes::<O>(buf) as u64
            })?
        } else if psize == POINTER_16_SIZE {
            self.read_operand_with(pointer, &mut buf[..psize], source, |buf| {
                u16::from_bytes::<O>(buf) as u64
            })?
        } else if psize == POINTER_8_SIZE {
            self.read_operand_with(pointer, &mut buf[..psize], source, |buf| {
                u8::from_bytes::<O>(buf) as u64
            })?
        } else {
            return Err(Error::UnsupportedAddressSize(pointer.size()));
        };

        Ok(address)
    }

    #[inline]
    fn set_address_value<A>(
        &mut self,
        pointer: &Operand,
        value: A,
    ) -> Result<(), Error>
    where A: IntoAddress {
        let mut buf = [0u8; MAX_POINTER_SIZE];
        let psize = pointer.size();

        let address = value.into_address(self.state.memory_space_ref());

        if psize == POINTER_64_SIZE {
            u64::from(address).into_bytes::<O>(&mut buf[..psize]);
            self.write_operand(pointer, &buf[..psize])
        } else if psize == POINTER_32_SIZE {
            u32::from(address).into_bytes::<O>(&mut buf[..psize]);
            self.write_operand(pointer, &buf[..psize])
        } else if psize == POINTER_16_SIZE {
            u16::from(address).into_bytes::<O>(&mut buf[..psize]);
            self.write_operand(pointer, &buf[..psize])
        } else if psize == POINTER_8_SIZE {
            u8::from(address).into_bytes::<O>(&mut buf[..psize]);
            self.write_operand(pointer, &buf[..psize])
        } else {
            Err(Error::UnsupportedAddressSize(psize))
        }
    }

    #[inline]
    fn copy_operand(&mut self, source: &Operand, destination: &Operand) -> Result<(), Error> {
        let size = source.size();
        if size > OPERAND_SIZE {
            return Err(Error::UnsupportedOperandSize(size, OPERAND_SIZE));
        }

        let mut buf = [0u8; OPERAND_SIZE];

        self.read_operand(source, &mut buf[..size], ViolationSource::Read)?;
        self.write_operand(destination, &buf[..size])?;

        Ok(())
    }
}

impl<O: Order, R: Clone + Default + 'static, const OPERAND_SIZE: usize> Interpreter
    for ConcreteContext<O, R, { OPERAND_SIZE }>
{
    type State = PCodeState<u8, O>;
    type Error = Error;
    type Outcome = R;

    fn fork(&self) -> Self {
        Self {
            database: self.database.clone(),
            translator: self.translator.clone(),
            translator_context: self.translator.context_database(),
            translator_cache: self.translator_cache.clone(),
            hook_names: self.hook_names.clone(),
            hooks: self.hooks.clone(),
            intrinsics: self.intrinsics.clone(),
            state: self.state.fork(),
            marker: self.marker,
        }
    }

    fn restore(&mut self, other: &Self) {
        self.hooks = other.hooks.clone();
        self.hook_names = other.hook_names.clone();
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
        space: AddressSpaceId,
    ) -> Result<Outcome<R>, Error> {
        let offset = self.get_address_value(source, ViolationSource::ReadVia)?;

        let space = self.translator.manager().space_by_id(space);
        let space_size = space.address_size();
        let space_word_size = space.word_size() as u64;

        debug_assert_eq!(space_size, source.size());

        let addr_val = offset.wrapping_mul(space_word_size)
            & 1u64
                .checked_shl(space_size.checked_shl(3).unwrap_or(0) as u32)
                .unwrap_or(0)
                .wrapping_sub(1);

        let address = Operand::Address {
            value: Address::new(space, addr_val),
            size: destination.size(),
        };

        self.copy_operand(&address, destination)?;

        Ok(Outcome::Branch(Branch::Next))
    }

    fn store(
        &mut self,
        source: &Operand,
        destination: &Operand,
        space: AddressSpaceId,
    ) -> Result<Outcome<R>, Error> {
        // Same semantics as copy and load, just with different address spaces
        let offset = self.get_address_value(destination, ViolationSource::WriteVia)?;

        let space = self.translator.manager().space_by_id(space);
        let space_size = space.address_size();
        let space_word_size = space.word_size() as u64;

        debug_assert_eq!(space_size, destination.size());

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
            value: Address::new(space, addr_val),
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
                let address_value = value.into_address_value(self.state.memory_space_ref());
                let action = Branch::Global(address_value);
                Ok(Outcome::Branch(action))
            }
            Operand::Register { .. } => {
                return Err(Error::UnsupportedBranchDestination(
                    self.translator.manager().register_space_id(),
                ))
            }
            Operand::Variable { space, .. } => {
                return Err(Error::UnsupportedBranchDestination(*space))
            }
        }
    }

    fn cbranch(&mut self, destination: &Operand, condition: &Operand) -> Result<Outcome<R>, Error> {
        assert!(condition.size() == 1);

        let mut flip = false;

        // Invoke hook
        for hook in self.hooks.iter_mut() {
            match hook
                .hook_cbranch(&mut self.state, destination, condition)
                .map_err(Error::Hook)?
                .action
            {
                HookCBranchAction::Pass => (),
                HookCBranchAction::Flip => { flip = true; },
                HookCBranchAction::Halt(r) => return Ok(Outcome::Halt(r))
            }
        }

        // The hook may change the condition value, so we need to read it here
        let mut buf = [0u8; 1];
        self.read_operand(condition, &mut buf[..], ViolationSource::Read)?;

        let condition_value = {
            let v = bool::from_bytes::<O>(&buf);
            if flip {
                let nv = !v;
                log::trace!("flipped branch condition {} to {}", condition, nv);
                self.state.set_operand(condition, nv)?;
                nv
            } else {
                v
            }
        };

        if condition_value {
            self.branch(destination)
        } else {
            Ok(Outcome::Branch(Branch::Next))
        }
    }

    fn ibranch(&mut self, destination: &Operand) -> Result<Outcome<R>, Error> {
        if destination == &*self.state.registers().program_counter() {
            return self.icall(destination);
        }

        let address = AddressValue::new(
            self.state.memory_space(),
            self.get_address_value(destination, ViolationSource::ReadVia)?,
        );
        Ok(Outcome::Branch(Branch::Global(address)))
    }

    fn call(&mut self, destination: &Operand) -> Result<Outcome<R>, Error> {
        match destination {
            Operand::Address { value, .. } => {
                let mut skip = false;
                let address_value = value.into_address_value(self.state.memory_space_ref());
                for hook in self.hooks.iter_mut() {
                    match hook
                        .hook_call(&mut self.state, &(&address_value).into())
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
            Operand::Constant { .. } => Err(Error::UnsupportedBranchDestination(
                self.translator.manager().constant_space_id(),
            )),
            Operand::Register { .. } => Err(Error::UnsupportedBranchDestination(
                self.translator.manager().register_space_id(),
            )),
            Operand::Variable { space, .. } => {
                Err(Error::UnsupportedBranchDestination(space.clone()))
            }
        }
    }

    fn icall(&mut self, destination: &Operand) -> Result<Outcome<R>, Error> {
        let address_value = AddressValue::new(
            self.state.memory_space(),
            self.get_address_value(destination, ViolationSource::ReadVia)?,
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
        let address = AddressValue::new(
            self.state.memory_space(),
            self.get_address_value(destination, ViolationSource::ReadVia)?,
        );
        Ok(Outcome::Branch(Branch::Global(address)))
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
            |u, v| Ok(u.signed_carry(&v)),
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
            |u, v| Ok(u.signed_borrow(&v)),
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
                let ival = u.as_bigint().into_owned();
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

        let amount = self.read_operand_with(
            amount,
            &mut buf[..amount_size],
            ViolationSource::Read,
            |buf| {
                BitVec::from_bytes::<O>(&buf[..amount_size], false)
                    .to_usize()
                    .expect("subpiece `amount` can be stored within usize")
            },
        )?;

        let mut input_buf = [0u8; OPERAND_SIZE];
        let input_view = &mut input_buf[..input_size];

        self.read_operand(operand, input_view, ViolationSource::Read)?;

        let mut output_buf = [0u8; OPERAND_SIZE];
        let output_view = &mut output_buf[..destination_size];

        O::subpiece(output_view, input_view, amount);

        self.write_operand(destination, &output_view)?;

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
        name: &str,
        operands: &[Operand],
        result: Option<&Operand>,
    ) -> Result<Outcome<R>, Error> {
        // TODO: should we trigger operand read events on intrinsics?

        let outcome = self
            .intrinsics
            .handle(name, &mut self.state, operands, result)
            .map_err(Error::Intrinsic)?;

        Ok(match outcome {
            IntrinsicAction::Pass => Outcome::Branch(Branch::Next),
            IntrinsicAction::Branch(address) => Outcome::Branch(Branch::Global(address)),
            IntrinsicAction::Halt(reason) => Outcome::Halt(reason),
        })
    }

    fn lift<A>(&mut self, address: A) -> Result<OrOutcome<StepState, Self::Outcome>, Error>
    where
        A: IntoAddress,
    {
        let address_value = address.into_address_value(self.state.memory_space_ref());
        let address = Address::from(&address_value);

        // begin read lock region
        let rlock = self.translator_cache.read();

        let cached = rlock.get(&address).map(|step_state| step_state.clone());

        drop(rlock);
        // end read lock region

        let step_state = if let Some(step_state) = cached {
            step_state.clone()
        } else {
            // NOTE: possible race here, if another thread populates
            // the same address. We don't really care, I suppose.

            let view = self
                .state
                .view_values_from(address)
                .map_err(Error::State)?;
            let step_state = StepState::from(
                self.translator
                    .lift_pcode(&mut self.translator_context, address_value, view)
                    .map_err(|e| Error::Lift(address, e))?,
            );

            self.translator_cache
                .write()
                .insert(address, step_state.clone());

            step_state
        };

        // TODO: handle outcomes
        for hook in self.hooks.iter_mut() {
            hook.hook_architectural_step(&mut self.state, &address, &step_state)
                .map_err(Error::Hook)?;
        }

        let program_counter = self.state.registers().program_counter().clone();
        self.state
            .set_address(&program_counter, address)
            .map_err(Error::State)?;

        Ok(step_state.into())
    }

    fn operation(&mut self, location: &Location, step: &PCodeOp) -> Result<OrOutcome<(), Self::Outcome>, Self::Error> {
        // TODO: handle outcomes
        for hook in self.hooks.iter_mut() {
            hook.hook_operation_step(&mut self.state, location, step)
                .map_err(Error::Hook)?;
        }

        Ok(().into())
    }

    fn interpreter_space(&self) -> Arc<AddressSpace> {
        self.state.memory_space()
    }
}
