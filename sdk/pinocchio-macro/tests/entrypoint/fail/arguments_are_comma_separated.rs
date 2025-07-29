use pinocchio_macro::entrypoint;

entrypoint! {
    instruction_type: Instruction;
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
