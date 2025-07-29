use core::{mem::size_of, pin::Pin, slice};

use static_assertions::assert_eq_size;

use crate::{log, pubkey::Pubkey};

/// Account header as provided by the VM ABI.  This is how the account data is initially laid out
/// when provided by the VM.  For accounts that are not duplicates.
///
/// VM is ignoring values for some of the fields, and Pinocchio takes advantage of that, moving some
/// of the fields around.  Meaning that this header might not be correct after the account bytes
/// have been converted to be used by certain account types in Pinocchio.
///
/// Here is a layout of accounts that are not duplicates:
///
/// ```text,ignore
///           [0]  1 byte: duplicate reference index.  Should be 256, as this is not a duplicate
///                  account.
///                  This field is ignored by the VM upon program termination.
///
///           [1]  1 byte: is account a signer.  Non-zero means it is a signer.
///                  This field is ignored by the VM upon program termination.
///
///           [2]  1 byte: is account writable.  Non-zero means the account is writable.
///                  This field is ignored by the VM upon program termination.
///
///           [3]  1 byte: is account executable.  Non-zero means the account is executable.
///                  This field is ignored by the VM upon program termination.
///
///      [ 4.. 7]  4 bytes: resize delta.  This field is initialized to 0 by the runtime and is
///                  ignored when the program terminates.  It can be used to store the change in the
///                  account data length.  While the actual data length is stored in the "Account
///                  data length" it recorded the final value. The program might want to keep
///                  track of the difference with the original length, and this field is used for
///                  that.  Interpreted as i32.
///
///                  When this field is used, an invariant the VM imposes is that the difference
///                  must be under `MAX_PERMITTED_DATA_INCREASE`.
///
///                  This field is ignored by the VM upon program termination.
///
///      [ 8..39]  32 bytes: Address of the account.
///                  This field is ignored by the VM upon program termination.
///
///      [40..71]  32 bytes: Address of the owner account.
///                  This field is read by the VM upon program termination.
///
///      [72..79]  8 bytes: Account balance in lamports.
///                  This field is read by the VM upon program termination.
///
///      [80..87]  8 bytes: Account data length.  This value stores the absolute size of the account
///                  data.
///                  (This can probably be 4 bytes, as accounts have a maximum size of 10MBs).
///                  Needs to be kept in sync with the resize delta.
///
///                  This field is read by the VM upon program termination.
///
///                  Data length increase must not exceed `MAX_PERMITTED_DATA_INCREASE`, and the
///                  final length must be not exceed `MAX_PERMITTED_DATA_LENGTH`.
///
///      [88.. a]  data bytes: Exactly as many bytes as the value of the "Account data length"
///                  field.  Stored account data.
///
///      [a+1..b]  padding for growth: 10,240 bytes preallocated in case the account data is
///                  increased.  10k is the maximum data increase for a given account within a
///                  single instruction.  Aka `MAX_PERMITTED_DATA_INCREASE`.
///                  (Why do we allocate this for read-only accounts?)
///
///      [b+1..b+8]  rent epoch: This field is always set to u64::MAX, as accounts are not allowed
///                  to be non-rent exempt any more.
///
///                  This field is ignored by the VM upon program termination.
///
///      [b+8..c]  padding of up to 7 zero bytes: aligns the next account on the 8 byte boundary.
///                  (For some reason in the `serialization.rs` code this is called
///                  `BPF_ALIGN_OF_U128`, which is somewhat confusing.  I would expect the `u128`
///                  alignment to be 16 bytes, as it is in C and Rust.)
///
///                  This field is ignored by the VM upon program termination.
/// ```
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
#[repr(C)]
pub struct AbiAccountHeader {
    pub duplicate_reference: u8,
    pub is_signer: u8,
    pub is_writable: u8,
    pub is_executable: u8,
    pub resize_delta: [u8; size_of::<i32>()],
    pub address: Pubkey,
    pub owner: Pubkey,
    pub lamports: [u8; size_of::<u64>()],
    pub data_len: [u8; size_of::<u64>()],
}

