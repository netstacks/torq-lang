use torqc_ast::ast::*;

use crate::diagnostic::Diagnostic;
use crate::registry::BlockRegistry;

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

/// Walk every block body in `program` and check each `BlockCall` whose
/// argument count does not match the parameter count declared in the
/// corresponding block definition.
///
/// Too many arguments is an **error**. Too few arguments is a **warning**
/// because TORQ's syntax allows member-access chains (e.g. `%dict.field`)
/// to absorb trailing arguments during parsing, making static arity
/// analysis unreliable until the parser is improved.
///
/// Calls to blocks that are **not** present in `registry` are silently skipped
/// — the `resolve_blocks` check is responsible for reporting those.
pub fn check(program: &Program, registry: &BlockRegistry) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();

    for block in &program.blocks {
        for stmt in &block.body {
            check_statement(stmt, registry, &mut diagnostics);
        }
    }

    diagnostics
}

// ---------------------------------------------------------------------------
// Statement walker
// ---------------------------------------------------------------------------

fn check_statement(stmt: &Statement, reg: &BlockRegistry, diags: &mut Vec<Diagnostic>) {
    match stmt {
        Statement::Pipeline(pipeline) => check_pipeline(pipeline, reg, diags),
        Statement::Assignment(assign) => check_expr(&assign.value, reg, diags),
        Statement::Each(each) => {
            check_expr(&each.iterable, reg, diags);
            for stmt in &each.body {
                check_statement(stmt, reg, diags);
            }
        }
        Statement::Loop(lp) => {
            if let Some(cond) = &lp.condition {
                check_expr(cond, reg, diags);
            }
            for stmt in &lp.body {
                check_statement(stmt, reg, diags);
            }
        }
        Statement::Expression(expr) => check_expr(expr, reg, diags),
    }
}

// ---------------------------------------------------------------------------
// Expression walker
// ---------------------------------------------------------------------------

fn check_pipeline(pipeline: &Pipeline, reg: &BlockRegistry, diags: &mut Vec<Diagnostic>) {
    for stage in &pipeline.stages {
        check_expr(stage, reg, diags);
    }
}

