#![cfg(all(feature = "test_utils", not(target_os = "solana")))]
//! Tools used in testing of the Pinocchio library itself.  Mostly when writing unit tests.
//!
//! Some of these could be useful when testing programs that use Pinocchio, or even programs that do
//! not use Pinocchio.

use std::{vec, vec::Vec};

use crate::pubkey::Pubkey;

pub struct MockFullAccountData {
    pub signer: bool,
    pub writable: bool,
    pub executable: bool,
    pub resize_delta: i32,
    pub address: Pubkey,
    pub owner: Pubkey,
    pub balance: u64,
    pub data: Vec<u8>,
}

impl MockFullAccountData {
    pub fn new() -> Self {
        MockFullAccountData {
            signer: false,
            writable: true,
            executable: false,
            resize_delta: 0,
            address: Pubkey::new_tl_unique(),
            owner: Pubkey::new_tl_unique(),
            balance: 0,
            data: vec![],
        }
    }

    pub fn with_address(mut self, address: Pubkey) -> Self {
        self.address = address;
        self
    }

    pub fn with_owner(mut self, owner: Pubkey) -> Self {
        self.owner = owner;
        self
    }

    pub fn with_balance(mut self, balance: u64) -> Self {
        self.balance = balance;
        self
    }

    pub fn with_data(mut self, data: Vec<u8>) -> Self {
        self.data = data;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mock_full_account_data() {
        let mock_account = MockFullAccountData::new()
            .with_address(Pubkey::new_tl_unique())
            .with_owner(Pubkey::new_tl_unique())
            .with_balance(100)
            .with_data(vec![1, 2, 3]);

        assert_eq!(mock_account.address, Pubkey::new_tl_unique());
        assert_eq!(mock_account.owner, Pubkey::new_tl_unique());
        assert_eq!(mock_account.balance, 100);
        assert_eq!(mock_account.data, vec![1, 2, 3]);
    }
}
