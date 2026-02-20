use std::fmt;

use torqc_ast::ast::*;
use torqc_lexer::lexer::{LexToken, SpannedToken};
use torqc_lexer::token::Token;

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct ParseError {
    pub message: String,
    pub line: usize,
    pub col: usize,
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}: {}", self.line, self.col, self.message)
    }
}

impl std::error::Error for ParseError {}

// ---------------------------------------------------------------------------
// Parser
// ---------------------------------------------------------------------------

pub struct Parser {
    tokens: Vec<SpannedToken>,
    pos: usize,
    file: String,
}

impl Parser {
    // ── constructor ─────────────────────────────────────────────────

    pub fn new(tokens: Vec<SpannedToken>, file: &str) -> Self {
        Self {
            tokens,
            pos: 0,
            file: file.to_string(),
        }
    }

    // ── public entry point ──────────────────────────────────────────

    pub fn parse(mut self) -> Result<Program, ParseError> {
        let mut blocks = Vec::new();
        let mut doc_comments: Vec<String> = Vec::new();

        self.skip_newlines();

        while !self.at_eof() {
            // Collect doc comments
            if self.check_token(Token::DocComment) {
                let text = self.current_text().to_string();
                // Strip the leading "## " or "##"
                let comment = text.strip_prefix("## ").unwrap_or(
                    text.strip_prefix("##").unwrap_or(&text),
                );
                doc_comments.push(comment.to_string());
                self.advance();
                self.skip_newlines();
                continue;
            }

            if self.check_token(Token::BlockName) {
                let block = self.parse_block(std::mem::take(&mut doc_comments))?;
                blocks.push(block);
            } else {
                return Err(self.error(format!(
                    "expected block definition (::name), found {:?}",
                    self.current_lex_token()
                )));
            }
            self.skip_newlines();
        }

        Ok(Program { blocks })
    }

    // ── block parsing ───────────────────────────────────────────────

    fn parse_block(&mut self, doc_comments: Vec<String>) -> Result<Block, ParseError> {
        let span = self.current_span();
        let name_text = self.current_text().to_string();
        let name = name_text
            .strip_prefix("::")
            .unwrap_or(&name_text)
            .to_string();
        self.advance(); // consume BlockName

        // Parse parameters
        let mut params = Vec::new();
        while self.is_variable_token() {
            params.push(self.parse_param()?);
        }

        // Expect newline then indent
        self.expect_newline()?;
        self.skip_newlines();

        let body = if self.check_lex(LexToken::Indent) {
            self.advance(); // consume Indent
            self.skip_newlines();
            self.parse_body()?
        } else {
            Vec::new()
        };

        Ok(Block {
            name,
            params,
            body,
            doc_comments,
            span,
        })
    }

    fn parse_param(&mut self) -> Result<Param, ParseError> {
        let span = self.current_span();
        let (sigil, name) = self.parse_variable_parts()?;
        Ok(Param { sigil, name, span })
    }

    // ── body: parse statements until Dedent ─────────────────────────

    fn parse_body(&mut self) -> Result<Vec<Statement>, ParseError> {
        let mut stmts = Vec::new();

        loop {
            self.skip_newlines();
            if self.at_eof() || self.check_lex(LexToken::Dedent) {
                break;
            }
            let stmt = self.parse_statement()?;
            stmts.push(stmt);
            self.skip_newlines();
        }

        if self.check_lex(LexToken::Dedent) {
            self.advance(); // consume Dedent
        }

        Ok(stmts)
    }

    // ── statement ───────────────────────────────────────────────────

    fn parse_statement(&mut self) -> Result<Statement, ParseError> {
        self.skip_newlines();

        // Each statement: `expr | each $var [sequential]` or `each $var INDENT body DEDENT`
        if self.check_token(Token::Each) {
            return self.parse_each_statement();
        }

        // Loop statement
        if self.check_token(Token::Loop) {
            return self.parse_loop_statement();
        }

        // Break
        if self.check_token(Token::Break) {
            let span = self.current_span();
            self.advance();
            return Ok(Statement::Expression(Expr::Break(span)));
        }

        // Detect `$var = expr` assignment BEFORE expression parsing.
        // We look ahead: if current is a variable token and the next token is `=`,
        // parse it as an assignment directly.
        if self.is_variable_token() && self.peek_token_at(1) == Some(Token::Eq) {
            let var = self.parse_variable()?;
            let span = var.span.clone();
            self.advance(); // consume `=`
            let value = self.parse_expr()?;
            return Ok(Statement::Assignment(Assignment {
                target: var,
                value: Box::new(value),
                span,
            }));
        }

        // Try to parse as expression, then check for assignment / pipeline / each
        let expr = self.parse_expr()?;

        // Check for pipeline: `expr | ...`
        if self.check_token(Token::Pipe) {
            let first_span = self.expr_span(&expr);
            let pipeline_result = self.parse_pipeline_tail(expr)?;
            // pipeline_result is either a Statement::Pipeline, Statement::Each,
            // Statement::Expression(Match), or similar
            return self.check_arrow_assignment(pipeline_result, first_span);
        }

        // Check for `-> $var` (arrow assignment)
        if self.check_token(Token::Arrow) {
            self.advance(); // consume `->`
            let target = self.parse_variable()?;
            let span = self.expr_span(&expr);
            return Ok(Statement::Assignment(Assignment {
                target,
                value: Box::new(expr),
                span,
            }));
        }

        Ok(Statement::Expression(expr))
    }

    /// After parsing a pipeline, check for `-> $var` arrow assignment.
    fn check_arrow_assignment(
        &mut self,
        stmt: Statement,
        span: Span,
    ) -> Result<Statement, ParseError> {
        if self.check_token(Token::Arrow) {
            self.advance(); // consume `->`
            let target = self.parse_variable()?;
            let expr = match stmt {
                Statement::Pipeline(p) => Expr::Pipeline(p),
                Statement::Expression(e) => e,
                other => {
                    return Err(self.error(format!(
                        "cannot use -> assignment after {:?}",
                        other
                    )));
                }
            };
            Ok(Statement::Assignment(Assignment {
                target,
                value: Box::new(expr),
                span,
            }))
        } else {
            Ok(stmt)
        }
    }

    // ── pipeline tail: already have first expr, consume `| expr ...` ─

    fn parse_pipeline_tail(&mut self, first: Expr) -> Result<Statement, ParseError> {
        let mut stages = vec![first];
        let span = self.expr_span(&stages[0]);

        while self.check_token(Token::Pipe) {
            self.advance(); // consume `|`

            // Check for `each` in pipeline context
            if self.check_token(Token::Each) {
                return self.parse_pipeline_each(stages);
            }

            // Check for `match` in pipeline context
            if self.check_token(Token::Match) {
                let match_expr = self.parse_match_expr()?;
                // The match consumes the rest, build pipeline up to here + match
                if stages.len() == 1 {
                    // `expr | match` - the match itself is the statement
                    // But we need to represent the pipeline: source | match
                    let pipeline = Pipeline {
                        stages: vec![stages.remove(0), match_expr],
                        span,
                    };
                    return Ok(Statement::Pipeline(pipeline));
                } else {
                    stages.push(match_expr);
                    let pipeline = Pipeline { stages, span };
                    return Ok(Statement::Pipeline(pipeline));
                }
            }

            let expr = self.parse_pipe_stage_expr()?;
            stages.push(expr);
        }

        if stages.len() == 1 {
            Ok(Statement::Expression(stages.remove(0)))
        } else {
            Ok(Statement::Pipeline(Pipeline { stages, span }))
        }
    }

    // ── each statement ──────────────────────────────────────────────

