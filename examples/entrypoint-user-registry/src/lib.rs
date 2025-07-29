//! This is an example of a correct program.  Mostly matching an example from doc-comment for teh
//! `entrypoint!` macro.
//!
//! Things that are not implemented yet are marked with `//-`.

use core::mem::MaybeUninit;
use pinocchio::{
    account::{
        account_content_cast_bytes, account_content_cast_bytes_mut,
        initialize_bytes_with_sized_self, Account, AccountContent, AnyAccount, Initializable,
        NewAccountMut, OwnAccount, OwnAccountMut,
    },
    entrypoint_v2::ProgramDataParser,
    program_error::ProgramError,
    pubkey::Pubkey,
};
use pinocchio_macro::entrypoint;

//- #[derive(PartialEq, Eq, ProgramDataParser)]
#[derive(PartialEq, Eq)]
#[repr(u8)]
enum Instruction {
    Setup = 1,
    CreateUser = 2,
    SetUserName = 3,
    StakeTo = 4,
    RecordAction = 5,
    ComplexOp = 6,
}

//- #[derive(Debug, AccountContent, Initializable)]
#[derive(Debug)]
struct Metadata {
    admin: Pubkey,
}

//- #[derive(Debug, AccountContent, Initializable)]
#[derive(Debug)]
#[repr(C)]
struct User {
    admin: Pubkey,
    // See `../docs/strings.md` for some details on strings.
    //- name: FStr<20>,
}

impl User {
    fn set_name(&mut self, _name: &str) -> Result<(), ProgramError> {
        //- self.name.assign(name)
        todo!()
    }
}

//- #[derive(Debug, AccountContent, Initializable)]
#[derive(Debug)]
#[repr(C)]
struct Staker {
    owner: Pubkey,
}

//- #[derive(Debug, AccountContent, Initializable)]
#[derive(Debug)]
struct Log {
    number_of_entries: [u8; size_of::<u32>()],
    // Entries follow, each an instance of `LogEntry`.
    // TODO Define `LogEntry` and write an example implementation for interacting with log entries.
}

#[derive(Debug)]
#[repr(C)]
struct LogEntry {
    initiated_by: Pubkey,
    target_user: Pubkey,
    // See `../docs/strings.md` for some details on strings.
    //- description: FStr<20>,
    description: [u8; 20],
}

entrypoint! {
    instruction_type: Instruction,
    program_id: program_id;

    Instruction::Setup => |metadata: NewAccountMut<Metadata>; admin: &Pubkey|,

    Instruction::CreateUser => |
        metadata: &OwnAccount<Metadata>,
        new_user: NewAccountMut<User>,
        ;
        name: &str,
    |,

    Instruction::SetUserName => |user: OwnAccountMut<User>; name: &str|,

    Instruction::StakeTo => |
        metadata: &OwnAccount<Metadata>,
        staker: OwnAccountMut<Staker>,
        target_user: &AnyAccount,
        ;
        amount: u64,
    |,

    Instruction::RecordAction => |
        metadata: &OwnAccount<Metadata>,
        log: OwnAccountMut<Log>,
        ;
        initiated_by: &Pubkey,
        target_user: &Pubkey,
        description: &str,
    |,

    Instruction::ComplexOp => |
        participant1: &AnyAccount /* TODO: &SharedAccount<User> */,
        participant2: &AnyAccount /* TODO: &SharedAccount<User> */,
        ;
        args: &[u8],
    |,
}

fn setup(
    _program_id: &Pubkey,
    metadata: NewAccountMut<Metadata>,
    admin: &Pubkey,
) -> Result<(), ProgramError> {
    let _ = metadata.initialize(Metadata { admin: *admin })?;
    Ok(())
}

fn create_user(
    program_id: &Pubkey,
    metadata: &OwnAccount<Metadata>,
    new_user: NewAccountMut<User>,
    name: &str,
) -> Result<(), ProgramError> {
    let metadata: &Account<Metadata> = metadata.require_owner(program_id)?;
    let metadata: &Metadata = metadata.content();

    let user_account = new_user.initialize(User {
        admin: metadata.admin,
    })?;
    // TODO It would be better to provide `name` as part of initialization arguments.
    user_account.content_mut().set_name(name)?;

    Ok(())
}

fn set_user_name(
    program_id: &Pubkey,
    user: OwnAccountMut<User>,
    name: &str,
) -> Result<(), ProgramError> {
    let user: &mut User = user.require_owner_mut(program_id)?.content_mut();
    user.set_name(name)
}