impl AbiAccountHeader {
    #[inline(always)]
    pub fn resize_delta(&self) -> i32 {
        i32::from_le_bytes(self.resize_delta)
    }

    #[inline(always)]
    pub fn set_resize_delta(mut self: Pin<&mut Self>, value: i32) {
        self.resize_delta = value.to_le_bytes();
    }

    #[inline(always)]
    pub fn lamports(&self) -> u64 {
        u64::from_le_bytes(self.lamports)
    }

    #[inline(always)]
    pub fn set_lamports(mut self: Pin<&mut Self>, value: u64) {
        self.lamports = value.to_le_bytes();
    }

    #[inline(always)]
    pub fn data_len(&self) -> u64 {
        u64::from_le_bytes(self.data_len)
    }

    #[inline(always)]
    pub fn set_data_len(mut self: Pin<&mut Self>, value: u64) {
        self.data_len = value.to_le_bytes();
    }

    #[inline(always)]
    pub fn data_ptr(&self) -> *const u8 {
        // SAFETY: VM ABI guarantees that data follows the account header.
        unsafe { (self as *const _ as *const u8).add(size_of::<Self>()) }
    }

    #[inline(always)]
    pub fn data_ptr_mut(self: Pin<&mut Self>) -> *mut u8 {
        // SAFETY: VM ABI guarantees that data follows the account header.
        //
        // `get_unchecked_mut()` call is safe, as the is further projected, and the returned pointer
        // points to the data that is not subject to the `Pin` restriction.
        unsafe { (self.get_unchecked_mut() as *mut _ as *mut u8).add(size_of::<Self>()) }
    }

    #[inline(always)]
    pub fn data_slice(&self) -> &[u8] {
        assert_eq_size!(usize, u64);
        let data_len = self.data_len() as usize;
        // SAFETY: Valid account will contain as many bytes as the header specifies.
        unsafe { slice::from_raw_parts(self.data_ptr(), data_len) }
    }

    #[inline(always)]
    pub fn data_slice_mut(self: Pin<&mut Self>) -> &mut [u8] {
        assert_eq_size!(usize, u64);
        let data_len = self.data_len() as usize;
        // SAFETY: Valid account will contain as many bytes as the header specifies.
        unsafe { slice::from_raw_parts_mut(self.data_ptr_mut(), data_len) }
    }

    pub fn log(&self) {
        log::sol_log("duplicate_reference, is_signer, is_writable, is_executable, resize_delta");
        log::sol_log_64(
            self.duplicate_reference as u64,
            self.is_signer as u64,
            self.is_writable as u64,
            self.is_executable as u64,
            self.resize_delta() as u64,
        );
        log::sol_log("address");
        self.address.log();
        log::sol_log("owner");
        self.owner.log();
        log::sol_log("lamports, data_len");
        log::sol_log_64(self.lamports(), self.data_len(), 0, 0, 0);
    }
}

#[cfg(test)]
mod tests {
    use core::mem::{offset_of, size_of};

    use super::AbiAccountHeader;

    #[test]
    fn abi_account_header_field_offsets() {
        assert_eq!(0, offset_of!(AbiAccountHeader, duplicate_reference));
        assert_eq!(1, offset_of!(AbiAccountHeader, is_signer));
        assert_eq!(2, offset_of!(AbiAccountHeader, is_writable));
        assert_eq!(3, offset_of!(AbiAccountHeader, is_executable));
        assert_eq!(4, offset_of!(AbiAccountHeader, resize_delta));
        assert_eq!(8, offset_of!(AbiAccountHeader, address));
        assert_eq!(40, offset_of!(AbiAccountHeader, owner));
        assert_eq!(72, offset_of!(AbiAccountHeader, lamports));
        assert_eq!(80, offset_of!(AbiAccountHeader, data_len));
        assert_eq!(88, size_of::<AbiAccountHeader>());
    }
}
