use torqc_lexer::lexer::{Lexer, LexToken, SpannedToken};
use torqc_lexer::token::Token;

// ─── Helpers ─────────────────────────────────────────────────────────────────

/// Tokenize source and return only the LexToken sequence (no spans).
fn token_types(source: &str) -> Vec<LexToken> {
    Lexer::tokenize(source, "test.torq")
        .expect("tokenize failed")
        .into_iter()
        .map(|st| st.token)
        .collect()
}

/// Tokenize source and return full SpannedToken list.
fn tokenize(source: &str) -> Vec<SpannedToken> {
    Lexer::tokenize(source, "test.torq").expect("tokenize failed")
}

/// Shorthand: wrap a Token in LexToken::Token.
fn t(tok: Token) -> LexToken {
    LexToken::Token(tok)
}

/// Newline shorthand.
fn nl() -> LexToken {
    LexToken::Token(Token::Newline)
}

// ─── 1. Hello.torq example ──────────────────────────────────────────────────

#[test]
fn lex_hello_torq_example() {
    let source = "## Hello World — the simplest TORQ program\n\n::main\n  print \"hello world\"";
    let types = token_types(source);
    assert_eq!(
        types,
        vec![
            // ## Hello World ...
            t(Token::DocComment),
            nl(),
            // ::main
            t(Token::BlockName),
            nl(),
            // indent, print "hello world"
            LexToken::Indent,
            t(Token::Ident), // print
            t(Token::StringLit), // "hello world"
            nl(),
            LexToken::Dedent,
            LexToken::Eof,
        ]
    );
}

// ─── 2. Scalar variable assignments ─────────────────────────────────────────

#[test]
fn lex_scalar_variable_assignment() {
    let source = "$name = \"alice\"";
    let types = token_types(source);
    assert_eq!(
        types,
        vec![
            t(Token::ScalarVar),
            t(Token::Eq),
            t(Token::StringLit),
            nl(),
            LexToken::Eof,
        ]
    );
}

// ─── 3. Array literals ──────────────────────────────────────────────────────

#[test]
fn lex_array_literal() {
    let source = "@numbers = [1 2 3 4 5]";
    let types = token_types(source);
    assert_eq!(
        types,
        vec![
            t(Token::ArrayVar),
            t(Token::Eq),
            t(Token::LBracket),
            t(Token::Int),
            t(Token::Int),
            t(Token::Int),
            t(Token::Int),
            t(Token::Int),
            t(Token::RBracket),
            nl(),
            LexToken::Eof,
        ]
    );
}

// ─── 4. Dict literals ──────────────────────────────────────────────────────

#[test]
fn lex_dict_literal() {
    let source = "%user = { name \"alice\" age 30 }";
    let types = token_types(source);
    assert_eq!(
        types,
        vec![
            t(Token::DictVar),
            t(Token::Eq),
            t(Token::LBrace),
            t(Token::Ident),    // name
            t(Token::StringLit), // "alice"
            t(Token::Ident),    // age
            t(Token::Int),      // 30
            t(Token::RBrace),
            nl(),
            LexToken::Eof,
        ]
    );
}

// ─── 5. Pipe chains ────────────────────────────────────────────────────────

#[test]
fn lex_pipe_chain() {
    let source = "$data | filter | sort | print";
    let types = token_types(source);
    assert_eq!(
        types,
        vec![
            t(Token::ScalarVar), // $data
            t(Token::Pipe),
            t(Token::Ident), // filter
            t(Token::Pipe),
            t(Token::Ident), // sort
            t(Token::Pipe),
            t(Token::Ident), // print
            nl(),
            LexToken::Eof,
        ]
    );
}

// ─── 6. Block definitions with params ───────────────────────────────────────

#[test]
fn lex_block_definition_with_params() {
    let source = "::greet $name $greeting\n  print $greeting";
    let types = token_types(source);
    assert_eq!(
        types,
        vec![
            t(Token::BlockName),  // ::greet
            t(Token::ScalarVar),  // $name
            t(Token::ScalarVar),  // $greeting
            nl(),
            LexToken::Indent,
            t(Token::Ident),      // print
            t(Token::ScalarVar),  // $greeting
            nl(),
            LexToken::Dedent,
            LexToken::Eof,
        ]
    );
}

// ─── 7. Arrow assignments ──────────────────────────────────────────────────

#[test]
fn lex_arrow_assignment() {
    let source = "$result -> $output";
    let types = token_types(source);
    assert_eq!(
        types,
        vec![
            t(Token::ScalarVar), // $result
            t(Token::Arrow),     // ->
            t(Token::ScalarVar), // $output
            nl(),
            LexToken::Eof,
        ]
    );
}

