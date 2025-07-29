//! This macro parses a convenient DSL that defines processing of a Solana program arguments and
//! instruction data provided to an entrypoint.
//!
//! A program needs to specify how a specific instruction is selected.  Normally, the first byte or
//! a few bytes of the instruction data encodes the instruction index.  A program can use a type
//! (preferred) to describe the list of instructions it supports, or it can use of of the integer
//! types, such as `u8` and `u16`.
//!
//! Depending on the selected instruction the argument list is parsed into a list of `&Account`,
//! `Pin<&mut Account>`, and/or `&SharedAccount` values.  The rest of the instruction data can also
//! go thought an instruction specific parsing process, to remove some of the boilerplate.
//!
//! `Pin` is necessary because `Account` actually only represents a header, and the account data
//! follows the header.  It is thus incorrect to allow the header to be moved.  `Pin` expresses this
//! points exactly: it prevents the enclosed type value from being moved, yet allowing modification.
//! This only matter for the mutable case.
//!
//! This crate is implemented following structure suggested in "Structuring, testing and debugging
//! procedural macro crates".  See:
//!
//! https://ferrous-systems.com/blog/testing-proc-macros/
//!
//! As the macro is relatively simple, I've removed the "Model -> Ir" step, and `codegen()` is
//! directly invoked on the `Model`.
//!
//! # Examples
//!
//! Here is an example of how a program entry point can be described:
//!
//! TODO Document macro properly.  In particular see
//!
//! https://doc.rust-lang.org/rustdoc/write-documentation/documentation-tests.html#documenting-macros
//!
//! for instructions on how to import macros into example code.  And this example needs to be
//! finished in order for it to compile and be executable.
//!
//!  ```rust,ignore
//!  #[derive(PartialEq, Eq, ProgramDataParser)]
//!  #[repr(u8)]
//!  enum Instruction {
//!      Setup = 1,
//!      CreateUser = 2,
//!      SetUserName = 3,
//!      StakeTo = 4,
//!      RecordAction = 5,
//!      ComplexOp = 6,
//!  }
//!
//!  #[derive(Debug, AccountContent)]
//!  struct Metadata {
//!      admin: Pubkey,
//!  }
//!
//!  #[derive(Debug, AccountContent)]
//!  struct User {
//!      // See `../docs/strings.md` for some details on strings.
//!      name: FStr<20>,
//!  }
//!
//!  impl User {
//!      fn set_name(&mut self, name: &str) -> Result<(), ProgramError> {
//!          self.name.assign(name)
//!      }
//!  }
//!
//!  entrypoint! {
//!      instruction_type: Instruction;
//!      program_id: program_id;
//!
//!      Instruction::Setup => |metadata: NewAccountMut<Metadata>; admin: Pubkey| {
//!          metadata.initialize(Metadata::Init { admin })
//!      };
//!      Instruction::CreateUser => |
//!          metadata: &OwnAccount<Metadata>,
//!          new_user: Pin<&mut NewAccount<User>>,
//!          ;
//!          name: &str,
//!      | {
//!          let metadata: &Metadata = metadata
//!              .require_initialized_and_owner(&program_id)?
//!              .content();
//!          new_user.initialize((metadata, name))
//!      };
//!      Instruction::SetUserName => |user: Pin<&mut OwnAccount<User>>; name: &str| {
//!          let user: &mut User = user
//!              .require_initialized_and_owner(&program_id)?
//!              .content_mut();
//!          user.set_name(name)
//!      };
//!      Instruction::StakeTo => |
//!          metadata: &OwnAccount<Metadata>,
//!          staker: OwnAccountMut<Staker>,
//!          target_user: &AnyAccount<User>,
//!          ;
//!          amount: u64,
//!      | {
//!          let metadata: &Metadata = metadata
//!              .require_initialized_and_owner(&program_id)?
//!              .content();
//!          let staker = staker
//!              .require_initialized_and_owner(&program_id)?
//!              .require_signer()?;
//!          stake_to(metadata, staker, target_user, amount)
//!      };
//!      Instruction::RecordAction => |
//!          metadata: &OwnAccount<Metadata>,
//!          log: OwnAccountMut<Log>,
//!          ;
//!          initiated_by: Pubkey,
//!          target_user: Pubkey,
//!          description: &str,
//!      | {
//!          let metadata: &Metadata = metadata
//!              .require_initialized_and_owner(&program_id)?
//!              .content();
//!          let log: &mut Log = log
//!              .require_initialized_and_owner(&program_id)
//!              .content_mut();
//!          record_action(metadata, log, initiated_by, target_user, description)
//!      };
//!      Instruction::ComplexOp => |
//!          participant1: &SharedAccount<User>,
//!          participant2: &SharedAccount<User>,
//!          ..
//!          ;
//!          args: &[u8],
//!      | {
//!          complex_op(participant1, participant2, args)
//!      };
//!  }
//!  ```

use proc_macro::TokenStream;
use proc_macro_error::abort;
use proc_macro_error::proc_macro_error;

mod classify;
mod entrypoint;
#[cfg(test)]
mod test_utils;

use crate::entrypoint::analyze;
use crate::entrypoint::codegen;
use crate::entrypoint::parse;

#[proc_macro_error]
#[proc_macro]
pub fn entrypoint(ts: TokenStream) -> TokenStream {
    let ast = match parse::parse(ts.clone().into()) {
        Ok(ast) => ast,
        Err(e) => {
            abort!(e.span(), e)
        }
    };
    let model = match analyze::analyze(ast) {
        Ok(model) => model,
        Err(e) => {
            abort!(e.span(), e)
        }
    };
    codegen::codegen(model).into()
}
