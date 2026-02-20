use std::collections::HashSet;

use torqc_ast::ast::*;

use crate::diagnostic::Diagnostic;

// ---------------------------------------------------------------------------
// Scope — a stack of variable-name sets
// ---------------------------------------------------------------------------

struct Scope {
    levels: Vec<HashSet<String>>,
}

impl Scope {
    fn new() -> Self {
        Self {
            levels: vec![HashSet::new()],
        }
    }

    /// Push a new (empty) scope level.
    fn push(&mut self) {
        self.levels.push(HashSet::new());
    }

    /// Pop the topmost scope level.
    fn pop(&mut self) {
        self.levels.pop();
    }

    /// Define a variable in the current (topmost) scope level.
    fn define(&mut self, name: &str) {
        if let Some(top) = self.levels.last_mut() {
            top.insert(name.to_string());
        }
    }

    /// Check whether a variable is defined in *any* level (inner-to-outer).
    fn is_defined(&self, name: &str) -> bool {
        self.levels.iter().rev().any(|level| level.contains(name))
    }
}

// ---------------------------------------------------------------------------
// Sigil formatting helper
// ---------------------------------------------------------------------------

fn sigil_prefix(sigil: &Sigil) -> &'static str {
    match sigil {
        Sigil::Scalar => "$",
        Sigil::Array => "@",
        Sigil::Dict => "%",
        Sigil::Error => "!",
        Sigil::Regex => "/",
        Sigil::Shared => "*",
        Sigil::BlockRef => "&",
    }
}

fn format_variable(sigil: &Sigil, name: &str) -> String {
    format!("{}{}", sigil_prefix(sigil), name)
}

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

/// Walk every block in `program` and emit an error for each variable that is
/// used before it has been defined in any enclosing scope.
///
/// Shared (`*`) variables are treated as globally accessible and are never
/// flagged.
pub fn check(program: &Program) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();

    for block in &program.blocks {
        let mut scope = Scope::new();

        // Block parameters define variables in the block scope.
        for param in &block.params {
            scope.define(&param.name);
        }

        check_statements(&block.body, &mut scope, &mut diagnostics);
    }

    diagnostics
}

// ---------------------------------------------------------------------------
// Statement walker
// ---------------------------------------------------------------------------

fn check_statements(stmts: &[Statement], scope: &mut Scope, diags: &mut Vec<Diagnostic>) {
    for stmt in stmts {
        check_statement(stmt, scope, diags);
    }
}

fn check_statement(stmt: &Statement, scope: &mut Scope, diags: &mut Vec<Diagnostic>) {
    match stmt {
        Statement::Pipeline(pipeline) => {
            for stage in &pipeline.stages {
                check_expr(stage, scope, diags);
            }
        }
        Statement::Assignment(assign) => {
            // Evaluate the RHS first (before defining the variable).
            check_expr(&assign.value, scope, diags);
            // Then define the target variable in the current scope.
            scope.define(&assign.target.name);
        }
        Statement::Each(each) => {
            // Check the iterable in the current scope.
            check_expr(&each.iterable, scope, diags);
            // The binding is only visible inside the each body.
            scope.push();
            scope.define(&each.binding.name);
            check_statements(&each.body, scope, diags);
            scope.pop();
        }
        Statement::Loop(lp) => {
            if let Some(cond) = &lp.condition {
                check_expr(cond, scope, diags);
            }
            scope.push();
            check_statements(&lp.body, scope, diags);
            scope.pop();
        }
        Statement::Expression(expr) => {
            check_expr(expr, scope, diags);
        }
    }
}

// ---------------------------------------------------------------------------
// Expression walker
// ---------------------------------------------------------------------------