    fn parse_each_statement(&mut self) -> Result<Statement, ParseError> {
        let span = self.current_span();
        self.advance(); // consume `each`

        let binding = self.parse_variable()?;

        let sequential = if self.check_token(Token::Sequential) {
            self.advance();
            true
        } else {
            false
        };

        self.expect_newline()?;
        self.skip_newlines();

        let body = if self.check_lex(LexToken::Indent) {
            self.advance();
            self.skip_newlines();
            self.parse_body()?
        } else {
            Vec::new()
        };

        Ok(Statement::Each(Each {
            iterable: Box::new(Expr::Variable(Variable {
                sigil: Sigil::Scalar,
                name: "_pipe".to_string(),
                span: span.clone(),
            })),
            binding,
            sequential,
            body,
            span,
        }))
    }

    /// Parse `each` in pipeline context: `expr | each $var [sequential] INDENT body DEDENT`
    fn parse_pipeline_each(&mut self, stages: Vec<Expr>) -> Result<Statement, ParseError> {
        let span = self.current_span();
        self.advance(); // consume `each`

        let binding = self.parse_variable()?;

        let sequential = if self.check_token(Token::Sequential) {
            self.advance();
            true
        } else {
            false
        };

        self.expect_newline()?;
        self.skip_newlines();

        let body = if self.check_lex(LexToken::Indent) {
            self.advance();
            self.skip_newlines();
            self.parse_body()?
        } else {
            Vec::new()
        };

        // Build the iterable from the pipeline stages
        let iterable = if stages.len() == 1 {
            stages.into_iter().next().unwrap()
        } else {
            let pipe_span = self.expr_span(&stages[0]);
            Expr::Pipeline(Pipeline {
                stages,
                span: pipe_span,
            })
        };

        Ok(Statement::Each(Each {
            iterable: Box::new(iterable),
            binding,
            sequential,
            body,
            span,
        }))
    }

    // ── loop statement ──────────────────────────────────────────────

    fn parse_loop_statement(&mut self) -> Result<Statement, ParseError> {
        let span = self.current_span();
        self.advance(); // consume `loop`

        // Optional condition (anything before newline)
        let condition = if !self.at_newline() && !self.check_lex(LexToken::Indent) {
            Some(Box::new(self.parse_expr()?))
        } else {
            None
        };

        self.expect_newline()?;
        self.skip_newlines();

        let body = if self.check_lex(LexToken::Indent) {
            self.advance();
            self.skip_newlines();
            self.parse_body()?
        } else {
            Vec::new()
        };

        Ok(Statement::Loop(Loop {
            condition,
            body,
            span,
        }))
    }

    // ── match expression ────────────────────────────────────────────

    fn parse_match_expr(&mut self) -> Result<Expr, ParseError> {
        let span = self.current_span();
        self.advance(); // consume `match`

        self.expect_newline()?;
        self.skip_newlines();

        let mut arms = Vec::new();

        if self.check_lex(LexToken::Indent) {
            self.advance();
            self.skip_newlines();

            loop {
                self.skip_newlines();
                if self.at_eof() || self.check_lex(LexToken::Dedent) {
                    break;
                }
                let arm = self.parse_match_arm()?;
                arms.push(arm);
                self.skip_newlines();
            }

            if self.check_lex(LexToken::Dedent) {
                self.advance();
            }
        }

        Ok(Expr::Match(Match { arms, span }))
    }

    fn parse_match_arm(&mut self) -> Result<MatchArm, ParseError> {
        let span = self.current_span();
        let pattern = self.parse_pattern()?;

        self.expect_token(Token::Arrow)?;

        let body = self.parse_expr()?;

        Ok(MatchArm {
            pattern,
            body: Box::new(body),
            span,
        })
    }

    // ── pattern ─────────────────────────────────────────────────────

    fn parse_pattern(&mut self) -> Result<Pattern, ParseError> {
        let mut patterns = vec![self.parse_single_pattern()?];

        // Check for `&` (AND pattern)
        while self.check_token(Token::Ampersand) {
            self.advance();
            patterns.push(self.parse_single_pattern()?);
        }

        // Check for captures: pattern followed by variable(s) before `->`
        let mut captures = Vec::new();
        while self.is_variable_token() && !self.check_token(Token::Arrow) {
            captures.push(self.parse_variable()?);
        }

        let pattern = if patterns.len() == 1 {
            patterns.remove(0)
        } else {
            Pattern::And(patterns)
        };

        if captures.is_empty() {
            Ok(pattern)
        } else {
            Ok(Pattern::WithCaptures(Box::new(pattern), captures))
        }
    }

    fn parse_single_pattern(&mut self) -> Result<Pattern, ParseError> {
        // Wildcard: `_`
        if self.check_token(Token::Ident) && self.current_text() == "_" {
            self.advance();
            return Ok(Pattern::Wildcard);
        }

        // Field match: `.field = "value"` or `.field >= 21`
        if self.check_token(Token::Dot) {
            return self.parse_field_match_pattern();
        }

        // Comparison pattern: `>= 21`, `> 100`, etc.
        if let Some(op) = self.try_comparison_op() {
            self.advance(); // consume operator
            let expr = self.parse_primary_expr()?;
            return Ok(Pattern::Comparison(op, Box::new(expr)));
        }

        // Variable pattern
        if self.is_variable_token() {
            let var = self.parse_variable()?;
            return Ok(Pattern::Variable(var));
        }

        // Literal pattern
        let lit = self.parse_literal()?;
        Ok(Pattern::Literal(lit))
    }

    fn parse_field_match_pattern(&mut self) -> Result<Pattern, ParseError> {
        self.advance(); // consume `.`
        let field = if self.check_token(Token::Ident) {
            let f = self.current_text().to_string();
            self.advance();
            f
        } else {
            return Err(self.error("expected field name after '.'".to_string()));
        };

        let op = if let Some(op) = self.try_comparison_op() {
            self.advance();
            op
        } else if self.check_token(Token::Eq) {
            self.advance();
            ComparisonOp::Eq
        } else {
            return Err(self.error("expected comparison operator in field match".to_string()));
        };

        let value = self.parse_primary_expr()?;

        Ok(Pattern::FieldMatch(FieldMatch {
            field,
            op,
            value: Box::new(value),
        }))
    }

    fn try_comparison_op(&self) -> Option<ComparisonOp> {
        match self.current_token() {
            Some(Token::GtEq) => Some(ComparisonOp::GtEq),
            Some(Token::LtEq) => Some(ComparisonOp::LtEq),
            Some(Token::Gt) => Some(ComparisonOp::Gt),
            Some(Token::Lt) => Some(ComparisonOp::Lt),
            Some(Token::NotEq) => Some(ComparisonOp::NotEq),
            _ => None,
        }
    }

    // ── expression parsing ──────────────────────────────────────────

    fn parse_expr(&mut self) -> Result<Expr, ParseError> {
        self.parse_ternary()
    }

    fn parse_ternary(&mut self) -> Result<Expr, ParseError> {
        let expr = self.parse_comparison()?;

        if self.check_token(Token::Question) {
            let span = self.expr_span(&expr);
            self.advance(); // consume `?`
            let then_expr = self.parse_comparison()?;
            self.expect_token(Token::Colon)?;
            let else_expr = self.parse_comparison()?;
            Ok(Expr::Ternary(Ternary {
                condition: Box::new(expr),
                then_expr: Box::new(then_expr),
                else_expr: Box::new(else_expr),
                span,
            }))
        } else {
            Ok(expr)
        }
    }

    fn parse_comparison(&mut self) -> Result<Expr, ParseError> {
        let left = self.parse_additive()?;

        if let Some(op) = self.try_binop_comparison() {
            let span = self.expr_span(&left);
            self.advance();
            let right = self.parse_additive()?;
            Ok(Expr::BinOp(BinOp {
                left: Box::new(left),
                op,
                right: Box::new(right),
                span,
            }))
        } else {
            Ok(left)
        }
    }

    fn try_binop_comparison(&self) -> Option<BinOpKind> {
        match self.current_token() {
            Some(Token::Gt) => Some(BinOpKind::Gt),
            Some(Token::Lt) => Some(BinOpKind::Lt),
            Some(Token::GtEq) => Some(BinOpKind::GtEq),
            Some(Token::LtEq) => Some(BinOpKind::LtEq),
            Some(Token::NotEq) => Some(BinOpKind::NotEq),
            // Note: Token::Eq (`=`) is assignment, not comparison.
            // Equality comparison in TORQ uses `=` only in pattern contexts.
            _ => None,
        }
    }