// ─── 8. All keywords in one line ────────────────────────────────────────────

#[test]
fn lex_all_keywords() {
    let source = "each sequential loop match break range true false null as to fail retry respond delay backoff exponential where by desc template validate required binary recursive redirect json xml yaml csv toml data rollback timeout";
    let types = token_types(source);
    assert_eq!(
        types,
        vec![
            t(Token::Each),
            t(Token::Sequential),
            t(Token::Loop),
            t(Token::Match),
            t(Token::Break),
            t(Token::Range),
            t(Token::True),
            t(Token::False),
            t(Token::Null),
            t(Token::As),
            t(Token::To),
            t(Token::Fail),
            t(Token::Retry),
            t(Token::Respond),
            t(Token::Delay),
            t(Token::Backoff),
            t(Token::Exponential),
            t(Token::Where),
            t(Token::By),
            t(Token::Desc),
            t(Token::Template),
            t(Token::Validate),
            t(Token::Required),
            t(Token::Binary),
            t(Token::Recursive),
            t(Token::Redirect),
            t(Token::Json),
            t(Token::Xml),
            t(Token::Yaml),
            t(Token::Csv),
            t(Token::Toml),
            t(Token::Data),
            t(Token::Rollback),
            t(Token::Timeout),
            nl(),
            LexToken::Eof,
        ]
    );
}

// ─── 9. Comparison operators ────────────────────────────────────────────────

#[test]
fn lex_comparison_operators() {
    let source = "$a = $b != $c >= $d <= $e > $f < $g";
    let types = token_types(source);
    assert_eq!(
        types,
        vec![
            t(Token::ScalarVar), // $a
            t(Token::Eq),
            t(Token::ScalarVar), // $b
            t(Token::NotEq),
            t(Token::ScalarVar), // $c
            t(Token::GtEq),
            t(Token::ScalarVar), // $d
            t(Token::LtEq),
            t(Token::ScalarVar), // $e
            t(Token::Gt),
            t(Token::ScalarVar), // $f
            t(Token::Lt),
            t(Token::ScalarVar), // $g
            nl(),
            LexToken::Eof,
        ]
    );
}

// ─── 10. Math operators ────────────────────────────────────────────────────

#[test]
fn lex_math_operators() {
    let source = "$a + $b - $c * $d / $e ** $f";
    let types = token_types(source);
    assert_eq!(
        types,
        vec![
            t(Token::ScalarVar), // $a
            t(Token::Plus),
            t(Token::ScalarVar), // $b
            t(Token::Minus),
            t(Token::ScalarVar), // $c
            t(Token::Star),
            t(Token::ScalarVar), // $d
            t(Token::Slash),
            t(Token::ScalarVar), // $e
            t(Token::Power),
            t(Token::ScalarVar), // $f
            nl(),
            LexToken::Eof,
        ]
    );
}

// ─── 11. Duration literals ─────────────────────────────────────────────────

#[test]
fn lex_duration_literals() {
    let source = "100ms 1s 5m 2h 7d";
    let types = token_types(source);
    assert_eq!(
        types,
        vec![
            t(Token::DurationMs),
            t(Token::DurationS),
            t(Token::DurationM),
            t(Token::DurationH),
            t(Token::DurationD),
            nl(),
            LexToken::Eof,
        ]
    );
}

// ─── 12. Float literals ────────────────────────────────────────────────────

#[test]
fn lex_float_literals() {
    let source = "19.99 3.14 0.5";
    let types = token_types(source);
    assert_eq!(
        types,
        vec![
            t(Token::Float),
            t(Token::Float),
            t(Token::Float),
            nl(),
            LexToken::Eof,
        ]
    );
}

// ─── 13. Block references ──────────────────────────────────────────────────

#[test]
fn lex_block_reference() {
    let source = "&::process";
    let types = token_types(source);
    assert_eq!(
        types,
        vec![
            t(Token::BlockRef),
            nl(),
            LexToken::Eof,
        ]
    );
}

#[test]
fn lex_block_ref_in_context() {
    let source = "$items | each &::process";
    let types = token_types(source);
    assert_eq!(
        types,
        vec![
            t(Token::ScalarVar), // $items
            t(Token::Pipe),
            t(Token::Each),
            t(Token::BlockRef), // &::process
            nl(),
            LexToken::Eof,
        ]
    );
}

// ─── 14. Shared variables ──────────────────────────────────────────────────

