use logos::Logos;

/// All token types for the TORQ language.
///
/// Logos config: skip horizontal whitespace (spaces and tabs) but NOT newlines,
/// since newlines are structurally significant in TORQ.
#[derive(Logos, Debug, Clone, PartialEq)]
#[logos(skip r"[ \t]+")]
pub enum Token {
    // ───────────────────────── Sigil Variables ─────────────────────────

    /// `$name` or `$name.field.sub` — scalar variable (dotted access)
    #[regex(r"\$[a-zA-Z_][a-zA-Z0-9_]*(\.[a-zA-Z_][a-zA-Z0-9_]*)*")]
    ScalarVar,

    /// `@name` — array variable
    #[regex(r"@[a-zA-Z_][a-zA-Z0-9_]*")]
    ArrayVar,

    /// `%name` — dictionary variable
    #[regex(r"%[a-zA-Z_][a-zA-Z0-9_]*")]
    DictVar,

    /// `!name` — error variable
    #[regex(r"![a-zA-Z_][a-zA-Z0-9_]*")]
    ErrorVar,

    /// `*name` — shared variable
    #[regex(r"\*[a-zA-Z_][a-zA-Z0-9_]*")]
    SharedVar,

    /// `~name` — regex variable
    #[regex(r"~[a-zA-Z_][a-zA-Z0-9_]*")]
    RegexVar,

    /// `&::name` — block reference
    #[regex(r"&::[a-zA-Z_][a-zA-Z0-9_]*")]
    BlockRef,

    /// `::name` — block name
    #[regex(r"::[a-zA-Z_][a-zA-Z0-9_]*")]
    BlockName,

    // ───────────────────────── Literals ─────────────────────────

    /// Floating-point literal, e.g. `19.99`
    /// Priority 3 so it matches before Int.
    #[regex(r"[0-9]+\.[0-9]+", priority = 3)]
    Float,

    /// Integer literal, e.g. `42`
    #[regex(r"[0-9]+", priority = 2)]
    Int,

    /// Double-quoted string literal with escape support, e.g. `"hello world"`
    #[regex(r#""([^"\\]|\\.)*""#)]
    StringLit,

    /// Triple-quote marker for multiline strings: `"""`
    /// Priority 5 so it matches before StringLit.
    #[token("\"\"\"", priority = 5)]
    TripleQuote,

    /// Duration in milliseconds, e.g. `100ms`
    #[regex(r"[0-9]+ms", priority = 4)]
    DurationMs,

    /// Duration in seconds, e.g. `1s`
    #[regex(r"[0-9]+s", priority = 4)]
    DurationS,

    /// Duration in minutes, e.g. `5m`
    #[regex(r"[0-9]+m", priority = 4)]
    DurationM,

    /// Duration in hours, e.g. `2h`
    #[regex(r"[0-9]+h", priority = 4)]
    DurationH,

    /// Duration in days, e.g. `7d`
    #[regex(r"[0-9]+d", priority = 4)]
    DurationD,

    // ───────────────────────── Keywords ─────────────────────────
    // All keywords have priority 3 so they match before Ident (priority 1).

    #[token("each", priority = 3)]
    Each,

    #[token("sequential", priority = 3)]
    Sequential,

    #[token("loop", priority = 3)]
    Loop,

    #[token("match", priority = 3)]
    Match,

    #[token("break", priority = 3)]
    Break,

    #[token("range", priority = 3)]
    Range,

    #[token("true", priority = 3)]
    True,

    #[token("false", priority = 3)]
    False,

    #[token("null", priority = 3)]
    Null,

    #[token("as", priority = 3)]
    As,

    #[token("to", priority = 3)]
    To,

    #[token("fail", priority = 3)]
    Fail,

    #[token("retry", priority = 3)]
    Retry,

    #[token("respond", priority = 3)]
    Respond,

    #[token("delay", priority = 3)]
    Delay,

    #[token("backoff", priority = 3)]
    Backoff,

    #[token("exponential", priority = 3)]
    Exponential,

    #[token("where", priority = 3)]
    Where,

    #[token("by", priority = 3)]
    By,

    #[token("desc", priority = 3)]
    Desc,

    #[token("template", priority = 3)]
    Template,