    fn parse_additive(&mut self) -> Result<Expr, ParseError> {
        let mut left = self.parse_multiplicative()?;

        loop {
            let op = match self.current_token() {
                Some(Token::Plus) => BinOpKind::Add,
                Some(Token::Minus) => BinOpKind::Sub,
                _ => break,
            };
            let span = self.expr_span(&left);
            self.advance();
            let right = self.parse_multiplicative()?;
            left = Expr::BinOp(BinOp {
                left: Box::new(left),
                op,
                right: Box::new(right),
                span,
            });
        }

        Ok(left)
    }

    fn parse_multiplicative(&mut self) -> Result<Expr, ParseError> {
        let mut left = self.parse_power()?;

        loop {
            let op = match self.current_token() {
                Some(Token::Star) => BinOpKind::Mul,
                Some(Token::Slash) => BinOpKind::Div,
                Some(Token::Percent) => {
                    // Only if not followed by LBrace (that would be %{ dict })
                    // and not followed by an identifier (that would be %var in pattern context)
                    if self.peek_token_at(1) == Some(Token::LBrace) {
                        break;
                    }
                    BinOpKind::Mod
                }
                _ => break,
            };
            let span = self.expr_span(&left);
            self.advance();
            let right = self.parse_power()?;
            left = Expr::BinOp(BinOp {
                left: Box::new(left),
                op,
                right: Box::new(right),
                span,
            });
        }

        Ok(left)
    }

    fn parse_power(&mut self) -> Result<Expr, ParseError> {
        let left = self.parse_call_or_primary()?;

        if self.check_token(Token::Power) {
            let span = self.expr_span(&left);
            self.advance();
            let right = self.parse_power()?; // right-associative
            Ok(Expr::BinOp(BinOp {
                left: Box::new(left),
                op: BinOpKind::Pow,
                right: Box::new(right),
                span,
            }))
        } else {
            Ok(left)
        }
    }

    // ── call / primary ──────────────────────────────────────────────

    fn parse_call_or_primary(&mut self) -> Result<Expr, ParseError> {
        // Block call: `::name args...`
        if self.check_token(Token::BlockName) {
            return self.parse_block_call();
        }

        // Function call: `ident args...` or keyword-as-function
        if self.check_token(Token::Ident) || self.is_callable_keyword() {
            return self.parse_function_call_or_ident();
        }

        self.parse_primary_expr()
    }

    fn parse_block_call(&mut self) -> Result<Expr, ParseError> {
        let span = self.current_span();
        let text = self.current_text().to_string();
        let name = text.strip_prefix("::").unwrap_or(&text).to_string();
        self.advance();

        let args = self.parse_call_args()?;

        Ok(Expr::BlockCall(BlockCall { name, args, span }))
    }

    fn parse_function_call_or_ident(&mut self) -> Result<Expr, ParseError> {
        let span = self.current_span();
        let name = self.current_text().to_string();
        self.advance();

        // Check if this looks like a function call (has arguments on the same line)
        if self.has_call_args() {
            let args = self.parse_call_args()?;
            if args.is_empty() {
                // Bare identifier, not a call
                return Ok(Expr::Variable(Variable {
                    sigil: Sigil::Scalar,
                    name,
                    span,
                }));
            }
            Ok(Expr::Call(Call { name, args, span }))
        } else {
            // Bare identifier — treat as a Call with no args for known functions,
            // or as a variable-like reference.
            // In TORQ, bare identifiers in expression position are function calls.
            Ok(Expr::Call(Call {
                name,
                args: Vec::new(),
                span,
            }))
        }
    }

    fn is_callable_keyword(&self) -> bool {
        matches!(
            self.current_token(),
            Some(
                Token::Range
                    | Token::Respond
                    | Token::Retry
                    | Token::Fail
                    | Token::Delay
                    | Token::Redirect
                    | Token::Validate
                    | Token::Template
                    | Token::Timeout
                    | Token::Rollback
                    | Token::Json
                    | Token::Xml
                    | Token::Yaml
                    | Token::Csv
                    | Token::Toml
                    | Token::Data
            )
        )
    }

    /// Check if there are arguments following a function name on the same line.
    fn has_call_args(&self) -> bool {
        if self.at_newline() || self.at_eof() || self.check_lex(LexToken::Indent) || self.check_lex(LexToken::Dedent) {
            return false;
        }
        // These tokens indicate "something follows" that could be an argument
        matches!(
            self.current_token(),
            Some(
                Token::ScalarVar
                    | Token::ArrayVar
                    | Token::DictVar
                    | Token::ErrorVar
                    | Token::SharedVar
                    | Token::RegexVar
                    | Token::BlockRef
                    | Token::Int
                    | Token::Float
                    | Token::StringLit
                    | Token::True
                    | Token::False
                    | Token::Null
                    | Token::DurationMs
                    | Token::DurationS
                    | Token::DurationM
                    | Token::DurationH
                    | Token::DurationD
                    | Token::LBracket
                    | Token::LBrace
                    | Token::LParen
                    | Token::BlockName
                    | Token::Percent
            )
        )
    }

    /// Parse arguments for a function or block call.
    /// Arguments continue until we hit something that ends the arg list.
    fn parse_call_args(&mut self) -> Result<Vec<Expr>, ParseError> {
        let mut args = Vec::new();

        while self.has_call_args() {
            let arg = self.parse_primary_expr()?;
            args.push(arg);
        }

        Ok(args)
    }

    // ── pipe stage expression ───────────────────────────────────────

    /// Parse an expression in pipe stage context.
    /// In pipe context, `.field` is a member access on the implicit pipe input,
    /// and function calls can include inline binary ops like `filter .age > 18`.
    fn parse_pipe_stage_expr(&mut self) -> Result<Expr, ParseError> {
        // A pipe stage is typically a call expression: `filter .age > 18`, `map .name`, `print`
        if self.check_token(Token::Ident) || self.is_callable_keyword() {
            let span = self.current_span();
            let name = self.current_text().to_string();
            self.advance();

            // Collect args — but in pipe context we parse more aggressively
            let mut args = Vec::new();
            while self.has_pipe_stage_args() {
                let arg = self.parse_pipe_arg()?;
                args.push(arg);
            }

            if args.is_empty() {
                Ok(Expr::Call(Call {
                    name,
                    args: Vec::new(),
                    span,
                }))
            } else {
                Ok(Expr::Call(Call { name, args, span }))
            }
        } else if self.check_token(Token::Dot) {
            // Bare member access in pipe: `| .field`
            self.parse_dot_access()
        } else {
            self.parse_primary_expr()
        }
    }

    fn has_pipe_stage_args(&self) -> bool {
        if self.at_newline() || self.at_eof()
            || self.check_token(Token::Pipe)
            || self.check_token(Token::Arrow)
            || self.check_lex(LexToken::Indent)
            || self.check_lex(LexToken::Dedent)
        {
            return false;
        }
        matches!(
            self.current_token(),
            Some(
                Token::ScalarVar
                    | Token::ArrayVar
                    | Token::DictVar
                    | Token::ErrorVar
                    | Token::SharedVar
                    | Token::RegexVar
                    | Token::Int
                    | Token::Float
                    | Token::StringLit
                    | Token::True
                    | Token::False
                    | Token::Null
                    | Token::DurationMs
                    | Token::DurationS
                    | Token::DurationM
                    | Token::DurationH
                    | Token::DurationD
                    | Token::LBracket
                    | Token::LBrace
                    | Token::LParen
                    | Token::Dot
                    | Token::Gt
                    | Token::Lt
                    | Token::GtEq
                    | Token::LtEq
                    | Token::NotEq
                    | Token::Percent
                    | Token::Plus
                    | Token::Minus
                    | Token::Star
                    | Token::Slash
            )
        )
    }

