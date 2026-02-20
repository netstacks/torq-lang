use torqc_ast::ast::*;

use crate::diagnostic::Diagnostic;

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

/// Walk every block in `program` and emit diagnostics for:
///
/// 1. **Break validation** — `break` may only appear inside a `loop` body.
///    Using `break` at the top level, inside `each`, or in a match arm (unless
///    nested inside a loop) is an error.
///
/// 2. **Shared state safety** — assigning to a shared (`*`) variable inside a
///    parallel `each` (where `sequential == false`) produces a warning
///    suggesting that the `each` be made sequential.
pub fn check(program: &Program) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();

    for block in &program.blocks {
        check_statements(&block.body, false, false, &mut diagnostics);
    }

    diagnostics
}

// ---------------------------------------------------------------------------
// Statement walker
// ---------------------------------------------------------------------------

fn check_statements(
    stmts: &[Statement],
    in_loop: bool,
    in_parallel_each: bool,
    diags: &mut Vec<Diagnostic>,
) {
    for stmt in stmts {
        check_statement(stmt, in_loop, in_parallel_each, diags);
    }
}

fn check_statement(
    stmt: &Statement,
    in_loop: bool,
    in_parallel_each: bool,
    diags: &mut Vec<Diagnostic>,
) {
    match stmt {
        Statement::Loop(lp) => {
            if let Some(cond) = &lp.condition {
                check_expr(cond, in_loop, in_parallel_each, diags);
            }
            // Inside the loop body, `in_loop` becomes true.
            check_statements(&lp.body, true, in_parallel_each, diags);
        }
        Statement::Each(each) => {
            check_expr(&each.iterable, in_loop, in_parallel_each, diags);
            // `each` does NOT set `in_loop` — break is only valid inside loop.
            // `in_parallel_each` is set when the each is not sequential.
            let parallel = !each.sequential;
            check_statements(&each.body, in_loop, parallel, diags);
        }
        Statement::Assignment(assign) => {
            // Check the RHS expression.
            check_expr(&assign.value, in_loop, in_parallel_each, diags);
            // Check: shared variable assigned inside parallel each.
            if in_parallel_each && assign.target.sigil == Sigil::Shared {
                diags.push(
                    Diagnostic::warning(
                        format!(
                            "shared variable *{} modified inside parallel each",
                            assign.target.name,
                        ),
                        assign.span.clone(),
                    )
                    .with_hint("consider adding 'sequential' to the each statement"),
                );
            }
        }
        Statement::Pipeline(pipeline) => {
            for stage in &pipeline.stages {
                check_expr(stage, in_loop, in_parallel_each, diags);
            }
        }
        Statement::Expression(expr) => {
            check_expr(expr, in_loop, in_parallel_each, diags);
        }
    }
}

// ---------------------------------------------------------------------------
// Expression walker
// ---------------------------------------------------------------------------

