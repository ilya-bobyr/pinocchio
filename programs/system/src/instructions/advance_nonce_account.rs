use pinocchio::{
    account_info::AccountInfo,
    instruction::{AccountMeta, Instruction, Signer},
    program::invoke_signed,
    pubkey::Pubkey,
    ProgramResult,
};

use crate::CanInvoke;

/// Consumes a stored nonce, replacing it with a successor.
///
/// ### Accounts:
///   0. `[WRITE]` Nonce account
///   1. `[]` Recent blockhashes sysvar
///   2. `[SIGNER]` Nonce authority
pub struct AdvanceNonceAccount<'a> {
    /// Nonce account.
    pub account: &'a AccountInfo,

    /// Recent blockhashes sysvar.
    pub recent_blockhashes_sysvar: &'a AccountInfo,

    /// Nonce authority.
    pub authority: &'a AccountInfo,
}

impl AdvanceNonceAccount<'_> {
    #[inline(always)]
    pub fn invoke(&self) -> ProgramResult {
        self.invoke_signed(&[])
    }

    #[inline(always)]
    pub fn invoke_signed(&self, signers: &[Signer]) -> ProgramResult {
        // account metadata
        let account_metas: [AccountMeta; 3] = [
            AccountMeta::writable(self.account.key()),
            AccountMeta::readonly(self.recent_blockhashes_sysvar.key()),
            AccountMeta::readonly_signer(self.authority.key()),
        ];

        // instruction
        let instruction = Instruction {
            program_id: &crate::ID,
            accounts: &account_metas,
            data: &[4],
        };

        invoke_signed(
            &instruction,
            &[self.account, self.recent_blockhashes_sysvar, self.authority],
            signers,
        )
    }
}

const ACCOUNTS_LEN: usize = 3;

impl CanInvoke<ACCOUNTS_LEN> for AdvanceNonceAccount<'_> {
    fn invoke_via(
        &self,
        invoke: impl FnOnce(
            /* program_id: */ &Pubkey,
            /* accounts: */ &[&AccountInfo; ACCOUNTS_LEN],
            /* account_metas: */ &[AccountMeta],
            /* data: */ &[u8],
        ) -> ProgramResult,
    ) -> ProgramResult {
        invoke(
            &crate::ID,
            &[self.account, self.recent_blockhashes_sysvar, self.authority],
            &[
                AccountMeta::writable(self.account.key()),
                AccountMeta::readonly(self.recent_blockhashes_sysvar.key()),
                AccountMeta::readonly_signer(self.authority.key()),
            ],
            &[4],
        )
    }
}
