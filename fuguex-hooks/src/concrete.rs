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
pub trait HookConcrete: Hook {
    fn hook_memory_read(
        &mut self,
        state: &mut Self::State,
        address: &Address,
        value: &[<Self::State as State>::Value]
    ) -> Result<(), Self::Error> {
        Ok(())
    }

    fn hook_memory_write(
        &mut self,
        state: &mut Self::State,
        address: &Address,
        value: &mut [<Self::State as State>::Value]
    ) -> Result<(), Self::Error> {
        Ok(())
    }

    fn hook_register_read(
        &mut self,
        state: &mut Self::State,
        register: &Register,
        value: &[<Self::State as State>::Value]
    ) -> Result<(), Self::Error> {
        Ok(())
    }

    fn hook_register_write(
        &mut self,
        state: &mut Self::State,
        register: &Register,
        value: &mut [<Self::State as State>::Value]
    ) -> Result<(), Self::Error> {
        Ok(())
    }

    fn hook_call(
        &mut self,
        state: &mut Self::State,
        destination: &Address,
    ) -> Result<(), Self::Error> {
        Ok(())
    }

    fn hook_branch(
        &mut self,
        state: &mut Self::State,
        destination: &Address,
    ) -> Result<(), Self::Error> {
        Ok(())
    }

    fn hook_cbranch(
        &mut self,
        state: &mut Self::State,
        destination: &Address,
        condition: &Operand,
    ) -> Result<(), Self::Error> {
        Ok(())
    }
}

impl<'space, T> HookMemoryRead for T where T: HookConcrete {
    fn hook_memory_read(&mut self, state: &mut Self::State, address: &Address, value: &[<Self::State as State>::Value]) -> Result<(), Self::Error> {
        <Self as HookConcrete>::hook_memory_read(self, state, address, value)
    }
}

impl<'space, T> HookMemoryWrite for T where T: HookConcrete {
    fn hook_memory_write(&mut self, state: &mut Self::State, address: &Address, value: &mut [<Self::State as State>::Value]) -> Result<(), Self::Error> {
        <Self as HookConcrete>::hook_memory_write(self, state, address, value)
    }
}

impl<'space, T> HookRegisterRead for T where T: HookConcrete {
    fn hook_register_read(&mut self, state: &mut Self::State, register: &Register, value: &[<Self::State as State>::Value]) -> Result<(), Self::Error> {
        <Self as HookConcrete>::hook_register_read(self, state, register, value)
    }
}

impl<'space, T> HookRegisterWrite for T where T: HookConcrete {
    fn hook_register_write(&mut self, state: &mut Self::State, register: &Register, value: &mut [<Self::State as State>::Value]) -> Result<(), Self::Error> {
        <Self as HookConcrete>::hook_register_write(self, state, register, value)
    }
}

impl<'space, T> HookCall for T where T: HookConcrete {
    fn hook_call(&mut self, state: &mut Self::State, destination: &Address) -> Result<(), Self::Error> {
        <Self as HookConcrete>::hook_call(self, state, destination)
    }
}

impl<'space, T> HookBranch for T where T: HookConcrete {
    fn hook_branch(&mut self, state: &mut Self::State, destination: &Address) -> Result<(), Self::Error> {
        <Self as HookConcrete>::hook_branch(self, state, destination)
    }
}

impl<'space, T> HookCBranch for T where T: HookConcrete {
    fn hook_cbranch(&mut self, state: &mut Self::State, destination: &Address, condition: &Operand) -> Result<(), Self::Error> {
        <Self as HookConcrete>::hook_cbranch(self, state, destination, condition)
    }
}
