use fugue::ir::il::ecode::Location;
use fugue::ir::il::pcode::{Operand, PCodeOp, Register};
use fugue::ir::Address;
use fuguex_machine::StepState;
use fuguex_microx::types::HookInvalidAccessAction;
use fuguex_state::StateOps;

use downcast_rs::{impl_downcast, Downcast};
use dyn_clone::{clone_trait_object, DynClone};

use fuguex_hooks::{
    Error, HookAction, HookCBranchAction, HookCallAction, HookOutcome, HookStepAction,
};
use fuguex_microx::ViolationSource;

#[allow(unused)]
pub trait HookConcrete: Downcast {
    type State: StateOps;
    type Error: std::error::Error + Send + Sync + 'static;
    type Outcome;

    fn hook_memory_read(
        &mut self,
        state: &mut Self::State,
        address: &Address,
        size: usize,
    ) -> Result<HookOutcome<HookAction<Self::Outcome>>, Error<Self::Error>> {
        Ok(HookAction::Pass.into())
    }

    fn hook_memory_write(
        &mut self,
        state: &mut Self::State,
        address: &Address,
        size: usize,
        value: &[<Self::State as StateOps>::Value],
    ) -> Result<HookOutcome<HookAction<Self::Outcome>>, Error<Self::Error>> {
        Ok(HookAction::Pass.into())
    }

    fn hook_invalid_memory_access(
        &mut self,
        state: &mut Self::State,
        address: &Address,
        size: usize,
        source: ViolationSource,
    ) -> Result<
        HookOutcome<HookInvalidAccessAction<Self::Outcome, <Self::State as StateOps>::Value>>,
        Error<Self::Error>,
    > {
        Ok(HookInvalidAccessAction::Pass.into())
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
        value: &[<Self::State as StateOps>::Value],
    ) -> Result<HookOutcome<HookAction<Self::Outcome>>, Error<Self::Error>> {
        Ok(HookAction::Pass.into())
    }

    fn hook_operand_read(
        &mut self,
        state: &mut Self::State,
        operand: &Operand,
    ) -> Result<HookOutcome<HookAction<Self::Outcome>>, Error<Self::Error>> {
        match operand {
            Operand::Address {
                value: address,
                size,
            } => self.hook_memory_read(state, &address, *size),
            Operand::Register { .. } => {
                self.hook_register_read(state, &operand.register().unwrap())
            }
            _ => Ok(HookAction::Pass.into()),
        }
    }

    fn hook_operand_write(
        &mut self,
        state: &mut Self::State,
        operand: &Operand,
        value: &[<Self::State as StateOps>::Value],
    ) -> Result<HookOutcome<HookAction<Self::Outcome>>, Error<Self::Error>> {
        match operand {
            Operand::Address {
                value: address,
                size,
            } => self.hook_memory_write(state, &address, *size, value),
            Operand::Register { .. } => {
                self.hook_register_write(state, &operand.register().unwrap(), value)
            }
            _ => Ok(HookAction::Pass.into()),
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

    fn hook_operation_step(
        &mut self,
        state: &mut Self::State,
        location: &Location,
        operation: &PCodeOp,
    ) -> Result<HookOutcome<HookStepAction<Self::Outcome>>, Error<Self::Error>> {
        Ok(HookStepAction::Pass.into())
    }

    fn hook_architectural_step(
        &mut self,
        state: &mut Self::State,
        address: &Address,
        operation: &StepState,
    ) -> Result<HookOutcome<HookStepAction<Self::Outcome>>, Error<Self::Error>> {
        Ok(HookStepAction::Pass.into())
    }
}

pub trait ClonableHookConcrete: DynClone + HookConcrete {}
clone_trait_object!(
    <State, Error, Outcome> ClonableHookConcrete<State=State, Error=Error, Outcome=Outcome>
    where State: fuguex_state::StateOps,
          Error: std::error::Error + Send + Sync + 'static,
);

impl_downcast!(
    ClonableHookConcrete assoc State, Outcome, Error
    where
        State: StateOps,
        Error: std::error::Error + Send + Sync
);
