use std::collections::HashMap;

use torqc_ast::ast::*;

use crate::diagnostic::Diagnostic;

// ---------------------------------------------------------------------------
// BlockInfo — metadata about a registered block
// ---------------------------------------------------------------------------

/// Summary information about a single block definition.
#[derive(Debug, Clone)]
pub struct BlockInfo {
    pub name: String,
    pub param_count: usize,
    pub span: Span,
}

// ---------------------------------------------------------------------------
// BlockRegistry — maps block names to their info
// ---------------------------------------------------------------------------

/// A registry of all blocks defined in a program.
///
/// Built from a parsed [`Program`] via [`BlockRegistry::build`], which also
/// returns any diagnostics encountered (e.g. duplicate block names).
#[derive(Debug)]
pub struct BlockRegistry {
    blocks: HashMap<String, BlockInfo>,
}

impl BlockRegistry {
    /// Scan every block in `program` and register it by name.
    ///
    /// If two blocks share the same name the second occurrence is rejected and
    /// an error diagnostic is emitted with a hint pointing to the first
    /// definition.
    pub fn build(program: &Program) -> (Self, Vec<Diagnostic>) {
        let mut blocks = HashMap::new();
        let mut diagnostics = Vec::new();

        for block in &program.blocks {
            if let Some(existing) = blocks.get(&block.name) {
                let existing: &BlockInfo = existing;
                let diag = Diagnostic::error(
                    format!("duplicate block ::{}",  block.name),
                    block.span.clone(),
                )
                .with_hint(format!(
                    "first defined at {}:{}:{}",
                    existing.span.file, existing.span.line, existing.span.col,
                ));
                diagnostics.push(diag);
            } else {
                blocks.insert(
                    block.name.clone(),
                    BlockInfo {
                        name: block.name.clone(),
                        param_count: block.params.len(),
                        span: block.span.clone(),
                    },
                );
            }
        }

        (Self { blocks }, diagnostics)
    }

    /// Look up a block by name.
    pub fn get(&self, name: &str) -> Option<&BlockInfo> {
        self.blocks.get(name)
    }

    /// Returns `true` if the registry contains a block with the given name.
    pub fn contains(&self, name: &str) -> bool {
        self.blocks.contains_key(name)
    }

    /// Return a sorted list of all registered block names.
    pub fn names(&self) -> Vec<&str> {
        let mut names: Vec<&str> = self.blocks.keys().map(|s| s.as_str()).collect();
        names.sort();
        names
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

    /// Helper: create a minimal block.
    fn make_block(name: &str, param_count: usize, line: usize) -> Block {
        let params = (0..param_count)
            .map(|i| Param {
                sigil: Sigil::Scalar,
                name: format!("p{}", i),
                span: span(line, 1),
            })
            .collect();

        Block {
            name: name.to_string(),
            params,
            body: vec![],
            doc_comments: vec![],
            span: span(line, 1),
        }
    }

    #[test]
    fn registers_blocks() {
        let program = Program {
            blocks: vec![
                make_block("main", 0, 1),
                make_block("helper", 2, 10),
            ],
        };

        let (registry, diagnostics) = BlockRegistry::build(&program);

        assert!(diagnostics.is_empty());
        assert!(registry.contains("main"));
        assert!(registry.contains("helper"));
        assert!(!registry.contains("missing"));

        let main = registry.get("main").expect("main should be registered");
        assert_eq!(main.param_count, 0);

        let helper = registry.get("helper").expect("helper should be registered");
        assert_eq!(helper.param_count, 2);

        assert_eq!(registry.names(), vec!["helper", "main"]);
    }

    #[test]
    fn detects_duplicate_blocks() {
        let program = Program {
            blocks: vec![
                make_block("main", 0, 1),
                make_block("main", 1, 20),
            ],
        };

        let (registry, diagnostics) = BlockRegistry::build(&program);

        assert_eq!(diagnostics.len(), 1);
        let diag = &diagnostics[0];
        assert!(diag.message.contains("duplicate block ::main"));
        assert!(diag.hint.is_some());

        // Only the first definition is kept.
        let main = registry.get("main").expect("main should be registered");
        assert_eq!(main.param_count, 0);
        assert_eq!(main.span.line, 1);
    }

    #[test]
    fn empty_program() {
        let program = Program { blocks: vec![] };

        let (registry, diagnostics) = BlockRegistry::build(&program);

        assert!(diagnostics.is_empty());
        assert!(registry.names().is_empty());
    }
}
