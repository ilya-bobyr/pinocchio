use pinocchio::{
    account_info::AccountInfo, instruction::AccountMeta, pubkey::Pubkey, ProgramResult,
};

use crate::CanInvoke;

/// Transfer lamports.
///
/// ### Accounts:
///   0. `[WRITE, SIGNER]` Funding account
///   1. `[WRITE]` Recipient account
pub struct Transfer<'a> {
    /// Funding account.
    pub from: &'a AccountInfo,

    /// Recipient account.
    pub to: &'a AccountInfo,

    /// Amount of lamports to transfer.
    pub lamports: u64,
}

const ACCOUNTS_LEN: usize = 2;

impl CanInvoke for Transfer<'_> {
    type Accounts<'a> = [&'a AccountInfo; ACCOUNTS_LEN];

    fn invoke_via(
        &self,
        invoke: impl for<'a> FnOnce(
            /* program_id: */ &'a Pubkey,
            /* accounts: */ &'a [&'a AccountInfo; ACCOUNTS_LEN],
            /* account_metas: */ &'a [AccountMeta],
            /* data: */ &'a [u8],
        ) -> ProgramResult,
        _slice_invoke: impl for<'a> FnOnce(
            /* program_id: */ &'a Pubkey,
            /* accounts: */ &'a [&'a AccountInfo],
            /* account_metas: */ &'a [AccountMeta],
            /* data: */ &'a [u8],
        ) -> ProgramResult,
    ) -> ProgramResult {
        // instruction data
        // -  [0..4 ]: instruction discriminator
        // -  [4..12]: lamports amount
        let mut instruction_data = [0; 12];
        instruction_data[0] = 2;
        instruction_data[4..12].copy_from_slice(&self.lamports.to_le_bytes());

        invoke(
            &crate::ID,
            &[self.from, self.to],
            &[
                AccountMeta::writable_signer(self.from.key()),
                AccountMeta::writable(self.to.key()),
            ],
            &instruction_data,
        )
    }
}

#[cfg(test)]
mod tests {
    use pinocchio::{
        account_info::{Account, AccountInfo},
        ProgramResult,
    };

    use crate::Invoke as _;

    use super::Transfer;

    const NOT_BORROWED: u8 = u8::MAX;

    #[test]
    fn simple_transfer() {
        // 8-bytes aligned account data.
        let mut from_data = {
            let mut data = [0u64; size_of::<Account>() / size_of::<u64>()];
            data[0] = NOT_BORROWED as u64;
            data
        };
        let from = AccountInfo {
            raw: from_data.as_mut_ptr() as *mut Account,
        };
        let mut to_data = {
            let mut data = [0u64; size_of::<Account>() / size_of::<u64>()];
            data[0] = NOT_BORROWED as u64;
            data
        };
        let to = AccountInfo {
            raw: to_data.as_mut_ptr() as *mut Account,
        };

        let res = Transfer {
            from: &from,
            to: &to,
            lamports: 42,
        }
        .invoke();
        assert_eq!(res, ProgramResult::Ok(()));
    }
}
