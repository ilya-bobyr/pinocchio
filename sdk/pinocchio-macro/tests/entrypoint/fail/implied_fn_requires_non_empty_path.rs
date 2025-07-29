use pinocchio_macro::entrypoint;

entrypoint! {
    instruction_type: Instruction,
    program_id: program_id;

    17 => |metadata: Pin<&mut Account<T>>; name: &str|,
}

fn main() {
    println!("OK");
}
