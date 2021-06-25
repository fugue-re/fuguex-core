use fugue::ir::Address;
use fugue::ir::il::pcode::Operand;

use fugue::ir::il::pcode::Register;
use fuguex_state::State;

use crate::traits::{
    Hook,
    HookMemoryRead,
    HookMemoryWrite,
    HookRegisterRead,
    HookRegisterWrite,
    HookCall,
    HookBranch,
    HookCBranch,
};

#[allow(unused)]
pub trait HookConcrete<'space>: Hook<'space> {
    fn hook_memory_read(
        &mut self,
        state: &mut Self::State,
        address: &Address<'space>,
        value: &[<Self::State as State>::Value]
    ) -> Result<(), Self::Error> {
        Ok(())
    }

    fn hook_memory_write(
        &mut self,
        state: &mut Self::State,
        address: &Address<'space>,
        value: &mut [<Self::State as State>::Value]
    ) -> Result<(), Self::Error> {
        Ok(())
    }

    fn hook_register_read(
        &mut self,
        state: &mut Self::State,
        register: &Register<'space>,
        value: &[<Self::State as State>::Value]
    ) -> Result<(), Self::Error> {
        Ok(())
    }

    fn hook_register_write(
        &mut self,
        state: &mut Self::State,
        register: &Register<'space>,
        value: &mut [<Self::State as State>::Value]
    ) -> Result<(), Self::Error> {
        Ok(())
    }

    fn hook_call(
        &mut self,
        state: &mut Self::State,
        destination: &Address<'space>,
    ) -> Result<(), Self::Error> {
        Ok(())
    }

    fn hook_branch(
        &mut self,
        state: &mut Self::State,
        destination: &Address<'space>,
    ) -> Result<(), Self::Error> {
        Ok(())
    }

    fn hook_cbranch(
        &mut self,
        state: &mut Self::State,
        destination: &Address<'space>,
        condition: &Operand<'space>,
    ) -> Result<(), Self::Error> {
        Ok(())
    }
}

impl<'space, T> HookMemoryRead<'space> for T where T: HookConcrete<'space> {
    fn hook_memory_read(&mut self, state: &mut Self::State, address: &Address<'space>, value: &[<Self::State as State>::Value]) -> Result<(), Self::Error> {
        <Self as HookConcrete>::hook_memory_read(self, state, address, value)
    }
}

impl<'space, T> HookMemoryWrite<'space> for T where T: HookConcrete<'space> {
    fn hook_memory_write(&mut self, state: &mut Self::State, address: &Address<'space>, value: &mut [<Self::State as State>::Value]) -> Result<(), Self::Error> {
        <Self as HookConcrete>::hook_memory_write(self, state, address, value)
    }
}

impl<'space, T> HookRegisterRead<'space> for T where T: HookConcrete<'space> {
    fn hook_register_read(&mut self, state: &mut Self::State, register: &Register<'space>, value: &[<Self::State as State>::Value]) -> Result<(), Self::Error> {
        <Self as HookConcrete>::hook_register_read(self, state, register, value)
    }
}

impl<'space, T> HookRegisterWrite<'space> for T where T: HookConcrete<'space> {
    fn hook_register_write(&mut self, state: &mut Self::State, register: &Register<'space>, value: &mut [<Self::State as State>::Value]) -> Result<(), Self::Error> {
        <Self as HookConcrete>::hook_register_write(self, state, register, value)
    }
}

impl<'space, T> HookCall<'space> for T where T: HookConcrete<'space> {
    fn hook_call(&mut self, state: &mut Self::State, destination: &Address<'space>) -> Result<(), Self::Error> {
        <Self as HookConcrete>::hook_call(self, state, destination)
    }
}

impl<'space, T> HookBranch<'space> for T where T: HookConcrete<'space> {
    fn hook_branch(&mut self, state: &mut Self::State, destination: &Address<'space>) -> Result<(), Self::Error> {
        <Self as HookConcrete>::hook_branch(self, state, destination)
    }
}

impl<'space, T> HookCBranch<'space> for T where T: HookConcrete<'space> {
    fn hook_cbranch(&mut self, state: &mut Self::State, destination: &Address<'space>, condition: &Operand<'space>) -> Result<(), Self::Error> {
        <Self as HookConcrete>::hook_cbranch(self, state, destination, condition)
    }
}