    #[token("validate", priority = 3)]
    Validate,

    #[token("required", priority = 3)]
    Required,

    #[token("binary", priority = 3)]
    Binary,

    #[token("recursive", priority = 3)]
    Recursive,

    #[token("redirect", priority = 3)]
    Redirect,

    #[token("json", priority = 3)]
    Json,

    #[token("xml", priority = 3)]
    Xml,

    #[token("yaml", priority = 3)]
    Yaml,

    #[token("csv", priority = 3)]
    Csv,

    #[token("toml", priority = 3)]
    Toml,

    #[token("data", priority = 3)]
    Data,

    #[token("rollback", priority = 3)]
    Rollback,

    #[token("timeout", priority = 3)]
    Timeout,

    // ───────────────────────── Operators ─────────────────────────

    /// `|` — pipe operator
    #[token("|")]
    Pipe,

    /// `->` — arrow
    #[token("->")]
    Arrow,

    /// `=` — equals / assignment
    #[token("=")]
    Eq,

    /// `!=` — not equal
    #[token("!=")]
    NotEq,

    /// `>=` — greater than or equal
    #[token(">=")]
    GtEq,

    /// `<=` — less than or equal
    #[token("<=")]
    LtEq,

    /// `>` — greater than
    #[token(">")]
    Gt,

    /// `<` — less than
    #[token("<")]
    Lt,

    /// `+` — plus
    #[token("+")]
    Plus,

    /// `-` — minus
    #[token("-")]
    Minus,

    /// `**` — power / exponentiation (priority 3, above single `*`)
    #[token("**", priority = 3)]
    Power,

    /// `*` — star (multiplication, glob, etc.) — only when not a SharedVar sigil
    #[token("*")]
    Star,

    /// `/` — slash / division
    #[token("/")]
    Slash,

    /// `?` — question mark
    #[token("?")]
    Question,

    /// `:` — colon
    #[token(":")]
    Colon,

    /// `&` — ampersand
    #[token("&")]
    Ampersand,

    /// `%` — percent / modulo (standalone, when not followed by identifier)
    #[token("%")]
    Percent,

    /// `.` — dot
    #[token(".")]
    Dot,

    // ───────────────────────── Delimiters ─────────────────────────

    #[token("[")]
    LBracket,

    #[token("]")]
    RBracket,

    #[token("{")]
    LBrace,

    #[token("}")]
    RBrace,

    #[token("(")]
    LParen,

    #[token(")")]
    RParen,

    // ───────────────────────── Structure ─────────────────────────

    /// Newline — structurally significant in TORQ
    #[token("\n")]
    Newline,

    // ───────────────────────── Comments ─────────────────────────

    /// Doc comment: `## ...` (priority 3, higher than PromptComment)
    #[regex(r"##[^\n]*", priority = 3)]
    DocComment,

    /// Prompt / line comment: `# ...`
    #[regex(r"#[^\n]*", priority = 2)]
    PromptComment,

    // ───────────────────────── Identifiers ─────────────────────────

    /// Bare identifier, optionally dotted: `print`, `http.get`, `sys.fs.read`
    #[regex(r"[a-zA-Z_][a-zA-Z0-9_]*(\.[a-zA-Z_][a-zA-Z0-9_]*)*", priority = 1)]
    Ident,
}

#[cfg(test)]
mod tests {
    use super::*;
    use logos::Logos;

    /// Helper: lex the input and collect all Ok tokens.
    fn lex(input: &str) -> Vec<Token> {
        Token::lexer(input)
            .into_iter()
            .map(|r| r.expect("unexpected lexer error"))
            .collect()
    }

    // ── Sigil variables ──────────────────────────────────────────

    #[test]
    fn sigil_scalar() {
        assert_eq!(lex("$name"), vec![Token::ScalarVar]);
    }

    #[test]
    fn sigil_scalar_dotted() {
        assert_eq!(lex("$req.body"), vec![Token::ScalarVar]);
    }

    #[test]
    fn sigil_array() {
        assert_eq!(lex("@items"), vec![Token::ArrayVar]);
    }

    #[test]
    fn sigil_dict() {
        assert_eq!(lex("%config"), vec![Token::DictVar]);
    }

