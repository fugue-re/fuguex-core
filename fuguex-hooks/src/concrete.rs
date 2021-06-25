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
use crate::types::{HookAction, HookBranchAction, HookCBranchAction, HookCallAction};

#[allow(unused)]
pub trait HookConcrete: Hook {
    fn hook_memory_read(
        &mut self,
        state: &mut Self::State,
        address: &Address,
        value: &[<Self::State as State>::Value]
    ) -> Result<HookAction, Self::Error> {
        Ok(HookAction::Pass)
    }

    fn hook_memory_write(
        &mut self,
        state: &mut Self::State,
        address: &Address,
        value: &mut [<Self::State as State>::Value]
    ) -> Result<HookAction, Self::Error> {
        Ok(HookAction::Pass)
    }

    fn hook_register_read(
        &mut self,
        state: &mut Self::State,
        register: &Register,
        value: &[<Self::State as State>::Value]
    ) -> Result<HookAction, Self::Error> {
        Ok(HookAction::Pass)
    }

    fn hook_register_write(
        &mut self,
        state: &mut Self::State,
        register: &Register,
        value: &mut [<Self::State as State>::Value]
    ) -> Result<HookAction, Self::Error> {
        Ok(HookAction::Pass)
    }

    fn hook_call(
        &mut self,
        state: &mut Self::State,
        destination: &Address,
    ) -> Result<HookCallAction, Self::Error> {
        Ok(HookCallAction::Pass)
    }

    fn hook_branch(
        &mut self,
        state: &mut Self::State,
        destination: &Address,
    ) -> Result<HookBranchAction, Self::Error> {
        Ok(HookBranchAction::Pass)
    }

    fn hook_cbranch(
        &mut self,
        state: &mut Self::State,
        destination: &Address,
        condition: &Operand,
    ) -> Result<HookCBranchAction, Self::Error> {
        Ok(HookCBranchAction::Pass)
    }
}

impl<'space, T> HookMemoryRead for T where T: HookConcrete {
    fn hook_memory_read(&mut self, state: &mut Self::State, address: &Address, value: &[<Self::State as State>::Value]) -> Result<HookAction, Self::Error> {
        <Self as HookConcrete>::hook_memory_read(self, state, address, value)
    }
}

impl<'space, T> HookMemoryWrite for T where T: HookConcrete {
    fn hook_memory_write(&mut self, state: &mut Self::State, address: &Address, value: &mut [<Self::State as State>::Value]) -> Result<HookAction, Self::Error> {
        <Self as HookConcrete>::hook_memory_write(self, state, address, value)
    }
}

impl<'space, T> HookRegisterRead for T where T: HookConcrete {
    fn hook_register_read(&mut self, state: &mut Self::State, register: &Register, value: &[<Self::State as State>::Value]) -> Result<HookAction, Self::Error> {
        <Self as HookConcrete>::hook_register_read(self, state, register, value)
    }
}

impl<'space, T> HookRegisterWrite for T where T: HookConcrete {
    fn hook_register_write(&mut self, state: &mut Self::State, register: &Register, value: &mut [<Self::State as State>::Value]) -> Result<HookAction, Self::Error> {
        <Self as HookConcrete>::hook_register_write(self, state, register, value)
    }
}

impl<'space, T> HookCall for T where T: HookConcrete {
    fn hook_call(&mut self, state: &mut Self::State, destination: &Address) -> Result<HookCallAction, Self::Error> {
        <Self as HookConcrete>::hook_call(self, state, destination)
    }
}

impl<'space, T> HookBranch for T where T: HookConcrete {
    fn hook_branch(&mut self, state: &mut Self::State, destination: &Address) -> Result<HookBranchAction, Self::Error> {
        <Self as HookConcrete>::hook_branch(self, state, destination)
    }
}

impl<'space, T> HookCBranch for T where T: HookConcrete {
    fn hook_cbranch(&mut self, state: &mut Self::State, destination: &Address, condition: &Operand) -> Result<HookCBranchAction, Self::Error> {
        <Self as HookConcrete>::hook_cbranch(self, state, destination, condition)
    }
}