fn check_expr(expr: &Expr, reg: &BlockRegistry, diags: &mut Vec<Diagnostic>) {
    match expr {
        Expr::BlockCall(bc) => {
            // Only check arity for blocks that exist in the registry.
            // Undefined blocks are reported by resolve_blocks.
            if let Some(info) = reg.get(&bc.name) {
                let expected = info.param_count;
                let actual = bc.args.len();
                if actual > expected {
                    // Too many args is always an error.
                    diags.push(Diagnostic::error(
                        format!(
                            "block ::{} expects {} argument(s), but {} provided",
                            bc.name, expected, actual,
                        ),
                        bc.span.clone(),
                    ));
                } else if actual < expected {
                    // Too few args may be caused by the parser absorbing
                    // trailing arguments into member-access chains, so
                    // report as a warning rather than an error.
                    diags.push(Diagnostic::warning(
                        format!(
                            "block ::{} expects {} argument(s), but {} provided",
                            bc.name, expected, actual,
                        ),
                        bc.span.clone(),
                    ));
                }
            }
            // Recurse into arguments
            for arg in &bc.args {
                check_expr(arg, reg, diags);
            }
        }
        Expr::Pipeline(pipeline) => check_pipeline(pipeline, reg, diags),
        Expr::BinOp(bin) => {
            check_expr(&bin.left, reg, diags);
            check_expr(&bin.right, reg, diags);
        }
        Expr::Match(m) => {
            for arm in &m.arms {
                check_expr(&arm.body, reg, diags);
            }
        }
        Expr::Call(call) => {
            for arg in &call.args {
                check_expr(arg, reg, diags);
            }
        }
        Expr::Array(elems, _) => {
            for elem in elems {
                check_expr(elem, reg, diags);
            }
        }
        Expr::Dict(entries, _) => {
            for entry in entries {
                check_expr(&entry.value, reg, diags);
            }
        }
        Expr::MemberAccess(ma) => {
            check_expr(&ma.object, reg, diags);
        }
        Expr::Ternary(tern) => {
            check_expr(&tern.condition, reg, diags);
            check_expr(&tern.then_expr, reg, diags);
            check_expr(&tern.else_expr, reg, diags);
        }
        Expr::Group(inner, _) => {
            check_expr(inner, reg, diags);
        }
        // Terminal expressions — nothing to recurse into
        Expr::Literal(_)
        | Expr::Variable(_)
        | Expr::BlockRef(_, _)
        | Expr::StringInterp(_, _)
        | Expr::Break(_) => {}
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

    /// Helper: create a block with the given name, number of params, and body.
    fn make_block(name: &str, param_count: usize, body: Vec<Statement>) -> Block {
        let params = (0..param_count)
            .map(|i| Param {
                sigil: Sigil::Scalar,
                name: format!("p{}", i),
                span: span(1, 1),
            })
            .collect();

        Block {
            name: name.to_string(),
            params,
            body,
            doc_comments: vec![],
            span: span(1, 1),
        }
    }

    /// Helper: create a literal expression for use as an argument.
    fn int_arg(val: i64) -> Expr {
        Expr::Literal(Literal::Int(val, span(1, 1)))
    }

    #[test]
    fn correct_arity() {
        // Block `add` expects 2 params, called with 2 args — no error.
        let program = Program {
            blocks: vec![
                make_block(
                    "main",
                    0,
                    vec![Statement::Expression(Expr::BlockCall(BlockCall {
                        name: "add".to_string(),
                        args: vec![int_arg(1), int_arg(2)],
                        span: span(2, 5),
                    }))],
                ),
                make_block("add", 2, vec![]),
            ],
        };

        let (registry, build_diags) = BlockRegistry::build(&program);
        assert!(build_diags.is_empty());

        let diags = check(&program, &registry);
        assert!(diags.is_empty(), "expected no diagnostics, got: {:?}", diags);
    }

    #[test]
    fn too_few_args() {
        // Block `add` expects 2 params, called with 1 arg — warning (not error).
        let program = Program {
            blocks: vec![
                make_block(
                    "main",
                    0,
                    vec![Statement::Expression(Expr::BlockCall(BlockCall {
                        name: "add".to_string(),
                        args: vec![int_arg(1)],
                        span: span(3, 5),
                    }))],
                ),
                make_block("add", 2, vec![]),
            ],
        };

        let (registry, _) = BlockRegistry::build(&program);
        let diags = check(&program, &registry);

        assert_eq!(diags.len(), 1);
        assert!(!diags[0].is_error(), "too-few-args should be a warning, not error");
        assert_eq!(
            diags[0].message,
            "block ::add expects 2 argument(s), but 1 provided"
        );
    }

    #[test]
    fn too_many_args() {
        // Block `noop` expects 0 params, called with 1 arg — error.
        let program = Program {
            blocks: vec![
                make_block(
                    "main",
                    0,
                    vec![Statement::Expression(Expr::BlockCall(BlockCall {
                        name: "noop".to_string(),
                        args: vec![int_arg(42)],
                        span: span(4, 5),
                    }))],
                ),
                make_block("noop", 0, vec![]),
            ],
        };

        let (registry, _) = BlockRegistry::build(&program);
        let diags = check(&program, &registry);

        assert_eq!(diags.len(), 1);
        assert!(diags[0].is_error());
        assert_eq!(
            diags[0].message,
            "block ::noop expects 0 argument(s), but 1 provided"
        );
    }

    #[test]
    fn undefined_block_skipped() {
        // Call to an undefined block should produce no arity error.
        let program = Program {
            blocks: vec![make_block(
                "main",
                0,
                vec![Statement::Expression(Expr::BlockCall(BlockCall {
                    name: "ghost".to_string(),
                    args: vec![int_arg(1), int_arg(2), int_arg(3)],
                    span: span(5, 5),
                }))],
            )],
        };

        let (registry, _) = BlockRegistry::build(&program);
        let diags = check(&program, &registry);

        assert!(
            diags.is_empty(),
            "undefined block should not produce arity error, got: {:?}",
            diags
        );
    }

    #[test]
    fn nested_block_call_in_each() {
        // An arity mismatch (too few) inside an `each` body should still be caught
        // as a warning.
        let program = Program {
            blocks: vec![
                make_block(
                    "main",
                    0,
                    vec![Statement::Each(Each {
                        iterable: Box::new(Expr::Variable(Variable {
                            sigil: Sigil::Array,
                            name: "items".to_string(),
                            span: span(2, 1),
                        })),
                        binding: Variable {
                            sigil: Sigil::Scalar,
                            name: "item".to_string(),
                            span: span(2, 10),
                        },
                        sequential: false,
                        body: vec![Statement::Expression(Expr::BlockCall(BlockCall {
                            name: "process".to_string(),
                            args: vec![],
                            span: span(3, 5),
                        }))],
                        span: span(2, 1),
                    })],
                ),
                make_block("process", 1, vec![]),
            ],
        };

        let (registry, _) = BlockRegistry::build(&program);
        let diags = check(&program, &registry);

        assert_eq!(diags.len(), 1);
        assert!(!diags[0].is_error(), "too-few-args should be a warning");
        assert_eq!(
            diags[0].message,
            "block ::process expects 1 argument(s), but 0 provided"
        );
    }
}