fn check_expr(expr: &Expr, scope: &mut Scope, diags: &mut Vec<Diagnostic>) {
    match expr {
        Expr::Variable(var) => {
            check_variable_usage(var, scope, diags);
        }
        Expr::StringInterp(parts, _) => {
            for part in parts {
                if let StringPart::Interpolation(var) = part {
                    check_variable_usage(var, scope, diags);
                }
            }
        }
        Expr::BinOp(bin) => {
            check_expr(&bin.left, scope, diags);
            check_expr(&bin.right, scope, diags);
        }
        Expr::Pipeline(pipeline) => {
            for stage in &pipeline.stages {
                check_expr(stage, scope, diags);
            }
        }
        Expr::BlockCall(bc) => {
            for arg in &bc.args {
                check_expr(arg, scope, diags);
            }
        }
        Expr::Call(call) => {
            for arg in &call.args {
                check_expr(arg, scope, diags);
            }
        }
        Expr::Array(elems, _) => {
            for elem in elems {
                check_expr(elem, scope, diags);
            }
        }
        Expr::Dict(entries, _) => {
            for entry in entries {
                check_expr(&entry.value, scope, diags);
            }
        }
        Expr::MemberAccess(ma) => {
            check_expr(&ma.object, scope, diags);
        }
        Expr::Ternary(tern) => {
            check_expr(&tern.condition, scope, diags);
            check_expr(&tern.then_expr, scope, diags);
            check_expr(&tern.else_expr, scope, diags);
        }
        Expr::Group(inner, _) => {
            check_expr(inner, scope, diags);
        }
        Expr::Match(m) => {
            for arm in &m.arms {
                // Each arm gets its own scope for pattern-bound variables.
                scope.push();
                define_pattern_vars(&arm.pattern, scope);
                check_expr(&arm.body, scope, diags);
                scope.pop();
            }
        }
        // Terminal nodes with nothing to recurse into.
        Expr::Literal(_) | Expr::BlockRef(_, _) | Expr::Break(_) => {}
    }
}

// ---------------------------------------------------------------------------
// Variable usage check
// ---------------------------------------------------------------------------

fn check_variable_usage(var: &Variable, scope: &Scope, diags: &mut Vec<Diagnostic>) {
    // Shared variables (*) are globally accessible — skip checking.
    if var.sigil == Sigil::Shared {
        return;
    }

    if !scope.is_defined(&var.name) {
        diags.push(Diagnostic::error(
            format!(
                "variable '{}' used before definition",
                format_variable(&var.sigil, &var.name),
            ),
            var.span.clone(),
        ));
    }
}

// ---------------------------------------------------------------------------
// Pattern variable extraction
// ---------------------------------------------------------------------------