#[test]
fn lex_shared_variable_assignment() {
    let source = "*counter = 0";
    let types = token_types(source);
    assert_eq!(
        types,
        vec![
            t(Token::SharedVar), // *counter
            t(Token::Eq),
            t(Token::Int), // 0
            nl(),
            LexToken::Eof,
        ]
    );
}

// ─── 15. Error variables ───────────────────────────────────────────────────

#[test]
fn lex_error_variable_assignment() {
    let source = "!err = \"not found\"";
    let types = token_types(source);
    assert_eq!(
        types,
        vec![
            t(Token::ErrorVar),  // !err
            t(Token::Eq),
            t(Token::StringLit), // "not found"
            nl(),
            LexToken::Eof,
        ]
    );
}

// ─── 16. Doc comments preserved ────────────────────────────────────────────

#[test]
fn lex_doc_comments_preserved() {
    let source = "## This documents the block\n::main\n  print \"hi\"";
    let types = token_types(source);
    assert_eq!(
        types,
        vec![
            t(Token::DocComment),
            nl(),
            t(Token::BlockName), // ::main
            nl(),
            LexToken::Indent,
            t(Token::Ident),     // print
            t(Token::StringLit), // "hi"
            nl(),
            LexToken::Dedent,
            LexToken::Eof,
        ]
    );
}

// ─── 17. Prompt comments stripped ──────────────────────────────────────────

#[test]
fn lex_prompt_comments_stripped() {
    // A line that is entirely a prompt comment is skipped.
    let source = "# this is a comment\nprint \"ok\"";
    let types = token_types(source);
    assert_eq!(
        types,
        vec![
            t(Token::Ident),     // print
            t(Token::StringLit), // "ok"
            nl(),
            LexToken::Eof,
        ]
    );
}

#[test]
fn lex_inline_prompt_comment_stripped() {
    // Inline prompt comment after code is stripped.
    let source = "print \"ok\" # inline comment";
    let types = token_types(source);
    assert_eq!(
        types,
        vec![
            t(Token::Ident),     // print
            t(Token::StringLit), // "ok"
            nl(),
            LexToken::Eof,
        ]
    );
}

// ─── 18. Indentation with nested blocks ────────────────────────────────────

#[test]
fn lex_nested_indentation() {
    let source = "\
::main
  each $item
    match $item
      1 -> print \"one\"
      2 -> print \"two\"";
    let types = token_types(source);
    assert_eq!(
        types,
        vec![
            // ::main
            t(Token::BlockName),
            nl(),
            // indent: each $item
            LexToken::Indent,
            t(Token::Each),
            t(Token::ScalarVar),
            nl(),
            // indent: match $item
            LexToken::Indent,
            t(Token::Match),
            t(Token::ScalarVar),
            nl(),
            // indent: 1 -> print "one"
            LexToken::Indent,
            t(Token::Int),
            t(Token::Arrow),
            t(Token::Ident),
            t(Token::StringLit),
            nl(),
            // 2 -> print "two" (same indent)
            t(Token::Int),
            t(Token::Arrow),
            t(Token::Ident),
            t(Token::StringLit),
            nl(),
            // EOF: three dedents
            LexToken::Dedent,
            LexToken::Dedent,
            LexToken::Dedent,
            LexToken::Eof,
        ]
    );
}

// ─── 19. Member access in variables ─────────────────────────────────────────

#[test]
fn lex_member_access_in_variables() {
    let source = "$req.body";
    let tokens = tokenize(source);
    // Dotted scalar is a single ScalarVar token.
    assert_eq!(tokens[0].token, t(Token::ScalarVar));
    assert_eq!(tokens[0].line, 1);
    assert_eq!(tokens[0].col, 1);
    let types: Vec<LexToken> = tokens.into_iter().map(|st| st.token).collect();
    assert_eq!(
        types,
        vec![
            t(Token::ScalarVar), // $req.body
            nl(),
            LexToken::Eof,
        ]
    );
}

#[test]
fn lex_deep_member_access() {
    let source = "$req.body.data";
    let types = token_types(source);
    assert_eq!(
        types,
        vec![
            t(Token::ScalarVar), // $req.body.data
            nl(),
            LexToken::Eof,
        ]
    );
}

// ─── 20. Dotted function calls ──────────────────────────────────────────────

#[test]
fn lex_dotted_function_calls() {
    let source = "sys.fs.read \"file.txt\"";
    let types = token_types(source);
    assert_eq!(
        types,
        vec![
            t(Token::Ident),     // sys.fs.read
            t(Token::StringLit), // "file.txt"
            nl(),
            LexToken::Eof,
        ]
    );
}

