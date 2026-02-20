use torqc_ast::ast::*;

use crate::diagnostic::Diagnostic;
use crate::registry::BlockRegistry;

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

/// Walk every block body in `program` and emit an error for each `BlockCall`
/// or `BlockRef` that refers to a block not present in `registry`.
///
/// When the unknown name is close (edit distance <= 3) to a registered name,
/// a "did you mean ...?" hint is attached.
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
            if !reg.contains(&bc.name) {
                let mut diag = Diagnostic::error(
                    format!("undefined block ::{}", bc.name),
                    bc.span.clone(),
                );
                if let Some(suggestion) = suggest(reg, &bc.name) {
                    diag = diag.with_hint(format!("did you mean ::{}?", suggestion));
                }
                diags.push(diag);
            }
            // Also recurse into arguments
            for arg in &bc.args {
                check_expr(arg, reg, diags);
            }
        }
        Expr::BlockRef(name, span) => {
            if !reg.contains(name) {
                let mut diag = Diagnostic::error(
                    format!("undefined block reference &{}", name),
                    span.clone(),
                );
                if let Some(suggestion) = suggest(reg, name) {
                    diag = diag.with_hint(format!("did you mean &{}?", suggestion));
                }
                diags.push(diag);
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
        | Expr::StringInterp(_, _)
        | Expr::Break(_) => {}
    }
}

// ---------------------------------------------------------------------------
// "Did you mean?" via edit distance
// ---------------------------------------------------------------------------

/// Find the closest block name in the registry within an edit distance of 3.
fn suggest(reg: &BlockRegistry, name: &str) -> Option<String> {
    let mut best: Option<(&str, usize)> = None;

    for candidate in reg.names() {
        let dist = edit_distance(name, candidate);
        if dist <= 3 {
            if let Some((_, best_dist)) = best {
                if dist < best_dist {
                    best = Some((candidate, dist));
                }
            } else {
                best = Some((candidate, dist));
            }
        }
    }

    best.map(|(name, _)| name.to_string())
}

/// Compute the Levenshtein edit distance between two strings.
fn edit_distance(a: &str, b: &str) -> usize {
    let a_len = a.len();
    let b_len = b.len();

    // Quick bail-out: if the lengths differ by more than 3, distance > 3.
    if a_len.abs_diff(b_len) > 3 {
        return a_len.max(b_len);
    }

    let mut prev: Vec<usize> = (0..=b_len).collect();
    let mut curr = vec![0; b_len + 1];

    for (i, ca) in a.chars().enumerate() {
        curr[0] = i + 1;
        for (j, cb) in b.chars().enumerate() {
            let cost = if ca == cb { 0 } else { 1 };
            curr[j + 1] = (prev[j] + cost)
                .min(prev[j + 1] + 1)
                .min(curr[j] + 1);
        }
        std::mem::swap(&mut prev, &mut curr);
    }

    prev[b_len]
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

    /// Helper: create a minimal block with the given name, optional params,
    /// and a body of statements.
    fn make_block(name: &str, body: Vec<Statement>) -> Block {
        Block {
            name: name.to_string(),
            params: vec![],
            body,
            doc_comments: vec![],
            span: span(1, 1),
        }
    }

    // -- edit_distance unit tests -------------------------------------------

    #[test]
    fn edit_distance_identical() {
        assert_eq!(edit_distance("abc", "abc"), 0);
    }

    #[test]
    fn edit_distance_one_sub() {
        assert_eq!(edit_distance("abc", "aXc"), 1);
    }

    #[test]
    fn edit_distance_one_insert() {
        assert_eq!(edit_distance("abc", "abXc"), 1);
    }

    #[test]
    fn edit_distance_one_delete() {
        assert_eq!(edit_distance("abXc", "abc"), 1);
    }

    #[test]
    fn edit_distance_empty() {
        assert_eq!(edit_distance("", "abc"), 3);
        assert_eq!(edit_distance("abc", ""), 3);
    }

    // -- check() integration tests ------------------------------------------

    #[test]
    fn valid_block_call() {
        let program = Program {
            blocks: vec![
                make_block("main", vec![
                    Statement::Expression(Expr::BlockCall(BlockCall {
                        name: "helper".to_string(),
                        args: vec![],
                        span: span(2, 5),
                    })),
                ]),
                make_block("helper", vec![]),
            ],
        };

        let (registry, build_diags) = BlockRegistry::build(&program);
        assert!(build_diags.is_empty());

        let diags = check(&program, &registry);
        assert!(diags.is_empty(), "expected no diagnostics, got: {:?}", diags);
    }

    #[test]
    fn undefined_block_call() {
        let program = Program {
            blocks: vec![
                make_block("main", vec![
                    Statement::Expression(Expr::BlockCall(BlockCall {
                        name: "nonexistent".to_string(),
                        args: vec![],
                        span: span(3, 1),
                    })),
                ]),
            ],
        };

        let (registry, _) = BlockRegistry::build(&program);
        let diags = check(&program, &registry);

        assert_eq!(diags.len(), 1);
        assert!(diags[0].message.contains("undefined block"));
        assert!(diags[0].message.contains("nonexistent"));
        assert!(diags[0].is_error());
    }

    #[test]
    fn did_you_mean_suggestion() {
        let program = Program {
            blocks: vec![
                make_block("main", vec![
                    Statement::Expression(Expr::BlockCall(BlockCall {
                        name: "validate_car".to_string(),
                        args: vec![],
                        span: span(5, 1),
                    })),
                ]),
                make_block("validate_cart", vec![]),
            ],
        };

        let (registry, _) = BlockRegistry::build(&program);
        let diags = check(&program, &registry);

        assert_eq!(diags.len(), 1);
        assert!(diags[0].message.contains("undefined block ::validate_car"));
        let hint = diags[0].hint.as_deref().expect("expected a hint");
        assert!(
            hint.contains("validate_cart"),
            "expected hint to contain 'validate_cart', got: {}",
            hint,
        );
    }

    #[test]
    fn undefined_block_ref() {
        let program = Program {
            blocks: vec![
                make_block("main", vec![
                    Statement::Expression(Expr::BlockRef(
                        "ghost".to_string(),
                        span(4, 1),
                    )),
                ]),
            ],
        };

        let (registry, _) = BlockRegistry::build(&program);
        let diags = check(&program, &registry);

        assert_eq!(diags.len(), 1);
        assert!(diags[0].message.contains("undefined block reference"));
        assert!(diags[0].message.contains("ghost"));
        assert!(diags[0].is_error());
    }

    #[test]
    fn nested_block_call_checked() {
        // A block call inside an `each` body should still be caught.
        let program = Program {
            blocks: vec![
                make_block("main", vec![
                    Statement::Each(Each {
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
                        body: vec![
                            Statement::Expression(Expr::BlockCall(BlockCall {
                                name: "process".to_string(),
                                args: vec![],
                                span: span(3, 5),
                            })),
                        ],
                        span: span(2, 1),
                    }),
                ]),
            ],
        };

        let (registry, _) = BlockRegistry::build(&program);
        let diags = check(&program, &registry);

        assert_eq!(diags.len(), 1);
        assert!(diags[0].message.contains("undefined block ::process"));
    }
}