    fn parse_pipe_arg(&mut self) -> Result<Expr, ParseError> {
        if self.check_token(Token::Dot) {
            self.parse_dot_access()
        } else {
            self.parse_primary_expr()
        }
    }

    fn parse_dot_access(&mut self) -> Result<Expr, ParseError> {
        let span = self.current_span();
        self.advance(); // consume `.`

        let field = if self.check_token(Token::Ident) {
            let f = self.current_text().to_string();
            self.advance();
            f
        } else {
            return Err(self.error("expected field name after '.'".to_string()));
        };

        Ok(Expr::MemberAccess(MemberAccess {
            object: Box::new(Expr::Variable(Variable {
                sigil: Sigil::Scalar,
                name: "_".to_string(),
                span: span.clone(),
            })),
            field,
            span,
        }))
    }

    // ── primary expression ──────────────────────────────────────────

    fn parse_primary_expr(&mut self) -> Result<Expr, ParseError> {
        // Group: `(expr)`
        if self.check_token(Token::LParen) {
            return self.parse_group();
        }

        // Array: `[...]`
        if self.check_token(Token::LBracket) {
            return self.parse_array();
        }

        // Dict: `{ key "value" ... }`
        if self.check_token(Token::LBrace) {
            return self.parse_dict();
        }

        // Dict with percent sigil: `%{ key "value" ... }`
        if self.check_token(Token::Percent) && self.peek_token_at(1) == Some(Token::LBrace) {
            return self.parse_dict_percent();
        }

        // Block reference: `&::name`
        if self.check_token(Token::BlockRef) {
            let span = self.current_span();
            let text = self.current_text().to_string();
            let name = text.strip_prefix("&::").unwrap_or(&text).to_string();
            self.advance();
            return Ok(Expr::BlockRef(name, span));
        }

        // Variables
        if self.is_variable_token() {
            let var = self.parse_variable()?;
            return Ok(Expr::Variable(var));
        }

        // Literals
        if self.is_literal_token() {
            let lit = self.parse_literal()?;
            // Check for string interpolation
            if let Literal::String(ref s, ref span) = lit {
                if s.contains('$') {
                    let parts = self.parse_string_interpolation_parts(s);
                    if parts.iter().any(|p| matches!(p, StringPart::Interpolation(_))) {
                        return Ok(Expr::StringInterp(parts, span.clone()));
                    }
                }
            }
            return Ok(Expr::Literal(lit));
        }

        // Boolean / null keywords
        if self.check_token(Token::True) {
            let span = self.current_span();
            self.advance();
            return Ok(Expr::Literal(Literal::Bool(true, span)));
        }
        if self.check_token(Token::False) {
            let span = self.current_span();
            self.advance();
            return Ok(Expr::Literal(Literal::Bool(false, span)));
        }
        if self.check_token(Token::Null) {
            let span = self.current_span();
            self.advance();
            return Ok(Expr::Literal(Literal::Null(span)));
        }

        // Negative number: `-42` or `-19.99`
        if self.check_token(Token::Minus) {
            if let Some(Token::Int) | Some(Token::Float) = self.peek_token_at(1) {
                let span = self.current_span();
                self.advance(); // consume `-`
                let lit = self.parse_literal()?;
                return match lit {
                    Literal::Int(v, _) => Ok(Expr::Literal(Literal::Int(-v, span))),
                    Literal::Float(v, _) => Ok(Expr::Literal(Literal::Float(-v, span))),
                    _ => unreachable!(),
                };
            }
        }

        Err(self.error(format!(
            "unexpected token in expression: {:?} (text: {:?})",
            self.current_lex_token(),
            self.current_text()
        )))
    }

    fn parse_group(&mut self) -> Result<Expr, ParseError> {
        let span = self.current_span();
        self.advance(); // consume `(`
        let expr = self.parse_expr()?;
        self.expect_token(Token::RParen)?;
        Ok(Expr::Group(Box::new(expr), span))
    }

    fn parse_array(&mut self) -> Result<Expr, ParseError> {
        let span = self.current_span();
        self.advance(); // consume `[`
        let mut elements = Vec::new();

        while !self.check_token(Token::RBracket) && !self.at_eof() {
            self.skip_newlines();
            if self.check_token(Token::RBracket) {
                break;
            }
            let elem = self.parse_expr()?;
            elements.push(elem);
            self.skip_newlines();
        }

        self.expect_token(Token::RBracket)?;
        Ok(Expr::Array(elements, span))
    }

    fn parse_dict(&mut self) -> Result<Expr, ParseError> {
        let span = self.current_span();
        self.advance(); // consume `{`
        let entries = self.parse_dict_entries()?;
        self.expect_token(Token::RBrace)?;
        Ok(Expr::Dict(entries, span))
    }

    fn parse_dict_percent(&mut self) -> Result<Expr, ParseError> {
        let span = self.current_span();
        self.advance(); // consume `%`
        self.advance(); // consume `{`
        let entries = self.parse_dict_entries()?;
        self.expect_token(Token::RBrace)?;
        Ok(Expr::Dict(entries, span))
    }

    fn parse_dict_entries(&mut self) -> Result<Vec<DictEntry>, ParseError> {
        let mut entries = Vec::new();

        loop {
            self.skip_newlines();
            if self.check_token(Token::RBrace) || self.at_eof() {
                break;
            }

            let entry_span = self.current_span();
            // Key is an identifier or a string
            let key = if self.check_token(Token::Ident) {
                let k = self.current_text().to_string();
                self.advance();
                k
            } else if self.check_token(Token::StringLit) {
                let k = self.extract_string_content();
                self.advance();
                k
            } else if self.is_keyword_usable_as_key() {
                let k = self.current_text().to_string();
                self.advance();
                k
            } else {
                return Err(self.error(format!(
                    "expected dict key (identifier or string), found {:?}",
                    self.current_lex_token()
                )));
            };

            let value = self.parse_expr()?;

            entries.push(DictEntry {
                key,
                value,
                span: entry_span,
            });

            self.skip_newlines();
        }

        Ok(entries)
    }

    fn is_keyword_usable_as_key(&self) -> bool {
        // Many keywords can be used as dict keys in TORQ
        matches!(
            self.current_token(),
            Some(
                Token::Data
                    | Token::Json
                    | Token::Xml
                    | Token::Yaml
                    | Token::Csv
                    | Token::Toml
                    | Token::Template
                    | Token::Validate
                    | Token::Required
                    | Token::Timeout
                    | Token::Redirect
                    | Token::Retry
                    | Token::Delay
                    | Token::Backoff
                    | Token::Binary
                    | Token::Recursive
                    | Token::Respond
                    | Token::Range
                    | Token::As
                    | Token::To
                    | Token::Where
                    | Token::By
                    | Token::Desc
            )
        )
    }

    // ── variable parsing ────────────────────────────────────────────

    fn parse_variable(&mut self) -> Result<Variable, ParseError> {
        let span = self.current_span();
        let (sigil, name) = self.parse_variable_parts()?;
        Ok(Variable { sigil, name, span })
    }

    fn parse_variable_parts(&mut self) -> Result<(Sigil, String), ParseError> {
        let text = self.current_text().to_string();
        match self.current_token() {
            Some(Token::ScalarVar) => {
                self.advance();
                Ok((Sigil::Scalar, text[1..].to_string()))
            }
            Some(Token::ArrayVar) => {
                self.advance();
                Ok((Sigil::Array, text[1..].to_string()))
            }
            Some(Token::DictVar) => {
                self.advance();
                Ok((Sigil::Dict, text[1..].to_string()))
            }
            Some(Token::ErrorVar) => {
                self.advance();
                Ok((Sigil::Error, text[1..].to_string()))
            }
            Some(Token::SharedVar) => {
                self.advance();
                Ok((Sigil::Shared, text[1..].to_string()))
            }
            Some(Token::RegexVar) => {
                self.advance();
                Ok((Sigil::Regex, text[1..].to_string()))
            }
            Some(Token::BlockRef) => {
                self.advance();
                // &::name -> strip "&::"
                Ok((Sigil::BlockRef, text[3..].to_string()))
            }
            _ => Err(self.error(format!(
                "expected variable, found {:?}",
                self.current_lex_token()
            ))),
        }
    }

