use pinocchio_macro::entrypoint;

entrypoint! {
    instruction_type: OneInstruction,
    instruction_type: OtherInstruction,
    program_id: program_id;

    Instruction::NewUser => |
        metadata: Pin<&mut Account<T>>;
        name: &str,
    | {
        new_user(metadata, name)
    };
}

fn main() {
    println!("OK");
}
