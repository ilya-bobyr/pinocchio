//! Tools that help with testing.

use syn::Error;

#[track_caller]
pub(crate) fn assert_macro_error<T>(
    expected_failure_reason: &str,
    actual: Result<T, Error>,
    expected: &str,
) {
    assert_macro_error(expected_failure_reason, actual.err(), expected);

    #[track_caller]
    fn assert_macro_error(expected_failure_reason: &str, actual: Option<Error>, expected: &str) {
        let actual_error = actual.expect(expected_failure_reason);
        let error_message = actual_error.into_compile_error().to_string();
        assert_eq!(error_message, expected);
    }
}
