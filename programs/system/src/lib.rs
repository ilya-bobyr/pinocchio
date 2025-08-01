#![no_std]

use pinocchio::{
    account_info::AccountInfo,
    cpi,
    instruction::{AccountMeta, Instruction, Signer},
    pubkey::Pubkey,
    ProgramResult,
};

pub mod instructions;

pinocchio_pubkey::declare_id!("11111111111111111111111111111111");

mod sealed {
    pub trait Sealed {}
    impl<T> Sealed for T where T: crate::CanInvoke {}
}

pub trait CanInvoke {
    type Accounts<'a>;

    fn invoke_via(
        &self,
        invoke: impl for<'a> FnOnce(
            /* program_id: */ &'a Pubkey,
            /* accounts: */ &'a Self::Accounts<'a>,
            /* account_metas: */ &'a [AccountMeta],
            /* data: */ &'a [u8],
        ) -> ProgramResult,
        slice_invoke: impl for<'a> FnOnce(
            /* program_id: */ &'a Pubkey,
            /* accounts: */ &'a [&'a AccountInfo],
            /* account_metas: */ &'a [AccountMeta],
            /* data: */ &'a [u8],
        ) -> ProgramResult,
    ) -> ProgramResult;
}

pub trait Invoke: sealed::Sealed {
    fn invoke(&self) -> ProgramResult;
    fn invoke_signed(&self, signers: &[Signer]) -> ProgramResult;
}

impl<const ACCOUNTS_LEN: usize, T> Invoke for T
where
    T: for<'a> CanInvoke<Accounts<'a> = [&'a AccountInfo; ACCOUNTS_LEN]>,
{
    fn invoke(&self) -> ProgramResult {
        self.invoke_via(
            |program_id, accounts, account_metas, data| {
                let instruction = Instruction {
                    program_id,
                    accounts: &account_metas,
                    data,
                };
                cpi::invoke(&instruction, accounts)
            },
            |program_id, accounts, account_metas, data| {
                let instruction = Instruction {
                    program_id,
                    accounts: &account_metas,
                    data,
                };
                cpi::slice_invoke(&instruction, accounts)
            },
        )
    }

    fn invoke_signed(&self, signers: &[Signer]) -> ProgramResult {
        self.invoke_via(
            |program_id, accounts, account_metas, data| {
                let instruction = Instruction {
                    program_id,
                    accounts: &account_metas,
                    data,
                };
                cpi::invoke_signed(&instruction, accounts, signers)
            },
            |program_id, accounts, account_metas, data| {
                let instruction = Instruction {
                    program_id,
                    accounts: &account_metas,
                    data,
                };
                cpi::slice_invoke_signed(&instruction, accounts, signers)
            },
        )
    }
}