fn stake_to(
    program_id: &Pubkey,
    metadata: &OwnAccount<Metadata>,
    staker: OwnAccountMut<Staker>,
    _target_user: &AnyAccount,
    _amount: u64,
) -> Result<(), ProgramError> {
    let metadata = metadata.require_owner(program_id)?;
    let _metadata: &Metadata = metadata.content();

    let _staker = staker.require_signer_mut()?.require_owner_mut(program_id)?;

    todo!("StakeTo is not implemented yet")
}

fn record_action(
    program_id: &Pubkey,
    metadata: &OwnAccount<Metadata>,
    log: OwnAccountMut<Log>,
    _initiated_by: &Pubkey,
    _target_user: &Pubkey,
    _description: &str,
) -> Result<(), ProgramError> {
    let metadata = metadata.require_owner(program_id)?;
    let _metadata: &Metadata = metadata.content();

    let log = log.require_owner_mut(program_id)?;
    let _log: &mut Log = log.content_mut();

    todo!("RecordAction is not implemented yet")
}

fn complex_op(
    _program_id: &Pubkey,
    // TODO: _participant1: &SharedAccount<User>,
    _participant1: &AnyAccount,
    // TODO: _participant2: &SharedAccount<User>,
    _participant2: &AnyAccount,
    _args: &[u8],
) -> Result<(), ProgramError> {
    todo!("ComplexOp is not implemented yet")
}

// -- Instances below should be autogenerated in the future. --

impl<'data> ProgramDataParser<'data> for Instruction {
    type Error = ProgramError;

    fn parse(data: &'data [u8]) -> Result<(Self, &'data [u8]), Self::Error>
    where
        Self: Sized + 'data,
    {
        let Some((&instruction, data)) = data.split_first() else {
            return Err(ProgramError::InvalidInstructionData);
        };

        let instruction = match instruction {
            1 => Self::Setup,
            2 => Self::CreateUser,
            3 => Self::SetUserName,
            4 => Self::StakeTo,
            5 => Self::RecordAction,
            6 => Self::ComplexOp,
            _ => return Err(ProgramError::InvalidArgument),
        };

        Ok((instruction, data))
    }
}

// SAFETY: All accounts use same length discriminators and they are all unique.
unsafe impl AccountContent for Metadata {
    const DISCRIMINATOR: &'static [u8] = &[1];

    type AsContent<'data> = &'data Self;

    unsafe fn content(data: &[u8]) -> Self::AsContent<'_> {
        // SAFETY: `Metadata` is `#[repr(C)]` and can be constructed from its bytes.
        unsafe { account_content_cast_bytes::<Self>(data) }
    }

    type AsContentMut<'data> = &'data mut Self;

    unsafe fn content_mut(data: &mut [u8]) -> Self::AsContentMut<'_> {
        // SAFETY: `Metadata` is `#[repr(C)]` and can be constructed from its bytes.
        unsafe { account_content_cast_bytes_mut::<Self>(data) }
    }
}

impl Initializable for Metadata {
    type Init<'init> = Metadata;

    #[inline(always)]
    unsafe fn initialize_bytes(
        state: Self,
        data: &mut [MaybeUninit<u8>],
    ) -> Result<(), ProgramError> {
        initialize_bytes_with_sized_self::<Self, _>(
            state,
            data,
            |state, data| Ok(data.write(state)),
        )
    }
}

//- impl Initializable for Metadata {
//-     type Init = Metadata;
//-
//-     #[inline(always)]
//-     unsafe fn initialize<'account>(
//-         state: Self::Init,
//-         data: &'account mut std::mem::MaybeUninit<Self>,
//-     ) -> Result<&'account mut Self, ProgramError>
//-     where
//-         Self: Sized,
//-     {
//-         Ok(data.write(state))
//-     }
//- }

// SAFETY: All accounts use same length discriminators and they are all unique.
unsafe impl AccountContent for User {
    const DISCRIMINATOR: &'static [u8] = &[2];

    type AsContent<'data> = &'data Self;

    unsafe fn content(data: &[u8]) -> Self::AsContent<'_> {
        // SAFETY: `User` is `#[repr(C)]` and can be constructed from its bytes.
        unsafe { account_content_cast_bytes::<Self>(data) }
    }

    type AsContentMut<'data> = &'data mut Self;

    unsafe fn content_mut(data: &mut [u8]) -> Self::AsContentMut<'_> {
        // SAFETY: `User` is `#[repr(C)]` and can be constructed from its bytes.
        unsafe { account_content_cast_bytes_mut::<Self>(data) }
    }
}

impl Initializable for User {
    type Init<'init> = Self;

    #[inline(always)]
    unsafe fn initialize_bytes(
        state: Self,
        data: &mut [MaybeUninit<u8>],
    ) -> Result<(), ProgramError> {
        initialize_bytes_with_sized_self::<Self, _>(
            state,
            data,
            |state, data| Ok(data.write(state)),
        )
    }
}