    fn is_variable_token(&self) -> bool {
        matches!(
            self.current_token(),
            Some(
                Token::ScalarVar
                    | Token::ArrayVar
                    | Token::DictVar
                    | Token::ErrorVar
                    | Token::SharedVar
                    | Token::RegexVar
            )
        )
    }

    // ── literal parsing ─────────────────────────────────────────────

    fn parse_literal(&mut self) -> Result<Literal, ParseError> {
        let span = self.current_span();
        let text = self.current_text().to_string();

        match self.current_token() {
            Some(Token::Int) => {
                self.advance();
                let value: i64 = text.parse().map_err(|e| ParseError {
                    message: format!("invalid integer: {}", e),
                    line: span.line,
                    col: span.col,
                })?;
                Ok(Literal::Int(value, span))
            }
            Some(Token::Float) => {
                self.advance();
                let value: f64 = text.parse().map_err(|e| ParseError {
                    message: format!("invalid float: {}", e),
                    line: span.line,
                    col: span.col,
                })?;
                Ok(Literal::Float(value, span))
            }
            Some(Token::StringLit) => {
                let content = self.extract_string_content();
                self.advance();
                Ok(Literal::String(content, span))
            }
            Some(Token::True) => {
                self.advance();
                Ok(Literal::Bool(true, span))
            }
            Some(Token::False) => {
                self.advance();
                Ok(Literal::Bool(false, span))
            }
            Some(Token::Null) => {
                self.advance();
                Ok(Literal::Null(span))
            }
            Some(Token::DurationMs) => {
                self.advance();
                let val = self.parse_duration_value(&text, "ms")?;
                Ok(Literal::Duration(
                    Duration {
                        value: val,
                        unit: DurationUnit::Milliseconds,
                    },
                    span,
                ))
            }
            Some(Token::DurationS) => {
                self.advance();
                let val = self.parse_duration_value(&text, "s")?;
                Ok(Literal::Duration(
                    Duration {
                        value: val,
                        unit: DurationUnit::Seconds,
                    },
                    span,
                ))
            }
            Some(Token::DurationM) => {
                self.advance();
                let val = self.parse_duration_value(&text, "m")?;
                Ok(Literal::Duration(
                    Duration {
                        value: val,
                        unit: DurationUnit::Minutes,
                    },
                    span,
                ))
            }
            Some(Token::DurationH) => {
                self.advance();
                let val = self.parse_duration_value(&text, "h")?;
                Ok(Literal::Duration(
                    Duration {
                        value: val,
                        unit: DurationUnit::Hours,
                    },
                    span,
                ))
            }
            Some(Token::DurationD) => {
                self.advance();
                let val = self.parse_duration_value(&text, "d")?;
                Ok(Literal::Duration(
                    Duration {
                        value: val,
                        unit: DurationUnit::Days,
                    },
                    span,
                ))
            }
            _ => Err(self.error(format!(
                "expected literal, found {:?}",
                self.current_lex_token()
            ))),
        }
    }

    fn is_literal_token(&self) -> bool {
        matches!(
            self.current_token(),
            Some(
                Token::Int
                    | Token::Float
                    | Token::StringLit
                    | Token::DurationMs
                    | Token::DurationS
                    | Token::DurationM
                    | Token::DurationH
                    | Token::DurationD
            )
        )
    }

    fn parse_duration_value(&self, text: &str, suffix: &str) -> Result<u64, ParseError> {
        let num_str = text.strip_suffix(suffix).unwrap_or(text);
        num_str.parse::<u64>().map_err(|e| ParseError {
            message: format!("invalid duration value: {}", e),
            line: 0,
            col: 0,
        })
    }

    fn extract_string_content(&self) -> String {
        let text = self.current_text();
        // Strip surrounding quotes
        if text.starts_with('"') && text.ends_with('"') && text.len() >= 2 {
            let inner = &text[1..text.len() - 1];
            // Process escape sequences
            let mut result = String::new();
            let mut chars = inner.chars();
            while let Some(c) = chars.next() {
                if c == '\\' {
                    match chars.next() {
                        Some('n') => result.push('\n'),
                        Some('t') => result.push('\t'),
                        Some('\\') => result.push('\\'),
                        Some('"') => result.push('"'),
                        Some(other) => {
                            result.push('\\');
                            result.push(other);
                        }
                        None => result.push('\\'),
                    }
                } else {
                    result.push(c);
                }
            }
            result
        } else {
            text.to_string()
        }
    }

    // ── string interpolation ────────────────────────────────────────

    fn parse_string_interpolation_parts(&self, s: &str) -> Vec<StringPart> {
        let mut parts = Vec::new();
        let mut current_literal = String::new();
        let mut chars = s.chars().peekable();

        while let Some(c) = chars.next() {
            if c == '$' {
                // Try to read a variable name
                let mut var_name = String::new();
                while let Some(&nc) = chars.peek() {
                    if nc.is_alphanumeric() || nc == '_' || nc == '.' {
                        var_name.push(nc);
                        chars.next();
                    } else {
                        break;
                    }
                }
                if !var_name.is_empty() {
                    if !current_literal.is_empty() {
                        parts.push(StringPart::Literal(
                            std::mem::take(&mut current_literal),
                        ));
                    }
                    parts.push(StringPart::Interpolation(Variable {
                        sigil: Sigil::Scalar,
                        name: var_name,
                        span: Span {
                            file: self.file.clone(),
                            line: 0,
                            col: 0,
                        },
                    }));
                } else {
                    current_literal.push('$');
                }
            } else {
                current_literal.push(c);
            }
        }

        if !current_literal.is_empty() {
            parts.push(StringPart::Literal(current_literal));
        }

        parts
    }

    // ── token helpers ───────────────────────────────────────────────

    fn current(&self) -> &SpannedToken {
        &self.tokens[self.pos.min(self.tokens.len() - 1)]
    }

    fn current_text(&self) -> &str {
        &self.current().text
    }

    fn current_lex_token(&self) -> &LexToken {
        &self.current().token
    }

    fn current_token(&self) -> Option<Token> {
        match &self.current().token {
            LexToken::Token(t) => Some(t.clone()),
            _ => None,
        }
    }

    fn current_span(&self) -> Span {
        let st = self.current();
        Span {
            file: self.file.clone(),
            line: st.line,
            col: st.col,
        }
    }

    fn advance(&mut self) {
        if self.pos < self.tokens.len() - 1 {
            self.pos += 1;
        }
    }

    fn at_eof(&self) -> bool {
        matches!(self.current().token, LexToken::Eof)
    }

    fn at_newline(&self) -> bool {
        matches!(self.current().token, LexToken::Token(Token::Newline))
    }

    fn check_token(&self, token: Token) -> bool {
        self.current_token().as_ref() == Some(&token)
    }

    fn check_lex(&self, lex: LexToken) -> bool {
        self.current().token == lex
    }

    fn peek_token_at(&self, offset: usize) -> Option<Token> {
        let idx = self.pos + offset;
        if idx < self.tokens.len() {
            match &self.tokens[idx].token {
                LexToken::Token(t) => Some(t.clone()),
                _ => None,
            }
        } else {
            None
        }
    }

    fn expect_token(&mut self, expected: Token) -> Result<(), ParseError> {
        if self.check_token(expected.clone()) {
            self.advance();
            Ok(())
        } else {
            Err(self.error(format!(
                "expected {:?}, found {:?}",
                expected,
                self.current_lex_token()
            )))
        }
    }