#[allow(clippy::only_used_in_recursion)]
fn check_expr(expr: &Expr, in_loop: bool, in_parallel_each: bool, diags: &mut Vec<Diagnostic>) {
    match expr {
        Expr::Break(span) => {
            if !in_loop {
                diags.push(Diagnostic::error(
                    "break used outside of a loop",
                    span.clone(),
                ));
            }
        }
        Expr::BinOp(bin) => {
            check_expr(&bin.left, in_loop, in_parallel_each, diags);
            check_expr(&bin.right, in_loop, in_parallel_each, diags);
        }
        Expr::Pipeline(pipeline) => {
            for stage in &pipeline.stages {
                check_expr(stage, in_loop, in_parallel_each, diags);
            }
        }
        Expr::BlockCall(bc) => {
            for arg in &bc.args {
                check_expr(arg, in_loop, in_parallel_each, diags);
            }
        }
        Expr::Call(call) => {
            for arg in &call.args {
                check_expr(arg, in_loop, in_parallel_each, diags);
            }
        }
        Expr::Array(elems, _) => {
            for elem in elems {
                check_expr(elem, in_loop, in_parallel_each, diags);
            }
        }
        Expr::Dict(entries, _) => {
            for entry in entries {
                check_expr(&entry.value, in_loop, in_parallel_each, diags);
            }
        }
        Expr::MemberAccess(ma) => {
            check_expr(&ma.object, in_loop, in_parallel_each, diags);
        }
        Expr::Ternary(tern) => {
            check_expr(&tern.condition, in_loop, in_parallel_each, diags);
            check_expr(&tern.then_expr, in_loop, in_parallel_each, diags);
            check_expr(&tern.else_expr, in_loop, in_parallel_each, diags);
        }
        Expr::Group(inner, _) => {
            check_expr(inner, in_loop, in_parallel_each, diags);
        }
        Expr::Match(m) => {
            for arm in &m.arms {
                check_expr(&arm.body, in_loop, in_parallel_each, diags);
            }
        }
        // Terminal expressions — nothing to recurse into.
        Expr::Literal(_)
        | Expr::Variable(_)
        | Expr::BlockRef(_, _)
        | Expr::StringInterp(_, _) => {}
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    /// Helper: create a span for testing.
    fn span(line: usize, col: usize) -> Span {
        Span {
            file: "test.torq".to_string(),
            line,
            col,
        }
    }

    /// Helper: create a block with the given name and body (no params).
    fn make_block(name: &str, body: Vec<Statement>) -> Block {
        Block {
            name: name.to_string(),
            params: vec![],
            body,
            doc_comments: vec![],
            span: span(1, 1),
        }
    }

    /// Helper: create an iterable expression for `each`.
    fn dummy_iterable() -> Box<Expr> {
        Box::new(Expr::Variable(Variable {
            sigil: Sigil::Array,
            name: "items".to_string(),
            span: span(1, 1),
        }))
    }

    /// Helper: create a binding variable for `each`.
    fn each_binding() -> Variable {
        Variable {
            sigil: Sigil::Scalar,
            name: "item".to_string(),
            span: span(1, 10),
        }
    }

    // -- Break validation tests ----------------------------------------------

    #[test]
    fn break_inside_loop_ok() {
        // break inside a loop body — no error.
        let program = Program {
            blocks: vec![make_block(
                "main",
                vec![Statement::Loop(Loop {
                    condition: None,
                    body: vec![Statement::Expression(Expr::Break(span(3, 5)))],
                    span: span(2, 1),
                })],
            )],
        };

        let diags = check(&program);
        assert!(diags.is_empty(), "expected no diagnostics, got: {:?}", diags);
    }

    #[test]
    fn break_outside_loop_error() {
        // break at block level — error.
        let program = Program {
            blocks: vec![make_block(
                "main",
                vec![Statement::Expression(Expr::Break(span(2, 5)))],
            )],
        };

        let diags = check(&program);
        assert_eq!(diags.len(), 1);
        assert!(diags[0].is_error());
        assert_eq!(diags[0].message, "break used outside of a loop");
    }

    #[test]
    fn break_in_each_error() {
        // break inside each (not loop) — error.
        let program = Program {
            blocks: vec![make_block(
                "main",
                vec![Statement::Each(Each {
                    iterable: dummy_iterable(),
                    binding: each_binding(),
                    sequential: true,
                    body: vec![Statement::Expression(Expr::Break(span(3, 5)))],
                    span: span(2, 1),
                })],
            )],
        };

        let diags = check(&program);
        assert_eq!(diags.len(), 1);
        assert!(diags[0].is_error());
        assert_eq!(diags[0].message, "break used outside of a loop");
    }

    #[test]
    fn break_in_loop_inside_each_ok() {
        // break inside a loop that is nested inside an each — OK.
        let program = Program {
            blocks: vec![make_block(
                "main",
                vec![Statement::Each(Each {
                    iterable: dummy_iterable(),
                    binding: each_binding(),
                    sequential: false,
                    body: vec![Statement::Loop(Loop {
                        condition: None,
                        body: vec![Statement::Expression(Expr::Break(span(4, 9)))],
                        span: span(3, 5),
                    })],
                    span: span(2, 1),
                })],
            )],
        };

        let diags = check(&program);
        assert!(diags.is_empty(), "expected no diagnostics, got: {:?}", diags);
    }

    #[test]
    fn break_in_match_arm_outside_loop_error() {
        // break inside a match arm that is NOT inside a loop — error.
        let program = Program {
            blocks: vec![make_block(
                "main",
                vec![Statement::Expression(Expr::Match(Match {
                    arms: vec![MatchArm {
                        pattern: Pattern::Wildcard,
                        body: Box::new(Expr::Break(span(4, 9))),
                        span: span(3, 5),
                    }],
                    span: span(2, 1),
                }))],
            )],
        };

        let diags = check(&program);
        assert_eq!(diags.len(), 1);
        assert!(diags[0].is_error());
        assert_eq!(diags[0].message, "break used outside of a loop");
    }

    // -- Shared state safety tests -------------------------------------------

    #[test]
    fn shared_var_in_parallel_each_warns() {
        // *counter assigned in parallel each — warning (not error).
        let program = Program {
            blocks: vec![make_block(
                "main",
                vec![Statement::Each(Each {
                    iterable: dummy_iterable(),
                    binding: each_binding(),
                    sequential: false,
                    body: vec![Statement::Assignment(Assignment {
                        target: Variable {
                            sigil: Sigil::Shared,
                            name: "counter".to_string(),
                            span: span(3, 5),
                        },
                        value: Box::new(Expr::Literal(Literal::Int(1, span(3, 18)))),
                        span: span(3, 5),
                    })],
                    span: span(2, 1),
                })],
            )],
        };

        let diags = check(&program);
        assert_eq!(diags.len(), 1);
        assert!(!diags[0].is_error(), "expected warning, got error");
        assert_eq!(
            diags[0].message,
            "shared variable *counter modified inside parallel each"
        );
        assert_eq!(
            diags[0].hint.as_deref(),
            Some("consider adding 'sequential' to the each statement")
        );
    }

    #[test]
    fn shared_var_in_sequential_each_ok() {
        // *counter assigned in sequential each — no diagnostic.
        let program = Program {
            blocks: vec![make_block(
                "main",
                vec![Statement::Each(Each {
                    iterable: dummy_iterable(),
                    binding: each_binding(),
                    sequential: true,
                    body: vec![Statement::Assignment(Assignment {
                        target: Variable {
                            sigil: Sigil::Shared,
                            name: "counter".to_string(),
                            span: span(3, 5),
                        },
                        value: Box::new(Expr::Literal(Literal::Int(1, span(3, 18)))),
                        span: span(3, 5),
                    })],
                    span: span(2, 1),
                })],
            )],
        };

        let diags = check(&program);
        assert!(diags.is_empty(), "expected no diagnostics, got: {:?}", diags);
    }

    #[test]
    fn scalar_var_in_parallel_each_ok() {
        // $x (non-shared) assigned in parallel each — no warning.
        let program = Program {
            blocks: vec![make_block(
                "main",
                vec![Statement::Each(Each {
                    iterable: dummy_iterable(),
                    binding: each_binding(),
                    sequential: false,
                    body: vec![Statement::Assignment(Assignment {
                        target: Variable {
                            sigil: Sigil::Scalar,
                            name: "x".to_string(),
                            span: span(3, 5),
                        },
                        value: Box::new(Expr::Literal(Literal::Int(1, span(3, 10)))),
                        span: span(3, 5),
                    })],
                    span: span(2, 1),
                })],
            )],
        };

        let diags = check(&program);
        assert!(diags.is_empty(), "expected no diagnostics, got: {:?}", diags);
    }
}
