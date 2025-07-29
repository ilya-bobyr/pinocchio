//! `syn` has a module `classify` that helps navigate higher level Rust gramma rules.
//! Unfortunately, it is not public.
//!
//! This module is a copy of some of the `syn` functions.

use syn::Expr;

pub(crate) fn requires_comma_to_be_match_arm(expr: &Expr) -> bool {
    match expr {
        // TODO `syn::Expr` suggests this lint, but it is unstable.  Enabled when stabilized.
        // Issue #89554: https://github.com/rust-lang/rust/issues/89554
        // #![cfg_attr(test, deny(non_exhaustive_omitted_patterns))]

        Expr::If(_)
        | Expr::Match(_)
        // both under ExprKind::Block in rustc
        | Expr::Block(_) | Expr::Unsafe(_)
        | Expr::While(_)
        | Expr::Loop(_)
        | Expr::ForLoop(_)
        | Expr::TryBlock(_)
        | Expr::Const(_) => false,

        Expr::Array(_)
        | Expr::Assign(_)
        | Expr::Async(_)
        | Expr::Await(_)
        | Expr::Binary(_)
        | Expr::Break(_)
        | Expr::Call(_)
        | Expr::Cast(_)
        | Expr::Closure(_)
        | Expr::Continue(_)
        | Expr::Field(_)
        | Expr::Group(_)
        | Expr::Index(_)
        | Expr::Infer(_)
        | Expr::Let(_)
        | Expr::Lit(_)
        | Expr::Macro(_)
        | Expr::MethodCall(_)
        | Expr::Paren(_)
        | Expr::Path(_)
        | Expr::Range(_)
        | Expr::RawAddr(_)
        | Expr::Reference(_)
        | Expr::Repeat(_)
        | Expr::Return(_)
        | Expr::Struct(_)
        | Expr::Try(_)
        | Expr::Tuple(_)
        | Expr::Unary(_)
        | Expr::Yield(_)
        | Expr::Verbatim(_) => true,

        // As suggested in the `syn::Expr` implementation, we need to handle the default case, to
        // allow `syn` to add new elements to the `Expr` enum.  Yet, we require exhaustiveness in
        // tests, to make sure we update our code when a new case is indeed added.
        _ => true,
    }
}
