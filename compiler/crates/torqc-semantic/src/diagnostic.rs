use std::fmt;

use torqc_ast::ast::Span;

// ---------------------------------------------------------------------------
// Severity
// ---------------------------------------------------------------------------

/// Indicates whether a diagnostic is an error or a warning.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Error,
    Warning,
}

impl fmt::Display for Severity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Severity::Error => write!(f, "error"),
            Severity::Warning => write!(f, "warning"),
        }
    }
}

// ---------------------------------------------------------------------------
// Diagnostic
// ---------------------------------------------------------------------------

/// A single compiler diagnostic (error or warning) tied to a source location.
#[derive(Debug, Clone, PartialEq)]
pub struct Diagnostic {
    pub severity: Severity,
    pub message: String,
    pub span: Span,
    pub hint: Option<String>,
}

impl Diagnostic {
    /// Create an error diagnostic.
    pub fn error(message: impl Into<String>, span: Span) -> Self {
        Self {
            severity: Severity::Error,
            message: message.into(),
            span,
            hint: None,
        }
    }

    /// Create a warning diagnostic.
    pub fn warning(message: impl Into<String>, span: Span) -> Self {
        Self {
            severity: Severity::Warning,
            message: message.into(),
            span,
            hint: None,
        }
    }

    /// Attach a hint to this diagnostic, returning the modified diagnostic.
    pub fn with_hint(mut self, hint: impl Into<String>) -> Self {
        self.hint = Some(hint.into());
        self
    }

    /// Returns `true` if this diagnostic has `Severity::Error`.
    pub fn is_error(&self) -> bool {
        self.severity == Severity::Error
    }
}

impl fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}:{}:{}: {}: {}",
            self.span.file, self.span.line, self.span.col, self.severity, self.message,
        )?;
        if let Some(hint) = &self.hint {
            write!(f, "\n  hint: {}", hint)?;
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    fn test_span() -> Span {
        Span {
            file: "test.torq".to_string(),
            line: 10,
            col: 5,
        }
    }

    #[test]
    fn error_diagnostic() {
        let diag = Diagnostic::error("undefined variable `x`", test_span());

        assert_eq!(diag.severity, Severity::Error);
        assert_eq!(diag.message, "undefined variable `x`");
        assert_eq!(diag.span, test_span());
        assert!(diag.hint.is_none());
        assert!(diag.is_error());
        assert_eq!(
            diag.to_string(),
            "test.torq:10:5: error: undefined variable `x`"
        );
    }

    #[test]
    fn warning_diagnostic() {
        let diag = Diagnostic::warning("unused variable `y`", test_span());

        assert_eq!(diag.severity, Severity::Warning);
        assert_eq!(diag.message, "unused variable `y`");
        assert!(!diag.is_error());
        assert_eq!(
            diag.to_string(),
            "test.torq:10:5: warning: unused variable `y`"
        );
    }

    #[test]
    fn diagnostic_with_hint() {
        let diag = Diagnostic::error("type mismatch", test_span())
            .with_hint("expected `Int`, found `Str`");

        assert!(diag.hint.is_some());
        assert_eq!(diag.hint.as_deref(), Some("expected `Int`, found `Str`"));
        assert_eq!(
            diag.to_string(),
            "test.torq:10:5: error: type mismatch\n  hint: expected `Int`, found `Str`"
        );
    }
}