    #[test]
    fn sigil_error() {
        assert_eq!(lex("!err"), vec![Token::ErrorVar]);
    }

    #[test]
    fn sigil_shared() {
        assert_eq!(lex("*shared"), vec![Token::SharedVar]);
    }

    #[test]
    fn sigil_regex() {
        assert_eq!(lex("~pattern"), vec![Token::RegexVar]);
    }

    #[test]
    fn sigil_block_ref() {
        assert_eq!(lex("&::handler"), vec![Token::BlockRef]);
    }

    #[test]
    fn sigil_block_name() {
        assert_eq!(lex("::main"), vec![Token::BlockName]);
    }

    // ── Literals ──────────────────────────────────────────────────

    #[test]
    fn literal_float() {
        assert_eq!(lex("19.99"), vec![Token::Float]);
    }

    #[test]
    fn literal_int() {
        assert_eq!(lex("42"), vec![Token::Int]);
    }

    #[test]
    fn literal_float_before_int() {
        // Float must match "19.99" as one token, not Int + Dot + Int
        assert_eq!(lex("19.99"), vec![Token::Float]);
    }

    #[test]
    fn literal_string() {
        assert_eq!(lex(r#""hello world""#), vec![Token::StringLit]);
    }

    #[test]
    fn literal_string_with_escape() {
        assert_eq!(lex(r#""hello \"world\"""#), vec![Token::StringLit]);
    }

    #[test]
    fn literal_triple_quote() {
        assert_eq!(lex(r#"""""#), vec![Token::TripleQuote]);
    }

    #[test]
    fn duration_ms() {
        assert_eq!(lex("100ms"), vec![Token::DurationMs]);
    }

    #[test]
    fn duration_s() {
        assert_eq!(lex("1s"), vec![Token::DurationS]);
    }

    #[test]
    fn duration_m() {
        assert_eq!(lex("5m"), vec![Token::DurationM]);
    }

    #[test]
    fn duration_h() {
        assert_eq!(lex("2h"), vec![Token::DurationH]);
    }

    #[test]
    fn duration_d() {
        assert_eq!(lex("7d"), vec![Token::DurationD]);
    }

    // ── Keywords ──────────────────────────────────────────────────

    #[test]
    fn keywords() {
        assert_eq!(lex("each"), vec![Token::Each]);
        assert_eq!(lex("sequential"), vec![Token::Sequential]);
        assert_eq!(lex("loop"), vec![Token::Loop]);
        assert_eq!(lex("match"), vec![Token::Match]);
        assert_eq!(lex("break"), vec![Token::Break]);
        assert_eq!(lex("range"), vec![Token::Range]);
        assert_eq!(lex("true"), vec![Token::True]);
        assert_eq!(lex("false"), vec![Token::False]);
        assert_eq!(lex("null"), vec![Token::Null]);
        assert_eq!(lex("as"), vec![Token::As]);
        assert_eq!(lex("to"), vec![Token::To]);
        assert_eq!(lex("fail"), vec![Token::Fail]);
        assert_eq!(lex("retry"), vec![Token::Retry]);
        assert_eq!(lex("respond"), vec![Token::Respond]);
        assert_eq!(lex("delay"), vec![Token::Delay]);
        assert_eq!(lex("backoff"), vec![Token::Backoff]);
        assert_eq!(lex("exponential"), vec![Token::Exponential]);
        assert_eq!(lex("where"), vec![Token::Where]);
        assert_eq!(lex("by"), vec![Token::By]);
        assert_eq!(lex("desc"), vec![Token::Desc]);
        assert_eq!(lex("template"), vec![Token::Template]);
        assert_eq!(lex("validate"), vec![Token::Validate]);
        assert_eq!(lex("required"), vec![Token::Required]);
        assert_eq!(lex("binary"), vec![Token::Binary]);
        assert_eq!(lex("recursive"), vec![Token::Recursive]);
        assert_eq!(lex("redirect"), vec![Token::Redirect]);
        assert_eq!(lex("json"), vec![Token::Json]);
        assert_eq!(lex("xml"), vec![Token::Xml]);
        assert_eq!(lex("yaml"), vec![Token::Yaml]);
        assert_eq!(lex("csv"), vec![Token::Csv]);
        assert_eq!(lex("toml"), vec![Token::Toml]);
        assert_eq!(lex("data"), vec![Token::Data]);
        assert_eq!(lex("rollback"), vec![Token::Rollback]);
        assert_eq!(lex("timeout"), vec![Token::Timeout]);
    }

    #[test]
    fn keyword_vs_ident() {
        // A keyword followed by more word chars should be an Ident, not a keyword.
        assert_eq!(lex("matching"), vec![Token::Ident]);
        assert_eq!(lex("toml_file"), vec![Token::Ident]);
    }

    // ── Operators ──────────────────────────────────────────────────

    #[test]
    fn operators() {
        assert_eq!(lex("|"), vec![Token::Pipe]);
        assert_eq!(lex("->"), vec![Token::Arrow]);
        assert_eq!(lex("="), vec![Token::Eq]);
        assert_eq!(lex("!="), vec![Token::NotEq]);
        assert_eq!(lex(">="), vec![Token::GtEq]);
        assert_eq!(lex("<="), vec![Token::LtEq]);
        assert_eq!(lex(">"), vec![Token::Gt]);
        assert_eq!(lex("<"), vec![Token::Lt]);
        assert_eq!(lex("+"), vec![Token::Plus]);
        assert_eq!(lex("-"), vec![Token::Minus]);
        assert_eq!(lex("/"), vec![Token::Slash]);
        assert_eq!(lex("?"), vec![Token::Question]);
        assert_eq!(lex(":"), vec![Token::Colon]);
        assert_eq!(lex("&"), vec![Token::Ampersand]);
        assert_eq!(lex("."), vec![Token::Dot]);
    }

    #[test]
    fn power_vs_star() {
        // `**` should be a single Power token, not two Stars.
        assert_eq!(lex("**"), vec![Token::Power]);
        assert_eq!(lex("*"), vec![Token::Star]);
    }

    // ── Delimiters ──────────────────────────────────────────────────

    #[test]
    fn delimiters() {
        assert_eq!(
            lex("[ ] { } ( )"),
            vec![
                Token::LBracket,
                Token::RBracket,
                Token::LBrace,
                Token::RBrace,
                Token::LParen,
                Token::RParen,
            ]
        );
    }

    // ── Structure ──────────────────────────────────────────────────

    #[test]
    fn newline_preserved() {
        assert_eq!(lex("\n"), vec![Token::Newline]);
        assert_eq!(lex("\n\n"), vec![Token::Newline, Token::Newline]);
    }

    #[test]
    fn spaces_skipped() {
        assert_eq!(lex("   42   "), vec![Token::Int]);
    }

    #[test]
    fn tabs_skipped() {
        assert_eq!(lex("\t42\t"), vec![Token::Int]);
    }

    // ── Comments ──────────────────────────────────────────────────

    #[test]
    fn doc_comment() {
        assert_eq!(lex("## this is a doc comment"), vec![Token::DocComment]);
    }

    #[test]
    fn prompt_comment() {
        assert_eq!(lex("# this is a prompt"), vec![Token::PromptComment]);
    }

    #[test]
    fn doc_comment_priority_over_prompt() {
        // `##` must be DocComment, not PromptComment
        assert_eq!(lex("## doc"), vec![Token::DocComment]);
    }

    // ── Identifiers ──────────────────────────────────────────────────

    #[test]
    fn ident_simple() {
        assert_eq!(lex("print"), vec![Token::Ident]);
    }

    #[test]
    fn ident_dotted() {
        assert_eq!(lex("http.get"), vec![Token::Ident]);
        assert_eq!(lex("sys.fs.read"), vec![Token::Ident]);
    }

    // ── Compound expressions ──────────────────────────────────────

    #[test]
    fn pipe_chain() {
        assert_eq!(
            lex("$data | filter | print"),
            vec![
                Token::ScalarVar,
                Token::Pipe,
                Token::Ident,
                Token::Pipe,
                Token::Ident,
            ]
        );
    }

    #[test]
    fn multiline() {
        assert_eq!(
            lex("each $x\n  print"),
            vec![
                Token::Each,
                Token::ScalarVar,
                Token::Newline,
                Token::Ident,
            ]
        );
    }
}