#[test]
fn lex_dotted_ident_single_dot() {
    let source = "http.get";
    let types = token_types(source);
    assert_eq!(
        types,
        vec![
            t(Token::Ident), // http.get
            nl(),
            LexToken::Eof,
        ]
    );
}

// ─── 21. Ternary expression ────────────────────────────────────────────────

#[test]
fn lex_ternary_expression() {
    let source = "$active ? grant : deny";
    let types = token_types(source);
    assert_eq!(
        types,
        vec![
            t(Token::ScalarVar), // $active
            t(Token::Question),  // ?
            t(Token::Ident),     // grant
            t(Token::Colon),     // :
            t(Token::Ident),     // deny
            nl(),
            LexToken::Eof,
        ]
    );
}

// ─── 22. Fibonacci example without error ────────────────────────────────────

#[test]
fn lex_fibonacci_example_no_error() {
    let source = "\
## Fibonacci — demonstrating recursion and computation

::fibonacci $n
  $n | match
    0 -> 0
    1 -> 1
    _ -> ::fibonacci ($n - 1) + ::fibonacci ($n - 2)

::main
  range 1 20 | each $n sequential
    ::fibonacci $n | print";

    let result = Lexer::tokenize(source, "fibonacci.torq");
    assert!(result.is_ok(), "fibonacci.torq lexing failed: {:?}", result.err());

    let tokens = result.unwrap();
    // Should start with DocComment.
    assert_eq!(tokens[0].token, t(Token::DocComment));
    // Should end with Eof.
    assert_eq!(tokens.last().unwrap().token, LexToken::Eof);

    // Verify some key tokens are present.
    let types: Vec<LexToken> = tokens.into_iter().map(|st| st.token).collect();
    assert!(types.contains(&t(Token::BlockName)));   // ::fibonacci, ::main
    assert!(types.contains(&t(Token::Match)));
    assert!(types.contains(&t(Token::Arrow)));
    assert!(types.contains(&t(Token::Range)));
    assert!(types.contains(&t(Token::Each)));
    assert!(types.contains(&t(Token::Sequential)));
    assert!(types.contains(&t(Token::LParen)));
    assert!(types.contains(&t(Token::RParen)));
    assert!(types.contains(&t(Token::Plus)));
    assert!(types.contains(&t(Token::Minus)));
}

// ─── 23. Web server example without error ───────────────────────────────────

#[test]
fn lex_web_server_example_no_error() {
    // Adapted from web_server.torq — uses %var dict syntax instead of %{...}
    // inline dict literals (%{...}) since that syntax is not yet lexer-supported.
    let source = "\
## Simple REST API with TORQ

::health
  respond 200 json

::get_users
  db.query \"select * from users\"
  | respond 200 json

::get_user
  $id = $req.param \"id\"
  db.query \"select * from users where id=$id\"
  | fail -> respond 404 json
  | respond 200 json

::create_user
  $req.body | as data | validate
  | fail -> respond 400 $errors
  | db.insert \"users\"
  | respond 201 json

::delete_user
  $id = $req.param \"id\"
  db.delete \"users\" where $id
  | fail -> respond 404 json
  | respond 200 json

::main
  http.listen 8080
  | route get \"/health\" -> ::health
  | route get \"/users\" -> ::get_users
  | route get \"/users/:id\" -> ::get_user
  | route post \"/users\" -> ::create_user
  | route delete \"/users/:id\" -> ::delete_user";

    let result = Lexer::tokenize(source, "web_server.torq");
    assert!(result.is_ok(), "web_server.torq lexing failed: {:?}", result.err());

    let tokens = result.unwrap();
    // Should start with DocComment.
    assert_eq!(tokens[0].token, t(Token::DocComment));
    // Should end with Eof.
    assert_eq!(tokens.last().unwrap().token, LexToken::Eof);

    // Verify key tokens from the web server example.
    let types: Vec<LexToken> = tokens.into_iter().map(|st| st.token).collect();
    assert!(types.contains(&t(Token::BlockName)));   // ::health, ::get_users, etc.
    assert!(types.contains(&t(Token::Respond)));
    assert!(types.contains(&t(Token::Json)));
    assert!(types.contains(&t(Token::Fail)));
    assert!(types.contains(&t(Token::Arrow)));
    assert!(types.contains(&t(Token::Pipe)));
    assert!(types.contains(&t(Token::As)));
    assert!(types.contains(&t(Token::ScalarVar)));
    assert!(types.contains(&t(Token::Where)));
    assert!(types.contains(&t(Token::Int)));
    assert!(types.contains(&t(Token::StringLit)));
}