    fn expect_newline(&mut self) -> Result<(), ParseError> {
        if self.at_newline() {
            self.advance();
            Ok(())
        } else if self.at_eof() || self.check_lex(LexToken::Dedent) {
            // At EOF or dedent, newline is implicit
            Ok(())
        } else {
            Err(self.error(format!(
                "expected newline, found {:?}",
                self.current_lex_token()
            )))
        }
    }

    fn skip_newlines(&mut self) {
        while self.at_newline() {
            self.advance();
        }
    }

    fn error(&self, message: String) -> ParseError {
        let st = self.current();
        ParseError {
            message,
            line: st.line,
            col: st.col,
        }
    }

    fn expr_span(&self, expr: &Expr) -> Span {
        match expr {
            Expr::Literal(lit) => match lit {
                Literal::Int(_, s) => s.clone(),
                Literal::Float(_, s) => s.clone(),
                Literal::String(_, s) => s.clone(),
                Literal::Bool(_, s) => s.clone(),
                Literal::Null(s) => s.clone(),
                Literal::Duration(_, s) => s.clone(),
            },
            Expr::Variable(v) => v.span.clone(),
            Expr::Array(_, s) => s.clone(),
            Expr::Dict(_, s) => s.clone(),
            Expr::BlockCall(bc) => bc.span.clone(),
            Expr::BlockRef(_, s) => s.clone(),
            Expr::Call(c) => c.span.clone(),
            Expr::Match(m) => m.span.clone(),
            Expr::BinOp(b) => b.span.clone(),
            Expr::MemberAccess(m) => m.span.clone(),
            Expr::StringInterp(_, s) => s.clone(),
            Expr::Ternary(t) => t.span.clone(),
            Expr::Pipeline(p) => p.span.clone(),
            Expr::Break(s) => s.clone(),
            Expr::Group(_, s) => s.clone(),
        }
    }
}

// ---------------------------------------------------------------------------
// Convenience function
// ---------------------------------------------------------------------------

