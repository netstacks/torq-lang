use torqc_ast::ast::Program;

use crate::checks;
use crate::diagnostic::Diagnostic;
use crate::registry::BlockRegistry;

// ---------------------------------------------------------------------------
// AnalysisResult
// ---------------------------------------------------------------------------

/// The result of running all semantic checks on a [`Program`].
///
/// Contains every diagnostic (errors **and** warnings) discovered during
/// analysis, sorted by source location (file, line, column).
#[derive(Debug)]
pub struct AnalysisResult {
    pub diagnostics: Vec<Diagnostic>,
}

impl AnalysisResult {
    /// Returns `true` if any diagnostic is an error.
    pub fn has_errors(&self) -> bool {
        self.diagnostics.iter().any(|d| d.is_error())
    }

    /// Count of error-level diagnostics.
    pub fn error_count(&self) -> usize {
        self.diagnostics.iter().filter(|d| d.is_error()).count()
    }

    /// Count of warning-level diagnostics (everything that is not an error).
    pub fn warning_count(&self) -> usize {
        self.diagnostics.iter().filter(|d| !d.is_error()).count()
    }
}

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

/// Run **all** semantic checks on `program` and return the collected
/// diagnostics sorted by source location.
///
/// The analysis proceeds in phases:
///
/// 1. **Registry** — build the block registry (detects duplicate block names).
/// 2. **Block resolution** — verify every block call/ref targets a known block.
/// 3. **Parameter arity** — verify argument counts match parameter counts.
/// 4. **Variable scope** — verify variables are defined before use.
/// 5. **Control flow** — verify `break` placement and shared-state safety.
pub fn analyze(program: &Program) -> AnalysisResult {
    let mut diagnostics = Vec::new();

    // Phase 1: Build block registry (also detects duplicates)
    let (registry, mut reg_diags) = BlockRegistry::build(program);
    diagnostics.append(&mut reg_diags);

    // Phase 2: Block resolution
    diagnostics.append(&mut checks::resolve_blocks::check(program, &registry));

    // Phase 3: Parameter arity
    diagnostics.append(&mut checks::arity::check(program, &registry));

    // Phase 4: Variable scope
    diagnostics.append(&mut checks::scope::check(program));

    // Phase 5: Control flow
    diagnostics.append(&mut checks::control_flow::check(program));

    // Sort by file, line, col
    diagnostics.sort_by(|a, b| {
        a.span
            .file
            .cmp(&b.span.file)
            .then(a.span.line.cmp(&b.span.line))
            .then(a.span.col.cmp(&b.span.col))
    });

    AnalysisResult { diagnostics }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use torqc_ast::ast::*;

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

    // -- Test 1: clean program -----------------------------------------------

    #[test]
    fn clean_program() {
        // A single block with a literal expression should produce 0 diagnostics.
        let program = Program {
            blocks: vec![make_block(
                "main",
                vec![Statement::Expression(Expr::Literal(Literal::Int(
                    42,
                    span(2, 5),
                )))],
            )],
        };

        let result = analyze(&program);

        assert!(!result.has_errors());
        assert_eq!(result.error_count(), 0);
        assert_eq!(result.warning_count(), 0);
        assert!(result.diagnostics.is_empty());
    }

    // -- Test 2: multiple errors collected and sorted -------------------------

    #[test]
    fn multiple_diagnostics_collected() {
        // Three diagnostics at lines 3, 5, and 7.
        //
        // 1. Undefined block call   (line 3)  — ERROR   (resolve_blocks)
        // 2. Undefined variable      (line 5)  — WARNING (scope)
        // 3. Break outside loop      (line 7)  — ERROR   (control_flow)
        let program = Program {
            blocks: vec![make_block(
                "main",
                vec![
                    // line 3: call to undefined block
                    Statement::Expression(Expr::BlockCall(BlockCall {
                        name: "nonexistent".to_string(),
                        args: vec![],
                        span: span(3, 1),
                    })),
                    // line 5: use of undefined variable (warning)
                    Statement::Expression(Expr::Variable(Variable {
                        sigil: Sigil::Scalar,
                        name: "undef".to_string(),
                        span: span(5, 1),
                    })),
                    // line 7: break outside loop
                    Statement::Expression(Expr::Break(span(7, 1))),
                ],
            )],
        };

        let result = analyze(&program);

        assert!(result.has_errors());
        assert_eq!(result.error_count(), 2);
        assert_eq!(result.warning_count(), 1);

        // Verify sorted by line number
        assert_eq!(result.diagnostics[0].span.line, 3);
        assert_eq!(result.diagnostics[1].span.line, 5);
        assert_eq!(result.diagnostics[2].span.line, 7);

        // Verify each diagnostic is the expected kind
        assert!(result.diagnostics[0].message.contains("undefined block"));
        assert!(result.diagnostics[0].is_error());
        assert!(result.diagnostics[1].message.contains("variable"));
        assert!(!result.diagnostics[1].is_error()); // scope is now a warning
        assert!(result.diagnostics[2].message.contains("break"));
        assert!(result.diagnostics[2].is_error());
    }

    // -- Test 3: errors and warnings together ---------------------------------

    #[test]
    fn errors_and_warnings_together() {
        // 1 error  (break outside loop at line 2)
        // 1 warning (shared var in parallel each at line 5)
        let program = Program {
            blocks: vec![make_block(
                "main",
                vec![
                    // line 2: break outside loop — error
                    Statement::Expression(Expr::Break(span(2, 1))),
                    // line 4-6: parallel each with shared var assignment — warning
                    Statement::Each(Each {
                        iterable: Box::new(Expr::Array(vec![], span(4, 10))),
                        binding: Variable {
                            sigil: Sigil::Scalar,
                            name: "item".to_string(),
                            span: span(4, 6),
                        },
                        sequential: false,
                        body: vec![Statement::Assignment(Assignment {
                            target: Variable {
                                sigil: Sigil::Shared,
                                name: "counter".to_string(),
                                span: span(5, 5),
                            },
                            value: Box::new(Expr::Literal(Literal::Int(1, span(5, 18)))),
                            span: span(5, 5),
                        })],
                        span: span(4, 1),
                    }),
                ],
            )],
        };

        let result = analyze(&program);

        assert!(result.has_errors());
        assert_eq!(result.error_count(), 1);
        assert_eq!(result.warning_count(), 1);

        // Sorted: error at line 2 comes before warning at line 5
        assert!(result.diagnostics[0].is_error());
        assert_eq!(result.diagnostics[0].span.line, 2);
        assert!(!result.diagnostics[1].is_error());
        assert_eq!(result.diagnostics[1].span.line, 5);
    }

    // -- Test 4: empty program ------------------------------------------------

    #[test]
    fn empty_program() {
        let program = Program { blocks: vec![] };

        let result = analyze(&program);

        assert!(!result.has_errors());
        assert_eq!(result.error_count(), 0);
        assert_eq!(result.warning_count(), 0);
    }

    // -- Test 5: duplicate block detected via registry ------------------------

    #[test]
    fn duplicate_block_detected() {
        let program = Program {
            blocks: vec![
                make_block("main", vec![]),
                Block {
                    name: "main".to_string(),
                    params: vec![],
                    body: vec![],
                    doc_comments: vec![],
                    span: span(10, 1),
                },
            ],
        };

        let result = analyze(&program);

        assert!(result.has_errors());
        assert_eq!(result.error_count(), 1);
        assert!(result.diagnostics[0].message.contains("duplicate block"));
    }
}
