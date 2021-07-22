use fugue::ir::Address;
use fugue::ir::il::pcode::Operand;

use fugue::ir::il::pcode::Register;
use fuguex_state::State;

use dyn_clone::{DynClone, clone_trait_object};

use crate::traits::{
    Hook,
    HookMemoryRead,
    HookMemoryWrite,
    HookRegisterRead,
    HookRegisterWrite,
    HookCall,
    HookCBranch,
};
use crate::types::{Error, HookAction, HookCBranchAction, HookCallAction, HookOutcome};

#[allow(unused)]
pub trait HookConcrete: Hook {
    fn hook_memory_read(
        &mut self,
        state: &mut Self::State,
        address: &Address,
    ) -> Result<HookOutcome<HookAction<Self::Outcome>>, Error<Self::Error>> {
        Ok(HookAction::Pass.into())
    }

    fn hook_memory_write(
        &mut self,
        state: &mut Self::State,
        address: &Address,
        value: &[<Self::State as State>::Value]
    ) -> Result<HookOutcome<HookAction<Self::Outcome>>, Error<Self::Error>> {
        Ok(HookAction::Pass.into())
    }

    fn hook_register_read(
        &mut self,
        state: &mut Self::State,
        register: &Register,
    ) -> Result<HookOutcome<HookAction<Self::Outcome>>, Error<Self::Error>> {
        Ok(HookAction::Pass.into())
    }

    fn hook_register_write(
        &mut self,
        state: &mut Self::State,
        register: &Register,
        value: &[<Self::State as State>::Value]
    ) -> Result<HookOutcome<HookAction<Self::Outcome>>, Error<Self::Error>> {
        Ok(HookAction::Pass.into())
    }

    fn hook_operand_read(
        &mut self,
        state: &mut Self::State,
        operand: &Operand,
    ) -> Result<HookOutcome<HookAction<Self::Outcome>>, Error<Self::Error>> {
        match operand {
            Operand::Address { value: address, .. } => {
                self.hook_memory_read(state, &address.into())
            },
            Operand::Register { .. } => {
                self.hook_register_read(state, &operand.register().unwrap())
            },
            _ => Ok(HookAction::Pass.into())
        }
    }

    fn hook_operand_write(
        &mut self,
        state: &mut Self::State,
        operand: &Operand,
        value: &[<Self::State as State>::Value]
    ) -> Result<HookOutcome<HookAction<Self::Outcome>>, Error<Self::Error>> {
        match operand {
            Operand::Address { value: address, .. } => {
                self.hook_memory_write(state, &address.into(), value)
            },
            Operand::Register { .. } => {
                self.hook_register_write(state, &operand.register().unwrap(), value)
            },
            _ => Ok(HookAction::Pass.into())
        }
    }

    fn hook_call(
        &mut self,
        state: &mut Self::State,
        destination: &Address,
    ) -> Result<HookOutcome<HookCallAction<Self::Outcome>>, Error<Self::Error>> {
        Ok(HookCallAction::Pass.into())
    }

    fn hook_cbranch(
        &mut self,
        state: &mut Self::State,
        destination: &Operand,
        condition: &Operand,
    ) -> Result<HookOutcome<HookCBranchAction<Self::Outcome>>, Error<Self::Error>> {
        Ok(HookCBranchAction::Pass.into())
    }
}

impl<T> HookMemoryRead for T where T: HookConcrete {
    fn hook_memory_read(&mut self, state: &mut Self::State, address: &Address) -> Result<HookOutcome<HookAction<Self::Outcome>>, Error<Self::Error>> {
        <Self as HookConcrete>::hook_memory_read(self, state, address)
    }
}

impl<T> HookMemoryWrite for T where T: HookConcrete {
    fn hook_memory_write(&mut self, state: &mut Self::State, address: &Address, value: &[<Self::State as State>::Value]) -> Result<HookOutcome<HookAction<Self::Outcome>>, Error<Self::Error>> {
        <Self as HookConcrete>::hook_memory_write(self, state, address, value)
    }
}

impl<T> HookRegisterRead for T where T: HookConcrete {
    fn hook_register_read(&mut self, state: &mut Self::State, register: &Register) -> Result<HookOutcome<HookAction<Self::Outcome>>, Error<Self::Error>> {
        <Self as HookConcrete>::hook_register_read(self, state, register)
    }
}

impl<T> HookRegisterWrite for T where T: HookConcrete {
    fn hook_register_write(&mut self, state: &mut Self::State, register: &Register, value: &[<Self::State as State>::Value]) -> Result<HookOutcome<HookAction<Self::Outcome>>, Error<Self::Error>> {
        <Self as HookConcrete>::hook_register_write(self, state, register, value)
    }
}

impl<T> HookCall for T where T: HookConcrete {
    fn hook_call(&mut self, state: &mut Self::State, destination: &Address) -> Result<HookOutcome<HookCallAction<Self::Outcome>>, Error<Self::Error>> {
        <Self as HookConcrete>::hook_call(self, state, destination)
    }
}

impl<T> HookCBranch for T where T: HookConcrete {
    fn hook_cbranch(&mut self, state: &mut Self::State, destination: &Operand, condition: &Operand) -> Result<HookOutcome<HookCBranchAction<Self::Outcome>>, Error<Self::Error>> {
        <Self as HookConcrete>::hook_cbranch(self, state, destination, condition)
    }
}

pub trait ClonableHookConcrete: DynClone + HookConcrete { }
clone_trait_object!(
    <State, Error, Outcome> ClonableHookConcrete<State=State, Error=Error, Outcome=Outcome>
    where State: fuguex_state::State,
          Error: std::error::Error + Send + Sync + 'static,
);