/// Recursively define all variables introduced by a pattern.
fn define_pattern_vars(pattern: &Pattern, scope: &mut Scope) {
    match pattern {
        Pattern::Variable(var) => {
            scope.define(&var.name);
        }
        Pattern::WithCaptures(inner, captures) => {
            define_pattern_vars(inner, scope);
            for cap in captures {
                scope.define(&cap.name);
            }
        }
        Pattern::And(patterns) => {
            for p in patterns {
                define_pattern_vars(p, scope);
            }
        }
        // Literal, Comparison, FieldMatch, Wildcard do not bind variables.
        Pattern::Literal(_)
        | Pattern::Comparison(_, _)
        | Pattern::FieldMatch(_)
        | Pattern::Wildcard => {}
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

    /// Helper: create a scalar variable.
    fn scalar(name: &str, line: usize, col: usize) -> Variable {
        Variable {
            sigil: Sigil::Scalar,
            name: name.to_string(),
            span: span(line, col),
        }
    }

    /// Helper: create a block with params and body.
    fn make_block(name: &str, params: Vec<Param>, body: Vec<Statement>) -> Block {
        Block {
            name: name.to_string(),
            params,
            body,
            doc_comments: vec![],
            span: span(1, 1),
        }
    }

    /// Helper: create a scalar Param.
    fn scalar_param(name: &str) -> Param {
        Param {
            sigil: Sigil::Scalar,
            name: name.to_string(),
            span: span(1, 1),
        }
    }

    // -- Tests ---------------------------------------------------------------

    #[test]
    fn param_defines_variable() {
        let program = Program {
            blocks: vec![make_block(
                "greet",
                vec![scalar_param("name")],
                vec![Statement::Expression(Expr::Variable(scalar(
                    "name", 2, 5,
                )))],
            )],
        };

        let diags = check(&program);
        assert!(diags.is_empty(), "expected no diagnostics, got: {:?}", diags);
    }

    #[test]
    fn undefined_variable() {
        let program = Program {
            blocks: vec![make_block(
                "main",
                vec![],
                vec![Statement::Expression(Expr::Variable(scalar("x", 2, 5)))],
            )],
        };

        let diags = check(&program);
        assert_eq!(diags.len(), 1);
        assert!(diags[0].message.contains("variable '$x' used before definition"));
        assert!(diags[0].is_error());
    }

    #[test]
    fn assignment_defines_variable() {
        let program = Program {
            blocks: vec![make_block(
                "main",
                vec![],
                vec![
                    Statement::Assignment(Assignment {
                        target: scalar("x", 2, 1),
                        value: Box::new(Expr::Literal(Literal::Int(42, span(2, 7)))),
                        span: span(2, 1),
                    }),
                    Statement::Expression(Expr::Variable(scalar("x", 3, 5))),
                ],
            )],
        };

        let diags = check(&program);
        assert!(diags.is_empty(), "expected no diagnostics, got: {:?}", diags);
    }

    #[test]
    fn each_binding_defines_variable() {
        let program = Program {
            blocks: vec![make_block(
                "main",
                vec![scalar_param("items")],
                vec![Statement::Each(Each {
                    iterable: Box::new(Expr::Variable(Variable {
                        sigil: Sigil::Scalar,
                        name: "items".to_string(),
                        span: span(2, 10),
                    })),
                    binding: scalar("item", 2, 6),
                    sequential: false,
                    body: vec![Statement::Expression(Expr::Variable(scalar(
                        "item", 3, 5,
                    )))],
                    span: span(2, 1),
                })],
            )],
        };

        let diags = check(&program);
        assert!(diags.is_empty(), "expected no diagnostics, got: {:?}", diags);
    }

    #[test]
    fn each_binding_not_visible_outside() {
        let program = Program {
            blocks: vec![make_block(
                "main",
                vec![scalar_param("items")],
                vec![
                    Statement::Each(Each {
                        iterable: Box::new(Expr::Variable(Variable {
                            sigil: Sigil::Scalar,
                            name: "items".to_string(),
                            span: span(2, 10),
                        })),
                        binding: scalar("item", 2, 6),
                        sequential: false,
                        body: vec![],
                        span: span(2, 1),
                    }),
                    // Use $item after the each block — should be undefined.
                    Statement::Expression(Expr::Variable(scalar("item", 4, 5))),
                ],
            )],
        };

        let diags = check(&program);
        assert_eq!(diags.len(), 1);
        assert!(diags[0].message.contains("variable '$item' used before definition"));
    }

    #[test]
    fn shared_vars_always_accessible() {
        let program = Program {
            blocks: vec![make_block(
                "main",
                vec![],
                vec![Statement::Expression(Expr::Variable(Variable {
                    sigil: Sigil::Shared,
                    name: "counter".to_string(),
                    span: span(2, 5),
                }))],
            )],
        };

        let diags = check(&program);
        assert!(diags.is_empty(), "expected no diagnostics, got: {:?}", diags);
    }

    #[test]
    fn outer_scope_visible_in_loop() {
        let program = Program {
            blocks: vec![make_block(
                "main",
                vec![],
                vec![
                    Statement::Assignment(Assignment {
                        target: scalar("count", 2, 1),
                        value: Box::new(Expr::Literal(Literal::Int(0, span(2, 10)))),
                        span: span(2, 1),
                    }),
                    Statement::Loop(Loop {
                        condition: None,
                        body: vec![Statement::Expression(Expr::Variable(scalar(
                            "count", 3, 5,
                        )))],
                        span: span(3, 1),
                    }),
                ],
            )],
        };

        let diags = check(&program);
        assert!(diags.is_empty(), "expected no diagnostics, got: {:?}", diags);
    }

    #[test]
    fn match_pattern_defines_variable() {
        let program = Program {
            blocks: vec![make_block(
                "main",
                vec![scalar_param("val")],
                vec![Statement::Expression(Expr::Match(Match {
                    arms: vec![MatchArm {
                        pattern: Pattern::Variable(scalar("target", 3, 5)),
                        body: Box::new(Expr::Variable(scalar("target", 3, 15))),
                        span: span(3, 1),
                    }],
                    span: span(2, 1),
                }))],
            )],
        };

        let diags = check(&program);
        assert!(diags.is_empty(), "expected no diagnostics, got: {:?}", diags);
    }

    #[test]
    fn string_interp_checks_variables() {
        let program = Program {
            blocks: vec![make_block(
                "main",
                vec![],
                vec![Statement::Expression(Expr::StringInterp(
                    vec![
                        StringPart::Literal("hello ".to_string()),
                        StringPart::Interpolation(scalar("who", 2, 10)),
                    ],
                    span(2, 1),
                ))],
            )],
        };

        let diags = check(&program);
        assert_eq!(diags.len(), 1);
        assert!(diags[0].message.contains("$who"));
    }

    #[test]
    fn array_sigil_variable_error() {
        let program = Program {
            blocks: vec![make_block(
                "main",
                vec![],
                vec![Statement::Expression(Expr::Variable(Variable {
                    sigil: Sigil::Array,
                    name: "list".to_string(),
                    span: span(2, 5),
                }))],
            )],
        };

        let diags = check(&program);
        assert_eq!(diags.len(), 1);
        assert!(diags[0].message.contains("@list"));
    }
}
