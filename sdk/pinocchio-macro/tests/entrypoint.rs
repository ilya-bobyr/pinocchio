use paste::paste;
use trybuild;

macro_rules! pass_test {
    ($name:tt) => {
        paste! {
            #[test]
            fn [< pass_ $name >] () {
                let t = trybuild::TestCases::new();
                t.pass(concat!("tests/entrypoint/pass/", stringify!($name), ".rs"));
            }
        }
    };
}

macro_rules! fail_test {
    ($name:tt) => {
        paste! {
            #[test]
            fn [< fail_ $name >] () {
                let t = trybuild::TestCases::new();
                t.compile_fail(concat!("tests/entrypoint/fail/", stringify!($name), ".rs"));
            }
        }
    };
}

pass_test!(valid_syntax);

/* Parsing */

fail_test!(arguments_are_comma_separated);
fail_test!(duplicate_argument_name);
fail_test!(implied_fn_requires_non_empty_path);
fail_test!(implied_fn_wrong_return_type);
fail_test!(invalid_argument_name);