pub fn parse(tokens: Vec<SpannedToken>, file: &str) -> Result<Program, ParseError> {
    Parser::new(tokens, file).parse()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use torqc_lexer::lexer::Lexer;

    fn parse_torq(source: &str) -> Program {
        let tokens = Lexer::tokenize(source, "test.torq").expect("lexer failed");
        parse(tokens, "test.torq").expect("parse failed")
    }

    fn parse_torq_err(source: &str) -> ParseError {
        let tokens = Lexer::tokenize(source, "test.torq").expect("lexer failed");
        parse(tokens, "test.torq").expect_err("expected parse error")
    }

    // ── block definitions ───────────────────────────────────────────

    #[test]
    fn empty_block() {
        let prog = parse_torq("::main\n  print");
        assert_eq!(prog.blocks.len(), 1);
        assert_eq!(prog.blocks[0].name, "main");
        assert_eq!(prog.blocks[0].params.len(), 0);
        assert_eq!(prog.blocks[0].body.len(), 1);
    }

    #[test]
    fn block_with_params() {
        let prog = parse_torq("::greet $name @items\n  print $name");
        assert_eq!(prog.blocks[0].name, "greet");
        assert_eq!(prog.blocks[0].params.len(), 2);
        assert_eq!(prog.blocks[0].params[0].sigil, Sigil::Scalar);
        assert_eq!(prog.blocks[0].params[0].name, "name");
        assert_eq!(prog.blocks[0].params[1].sigil, Sigil::Array);
        assert_eq!(prog.blocks[0].params[1].name, "items");
    }

    #[test]
    fn multiple_blocks() {
        let source = "::first\n  print\n::second\n  print";
        let prog = parse_torq(source);
        assert_eq!(prog.blocks.len(), 2);
        assert_eq!(prog.blocks[0].name, "first");
        assert_eq!(prog.blocks[1].name, "second");
    }

    #[test]
    fn block_with_doc_comments() {
        let source = "## This is a doc comment\n## Another line\n::main\n  print";
        let prog = parse_torq(source);
        assert_eq!(prog.blocks[0].doc_comments.len(), 2);
        assert_eq!(prog.blocks[0].doc_comments[0], "This is a doc comment");
        assert_eq!(prog.blocks[0].doc_comments[1], "Another line");
    }

    // ── function calls ──────────────────────────────────────────────

    #[test]
    fn simple_call() {
        let prog = parse_torq("::main\n  print \"hello world\"");
        let stmt = &prog.blocks[0].body[0];
        match stmt {
            Statement::Expression(Expr::Call(call)) => {
                assert_eq!(call.name, "print");
                assert_eq!(call.args.len(), 1);
                match &call.args[0] {
                    Expr::Literal(Literal::String(s, _)) => assert_eq!(s, "hello world"),
                    other => panic!("expected string literal, got {:?}", other),
                }
            }
            other => panic!("expected call expression, got {:?}", other),
        }
    }

    #[test]
    fn call_with_multiple_args() {
        let prog = parse_torq("::main\n  print \"hello\" 42");
        match &prog.blocks[0].body[0] {
            Statement::Expression(Expr::Call(call)) => {
                assert_eq!(call.name, "print");
                assert_eq!(call.args.len(), 2);
            }
            other => panic!("expected call, got {:?}", other),
        }
    }

    #[test]
    fn dotted_function_call() {
        let prog = parse_torq("::main\n  sys.fs.read \"file.txt\"");
        match &prog.blocks[0].body[0] {
            Statement::Expression(Expr::Call(call)) => {
                assert_eq!(call.name, "sys.fs.read");
                assert_eq!(call.args.len(), 1);
            }
            other => panic!("expected call, got {:?}", other),
        }
    }

    // ── variables ───────────────────────────────────────────────────

    #[test]
    fn scalar_variable() {
        let prog = parse_torq("::main\n  print $name");
        match &prog.blocks[0].body[0] {
            Statement::Expression(Expr::Call(call)) => {
                match &call.args[0] {
                    Expr::Variable(v) => {
                        assert_eq!(v.sigil, Sigil::Scalar);
                        assert_eq!(v.name, "name");
                    }
                    other => panic!("expected variable, got {:?}", other),
                }
            }
            other => panic!("expected call, got {:?}", other),
        }
    }

    #[test]
    fn dotted_scalar_variable() {
        let prog = parse_torq("::main\n  print $req.body");
        match &prog.blocks[0].body[0] {
            Statement::Expression(Expr::Call(call)) => {
                match &call.args[0] {
                    Expr::Variable(v) => {
                        assert_eq!(v.sigil, Sigil::Scalar);
                        assert_eq!(v.name, "req.body");
                    }
                    other => panic!("expected variable, got {:?}", other),
                }
            }
            other => panic!("expected call, got {:?}", other),
        }
    }

    // ── assignments ─────────────────────────────────────────────────

    #[test]
    fn equals_assignment() {
        let prog = parse_torq("::main\n  $x = 42");
        match &prog.blocks[0].body[0] {
            Statement::Assignment(a) => {
                assert_eq!(a.target.name, "x");
                assert_eq!(a.target.sigil, Sigil::Scalar);
                match a.value.as_ref() {
                    Expr::Literal(Literal::Int(42, _)) => {}
                    other => panic!("expected int 42, got {:?}", other),
                }
            }
            other => panic!("expected assignment, got {:?}", other),
        }
    }

    #[test]
    fn arrow_assignment() {
        let prog = parse_torq("::main\n  42 -> $x");
        match &prog.blocks[0].body[0] {
            Statement::Assignment(a) => {
                assert_eq!(a.target.name, "x");
                match a.value.as_ref() {
                    Expr::Literal(Literal::Int(42, _)) => {}
                    other => panic!("expected int 42, got {:?}", other),
                }
            }
            other => panic!("expected assignment, got {:?}", other),
        }
    }

    // ── pipelines ───────────────────────────────────────────────────

    #[test]
    fn simple_pipeline() {
        let prog = parse_torq("::main\n  $data | print");
        match &prog.blocks[0].body[0] {
            Statement::Pipeline(p) => {
                assert_eq!(p.stages.len(), 2);
            }
            other => panic!("expected pipeline, got {:?}", other),
        }
    }

    #[test]
    fn pipeline_with_arrow() {
        let prog = parse_torq("::main\n  $data | filter | sort -> $result");
        match &prog.blocks[0].body[0] {
            Statement::Assignment(a) => {
                assert_eq!(a.target.name, "result");
                match a.value.as_ref() {
                    Expr::Pipeline(p) => {
                        assert_eq!(p.stages.len(), 3);
                    }
                    other => panic!("expected pipeline, got {:?}", other),
                }
            }
            other => panic!("expected assignment, got {:?}", other),
        }
    }

    // ── each ────────────────────────────────────────────────────────

    #[test]
    fn each_statement() {
        let prog = parse_torq("::main\n  each $item\n    print $item");
        match &prog.blocks[0].body[0] {
            Statement::Each(each) => {
                assert_eq!(each.binding.name, "item");
                assert!(!each.sequential);
                assert_eq!(each.body.len(), 1);
            }
            other => panic!("expected each, got {:?}", other),
        }
    }

    #[test]
    fn pipeline_each() {
        let prog = parse_torq("::main\n  @items | each $item\n    print $item");
        match &prog.blocks[0].body[0] {
            Statement::Each(each) => {
                assert_eq!(each.binding.name, "item");
                match each.iterable.as_ref() {
                    Expr::Variable(v) => assert_eq!(v.name, "items"),
                    other => panic!("expected variable iterable, got {:?}", other),
                }
            }
            other => panic!("expected each, got {:?}", other),
        }
    }

    #[test]
    fn each_sequential() {
        let prog = parse_torq("::main\n  @items | each $item sequential\n    print $item");
        match &prog.blocks[0].body[0] {
            Statement::Each(each) => {
                assert!(each.sequential);
            }
            other => panic!("expected each, got {:?}", other),
        }
    }

    // ── loop ────────────────────────────────────────────────────────

    #[test]
    fn loop_unconditional() {
        let prog = parse_torq("::main\n  loop\n    print \"tick\"");
        match &prog.blocks[0].body[0] {
            Statement::Loop(lp) => {
                assert!(lp.condition.is_none());
                assert_eq!(lp.body.len(), 1);
            }
            other => panic!("expected loop, got {:?}", other),
        }
    }

    // ── literals ────────────────────────────────────────────────────

    #[test]
    fn integer_literal() {
        let prog = parse_torq("::main\n  print 42");
        match &prog.blocks[0].body[0] {
            Statement::Expression(Expr::Call(call)) => {
                match &call.args[0] {
                    Expr::Literal(Literal::Int(42, _)) => {}
                    other => panic!("expected int 42, got {:?}", other),
                }
            }
            other => panic!("expected call, got {:?}", other),
        }
    }

    #[test]
    fn float_literal() {
        let prog = parse_torq("::main\n  print 19.99");
        match &prog.blocks[0].body[0] {
            Statement::Expression(Expr::Call(call)) => {
                match &call.args[0] {
                    Expr::Literal(Literal::Float(v, _)) => {
                        assert!((v - 19.99).abs() < f64::EPSILON);
                    }
                    other => panic!("expected float, got {:?}", other),
                }
            }
            other => panic!("expected call, got {:?}", other),
        }
    }

    #[test]
    fn boolean_literals() {
        let prog = parse_torq("::main\n  print true");
        match &prog.blocks[0].body[0] {
            Statement::Expression(Expr::Call(call)) => {
                match &call.args[0] {
                    Expr::Literal(Literal::Bool(true, _)) => {}
                    other => panic!("expected true, got {:?}", other),
                }
            }
            other => panic!("expected call, got {:?}", other),
        }
    }

    #[test]
    fn null_literal() {
        let prog = parse_torq("::main\n  print null");
        match &prog.blocks[0].body[0] {
            Statement::Expression(Expr::Call(call)) => {
                match &call.args[0] {
                    Expr::Literal(Literal::Null(_)) => {}
                    other => panic!("expected null, got {:?}", other),
                }
            }
            other => panic!("expected call, got {:?}", other),
        }
    }

    #[test]
    fn duration_literal() {
        let prog = parse_torq("::main\n  print 100ms");
        match &prog.blocks[0].body[0] {
            Statement::Expression(Expr::Call(call)) => {
                match &call.args[0] {
                    Expr::Literal(Literal::Duration(d, _)) => {
                        assert_eq!(d.value, 100);
                        assert_eq!(d.unit, DurationUnit::Milliseconds);
                    }
                    other => panic!("expected duration, got {:?}", other),
                }
            }
            other => panic!("expected call, got {:?}", other),
        }
    }

    #[test]
    fn duration_seconds() {
        let prog = parse_torq("::main\n  print 5s");
        match &prog.blocks[0].body[0] {
            Statement::Expression(Expr::Call(call)) => {
                match &call.args[0] {
                    Expr::Literal(Literal::Duration(d, _)) => {
                        assert_eq!(d.value, 5);
                        assert_eq!(d.unit, DurationUnit::Seconds);
                    }
                    other => panic!("expected duration, got {:?}", other),
                }
            }
            other => panic!("expected call, got {:?}", other),
        }
    }

    // ── arrays ──────────────────────────────────────────────────────

    #[test]
    fn array_literal() {
        let prog = parse_torq("::main\n  print [1 2 3]");
        match &prog.blocks[0].body[0] {
            Statement::Expression(Expr::Call(call)) => {
                match &call.args[0] {
                    Expr::Array(elems, _) => {
                        assert_eq!(elems.len(), 3);
                    }
                    other => panic!("expected array, got {:?}", other),
                }
            }
            other => panic!("expected call, got {:?}", other),
        }
    }

    // ── dicts ───────────────────────────────────────────────────────

    #[test]
    fn dict_literal() {
        let prog = parse_torq("::main\n  print { name \"Alice\" }");
        match &prog.blocks[0].body[0] {
            Statement::Expression(Expr::Call(call)) => {
                match &call.args[0] {
                    Expr::Dict(entries, _) => {
                        assert_eq!(entries.len(), 1);
                        assert_eq!(entries[0].key, "name");
                        match &entries[0].value {
                            Expr::Literal(Literal::String(s, _)) => assert_eq!(s, "Alice"),
                            other => panic!("expected string, got {:?}", other),
                        }
                    }
                    other => panic!("expected dict, got {:?}", other),
                }
            }
            other => panic!("expected call, got {:?}", other),
        }
    }

    #[test]
    fn dict_percent_literal() {
        let prog = parse_torq("::main\n  $x = %{ key \"value\" }");
        match &prog.blocks[0].body[0] {
            Statement::Assignment(a) => {
                match a.value.as_ref() {
                    Expr::Dict(entries, _) => {
                        assert_eq!(entries.len(), 1);
                        assert_eq!(entries[0].key, "key");
                    }
                    other => panic!("expected dict, got {:?}", other),
                }
            }
            other => panic!("expected assignment, got {:?}", other),
        }
    }

    // ── binary operations ───────────────────────────────────────────

    #[test]
    fn binary_add() {
        let prog = parse_torq("::main\n  $x = $a + $b");
        match &prog.blocks[0].body[0] {
            Statement::Assignment(a) => {
                match a.value.as_ref() {
                    Expr::BinOp(op) => {
                        assert_eq!(op.op, BinOpKind::Add);
                    }
                    other => panic!("expected binop, got {:?}", other),
                }
            }
            other => panic!("expected assignment, got {:?}", other),
        }
    }

    #[test]
    fn operator_precedence() {
        // $a + $b * $c should parse as $a + ($b * $c)
        let prog = parse_torq("::main\n  $x = $a + $b * $c");
        match &prog.blocks[0].body[0] {
            Statement::Assignment(a) => {
                match a.value.as_ref() {
                    Expr::BinOp(op) => {
                        assert_eq!(op.op, BinOpKind::Add);
                        match op.right.as_ref() {
                            Expr::BinOp(inner) => {
                                assert_eq!(inner.op, BinOpKind::Mul);
                            }
                            other => panic!("expected binop right, got {:?}", other),
                        }
                    }
                    other => panic!("expected binop, got {:?}", other),
                }
            }
            other => panic!("expected assignment, got {:?}", other),
        }
    }

    // ── group expressions ───────────────────────────────────────────

    #[test]
    fn grouped_expression() {
        let prog = parse_torq("::main\n  $x = ($a + $b)");
        match &prog.blocks[0].body[0] {
            Statement::Assignment(a) => {
                match a.value.as_ref() {
                    Expr::Group(inner, _) => {
                        match inner.as_ref() {
                            Expr::BinOp(op) => {
                                assert_eq!(op.op, BinOpKind::Add);
                            }
                            other => panic!("expected binop inside group, got {:?}", other),
                        }
                    }
                    other => panic!("expected group, got {:?}", other),
                }
            }
            other => panic!("expected assignment, got {:?}", other),
        }
    }

    // ── ternary ─────────────────────────────────────────────────────

    #[test]
    fn ternary_expression() {
        let prog = parse_torq("::main\n  $x = $cond ? $a : $b");
        match &prog.blocks[0].body[0] {
            Statement::Assignment(a) => {
                match a.value.as_ref() {
                    Expr::Ternary(t) => {
                        match t.condition.as_ref() {
                            Expr::Variable(v) => assert_eq!(v.name, "cond"),
                            other => panic!("expected variable condition, got {:?}", other),
                        }
                    }
                    other => panic!("expected ternary, got {:?}", other),
                }
            }
            other => panic!("expected assignment, got {:?}", other),
        }
    }

    // ── block calls ─────────────────────────────────────────────────

    #[test]
    fn block_call() {
        let prog = parse_torq("::main\n  ::greet \"Alice\"");
        match &prog.blocks[0].body[0] {
            Statement::Expression(Expr::BlockCall(bc)) => {
                assert_eq!(bc.name, "greet");
                assert_eq!(bc.args.len(), 1);
            }
            other => panic!("expected block call, got {:?}", other),
        }
    }

    // ── block references ────────────────────────────────────────────

    #[test]
    fn block_ref() {
        let prog = parse_torq("::main\n  print &::handler");
        match &prog.blocks[0].body[0] {
            Statement::Expression(Expr::Call(call)) => {
                match &call.args[0] {
                    Expr::BlockRef(name, _) => assert_eq!(name, "handler"),
                    other => panic!("expected block ref, got {:?}", other),
                }
            }
            other => panic!("expected call, got {:?}", other),
        }
    }

    // ── string interpolation ────────────────────────────────────────

    #[test]
    fn string_interpolation() {
        let prog = parse_torq("::main\n  print \"hello $name\"");
        match &prog.blocks[0].body[0] {
            Statement::Expression(Expr::Call(call)) => {
                match &call.args[0] {
                    Expr::StringInterp(parts, _) => {
                        assert_eq!(parts.len(), 2);
                        match &parts[0] {
                            StringPart::Literal(s) => assert_eq!(s, "hello "),
                            other => panic!("expected literal part, got {:?}", other),
                        }
                        match &parts[1] {
                            StringPart::Interpolation(v) => assert_eq!(v.name, "name"),
                            other => panic!("expected interpolation part, got {:?}", other),
                        }
                    }
                    other => panic!("expected string interp, got {:?}", other),
                }
            }
            other => panic!("expected call, got {:?}", other),
        }
    }

    // ── match expression ────────────────────────────────────────────

    #[test]
    fn match_with_literal_patterns() {
        let source = "::main\n  $x | match\n    1 -> print \"one\"\n    2 -> print \"two\"\n    _ -> print \"other\"";
        let prog = parse_torq(source);
        match &prog.blocks[0].body[0] {
            Statement::Pipeline(p) => {
                assert_eq!(p.stages.len(), 2);
                match &p.stages[1] {
                    Expr::Match(m) => {
                        assert_eq!(m.arms.len(), 3);
                        match &m.arms[2].pattern {
                            Pattern::Wildcard => {}
                            other => panic!("expected wildcard, got {:?}", other),
                        }
                    }
                    other => panic!("expected match, got {:?}", other),
                }
            }
            other => panic!("expected pipeline, got {:?}", other),
        }
    }

    // ── break ───────────────────────────────────────────────────────

    #[test]
    fn break_in_loop() {
        let source = "::main\n  loop\n    break";
        let prog = parse_torq(source);
        match &prog.blocks[0].body[0] {
            Statement::Loop(lp) => {
                match &lp.body[0] {
                    Statement::Expression(Expr::Break(_)) => {}
                    other => panic!("expected break, got {:?}", other),
                }
            }
            other => panic!("expected loop, got {:?}", other),
        }
    }

    // ── keyword as function call ────────────────────────────────────

    #[test]
    fn respond_as_call() {
        let prog = parse_torq("::main\n  respond 200 %data");
        match &prog.blocks[0].body[0] {
            Statement::Expression(Expr::Call(call)) => {
                assert_eq!(call.name, "respond");
                assert_eq!(call.args.len(), 2);
            }
            other => panic!("expected call, got {:?}", other),
        }
    }

    #[test]
    fn range_as_call() {
        let prog = parse_torq("::main\n  range 1 10");
        match &prog.blocks[0].body[0] {
            Statement::Expression(Expr::Call(call)) => {
                assert_eq!(call.name, "range");
                assert_eq!(call.args.len(), 2);
            }
            other => panic!("expected call, got {:?}", other),
        }
    }

    // ── negative numbers ────────────────────────────────────────────

    #[test]
    fn negative_integer() {
        let prog = parse_torq("::main\n  $x = -42");
        match &prog.blocks[0].body[0] {
            Statement::Assignment(a) => {
                match a.value.as_ref() {
                    Expr::Literal(Literal::Int(-42, _)) => {}
                    other => panic!("expected -42, got {:?}", other),
                }
            }
            other => panic!("expected assignment, got {:?}", other),
        }
    }

    // ── error handling ──────────────────────────────────────────────

    #[test]
    fn error_no_block() {
        let err = parse_torq_err("print \"hello\"");
        assert!(err.message.contains("expected block definition"));
    }

    // ── complex multi-statement block ───────────────────────────────

    #[test]
    fn multi_statement_block() {
        let source = "::main\n  $x = 1\n  $y = 2\n  print $x";
        let prog = parse_torq(source);
        assert_eq!(prog.blocks[0].body.len(), 3);
    }

    // ── pattern matching with field matches ─────────────────────────

    #[test]
    fn match_field_pattern() {
        let source = "::main\n  $x | match\n    .method = \"GET\" -> print \"get\"";
        let prog = parse_torq(source);
        match &prog.blocks[0].body[0] {
            Statement::Pipeline(p) => {
                match &p.stages[1] {
                    Expr::Match(m) => {
                        match &m.arms[0].pattern {
                            Pattern::FieldMatch(fm) => {
                                assert_eq!(fm.field, "method");
                                assert_eq!(fm.op, ComparisonOp::Eq);
                            }
                            other => panic!("expected field match, got {:?}", other),
                        }
                    }
                    other => panic!("expected match, got {:?}", other),
                }
            }
            other => panic!("expected pipeline, got {:?}", other),
        }
    }

    #[test]
    fn match_comparison_pattern() {
        let source = "::main\n  $x | match\n    >= 21 -> print \"adult\"";
        let prog = parse_torq(source);
        match &prog.blocks[0].body[0] {
            Statement::Pipeline(p) => {
                match &p.stages[1] {
                    Expr::Match(m) => {
                        match &m.arms[0].pattern {
                            Pattern::Comparison(ComparisonOp::GtEq, _) => {}
                            other => panic!("expected comparison pattern, got {:?}", other),
                        }
                    }
                    other => panic!("expected match, got {:?}", other),
                }
            }
            other => panic!("expected pipeline, got {:?}", other),
        }
    }
}