//- impl Initializable for User {
//-     type Init = Self;
//-
//-     #[inline(always)]
//-     unsafe fn initialize<'account>(
//-         state: Self::Init,
//-         data: &'account mut std::mem::MaybeUninit<Self>,
//-     ) -> Result<&'account mut Self, ProgramError>
//-     where
//-         Self: Sized,
//-     {
//-         Ok(data.write(state))
//-     }
//- }

// SAFETY: All accounts use same length discriminators and they are all unique.
unsafe impl AccountContent for Staker {
    const DISCRIMINATOR: &'static [u8] = &[3];

    type AsContent<'data> = &'data Self;

    unsafe fn content(data: &[u8]) -> Self::AsContent<'_> {
        // SAFETY: `Staker` is `#[repr(C)]` and can be constructed from its bytes.
        unsafe { account_content_cast_bytes::<Self>(data) }
    }

    type AsContentMut<'data> = &'data mut Self;

    unsafe fn content_mut(data: &mut [u8]) -> Self::AsContentMut<'_> {
        // SAFETY: `Staker` is `#[repr(C)]` and can be constructed from its bytes.
        unsafe { account_content_cast_bytes_mut::<Self>(data) }
    }
}

impl Initializable for Staker {
    type Init<'init> = Self;

    #[inline(always)]
    unsafe fn initialize_bytes(
        state: Self,
        data: &mut [MaybeUninit<u8>],
    ) -> Result<(), ProgramError> {
        initialize_bytes_with_sized_self::<Self, _>(
            state,
            data,
            |state, data| Ok(data.write(state)),
        )
    }
}

//- impl Initializable for Staker {
//-     type Init = Self;
//-
//-     #[inline(always)]
//-     unsafe fn initialize<'account>(
//-         state: Self::Init,
//-         data: &'account mut std::mem::MaybeUninit<Self>,
//-     ) -> Result<&'account mut Self, ProgramError>
//-     where
//-         Self: Sized,
//-     {
//-         Ok(data.write(state))
//-     }
//- }

// SAFETY: All accounts use same length discriminators and they are all unique.
unsafe impl AccountContent for Log {
    const DISCRIMINATOR: &'static [u8] = &[4];

    type AsContent<'data> = &'data Self;

    unsafe fn content(data: &[u8]) -> Self::AsContent<'_> {
        // TODO: This conversion is probably suboptimal, as it is missing the log entries part.
        //
        // One obvious solution here is to construct a fat pointer holding a reference to the rest
        // of the account data.  So `AsContent` would be something like `&[LogEntry]` and it will be
        // constructed as `core::slice::from_raw_parts`.
        //
        // TODO I do not fully understand yet what would be a nice API for changing the account
        // size.  It would need to reference the account itself, as it can not be done just via a
        // content reference.
        //
        // SAFETY: `Log` is `#[repr(C)]` and can be constructed from its bytes.
        unsafe { account_content_cast_bytes::<Self>(data) }
    }

    type AsContentMut<'data> = &'data mut Self;

    unsafe fn content_mut(data: &mut [u8]) -> Self::AsContentMut<'_> {
        // SAFETY: `Log` is `#[repr(C)]` and can be constructed from its bytes.
        unsafe { account_content_cast_bytes_mut::<Self>(data) }
    }
}

impl Initializable for Log {
    type Init<'init> = ();

    #[inline(always)]
    unsafe fn initialize_bytes(
        _state: Self::Init<'_>,
        _data: &mut [MaybeUninit<u8>],
    ) -> Result<(), ProgramError> {
        // All bytes in `data` are zero initialized, which works for our `Log` header, indicating an
        // empty log.
        Ok(())
    }
}

//- impl Initializable for Log {
//-     type Init = u32;
//-
//-     #[inline(always)]
//-     unsafe fn initialize<'account>(
//-         _capacity: Self::Init,
//-         data: &'account mut std::mem::MaybeUninit<Self>,
//-     ) -> Result<&'account mut Self, ProgramError>
//-     where
//-         Self: Sized,
//-     {
//-         // TODO `Initializable` need to be modified in order to allow more flexible allocation than
//-         // just `size_of::<Self>()`.  In this case we need to indicate that `size_of::<Log>() +
//-         // capacity * size_of::<LogEntry>()` has been initialized and need to be set as the account
//-         // size.
//-         //
//-         // Yet, we do not want to touch bytes beyond the log header, as those are already property
//-         // zero initialized.
//-         //
//-         // Actually, we do not need to initialize any bytes at all, as all zeros is a valid value
//-         // for the `number_of_entries` field and it is what we initially want.
//-         Ok(data.assume_init_mut())
//-     }
//- }

fn main() {}
