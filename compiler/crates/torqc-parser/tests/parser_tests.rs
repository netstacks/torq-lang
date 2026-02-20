use torqc_ast::ast::*;
use torqc_lexer::lexer::Lexer;
use torqc_parser::parser;

// ---------------------------------------------------------------------------
// Helper
// ---------------------------------------------------------------------------

fn parse_torq(source: &str) -> Program {
    let tokens = Lexer::tokenize(source, "test.torq").expect("lexer failed");
    parser::parse(tokens, "test.torq").expect("parse failed")
}

// ---------------------------------------------------------------------------
// 1. Parse hello.torq syntax
// ---------------------------------------------------------------------------

#[test]
fn parse_hello_torq() {
    let source = "\
## Hello World \u{2014} the simplest TORQ program

::main
  print \"hello world\"
";
    let prog = parse_torq(source);

    assert_eq!(prog.blocks.len(), 1);
    assert_eq!(prog.blocks[0].name, "main");
    assert_eq!(prog.blocks[0].params.len(), 0);
    assert_eq!(prog.blocks[0].body.len(), 1);

    // The single statement should be a `print` call with one string arg
    match &prog.blocks[0].body[0] {
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

// ---------------------------------------------------------------------------
// 2. Block with params
// ---------------------------------------------------------------------------

#[test]
fn block_with_params() {
    let source = "\
::greet $name $greeting
  print $name
";
    let prog = parse_torq(source);

    assert_eq!(prog.blocks.len(), 1);
    assert_eq!(prog.blocks[0].name, "greet");
    assert_eq!(prog.blocks[0].params.len(), 2);

    assert_eq!(prog.blocks[0].params[0].sigil, Sigil::Scalar);
    assert_eq!(prog.blocks[0].params[0].name, "name");

    assert_eq!(prog.blocks[0].params[1].sigil, Sigil::Scalar);
    assert_eq!(prog.blocks[0].params[1].name, "greeting");
}

// ---------------------------------------------------------------------------
// 3. Scalar assignment
// ---------------------------------------------------------------------------

#[test]
fn scalar_assignment() {
    let source = "\
::main
  $name = \"alice\"
";
    let prog = parse_torq(source);

    assert_eq!(prog.blocks[0].body.len(), 1);
    match &prog.blocks[0].body[0] {
        Statement::Assignment(a) => {
            assert_eq!(a.target.sigil, Sigil::Scalar);
            assert_eq!(a.target.name, "name");
            match a.value.as_ref() {
                Expr::Literal(Literal::String(s, _)) => assert_eq!(s, "alice"),
                other => panic!("expected string literal, got {:?}", other),
            }
        }
        other => panic!("expected assignment, got {:?}", other),
    }
}

// ---------------------------------------------------------------------------
// 4. Pipeline with 4 stages
// ---------------------------------------------------------------------------

#[test]
fn pipeline_four_stages() {
    let source = "\
::main
  $data | filter | sort | print
";
    let prog = parse_torq(source);

    assert_eq!(prog.blocks[0].body.len(), 1);
    match &prog.blocks[0].body[0] {
        Statement::Pipeline(p) => {
            assert_eq!(p.stages.len(), 4);

            // First stage is the variable $data
            match &p.stages[0] {
                Expr::Variable(v) => {
                    assert_eq!(v.sigil, Sigil::Scalar);
                    assert_eq!(v.name, "data");
                }
                other => panic!("expected variable $data, got {:?}", other),
            }

            // Remaining stages are bare function calls
            for (i, name) in [1, 2, 3].iter().zip(["filter", "sort", "print"]) {
                match &p.stages[*i] {
                    Expr::Call(c) => assert_eq!(c.name, name),
                    other => panic!("expected call '{}', got {:?}", name, other),
                }
            }
        }
        other => panic!("expected pipeline, got {:?}", other),
    }
}

// ---------------------------------------------------------------------------
// 5. Multiple blocks
// ---------------------------------------------------------------------------

#[test]
fn multiple_blocks() {
    let source = "\
::first
  print \"one\"

::second
  print \"two\"
";
    let prog = parse_torq(source);

    assert_eq!(prog.blocks.len(), 2);
    assert_eq!(prog.blocks[0].name, "first");
    assert_eq!(prog.blocks[0].body.len(), 1);
    assert_eq!(prog.blocks[1].name, "second");
    assert_eq!(prog.blocks[1].body.len(), 1);
}

// ---------------------------------------------------------------------------
// 6. Array literal
// ---------------------------------------------------------------------------

#[test]
fn array_literal_five_elements() {
    let source = "\
::main
  @nums = [1 2 3 4 5]
";
    let prog = parse_torq(source);

    assert_eq!(prog.blocks[0].body.len(), 1);
    match &prog.blocks[0].body[0] {
        Statement::Assignment(a) => {
            assert_eq!(a.target.sigil, Sigil::Array);
            assert_eq!(a.target.name, "nums");
            match a.value.as_ref() {
                Expr::Array(elems, _) => {
                    assert_eq!(elems.len(), 5);
                    // Verify each element is the expected integer
                    for (i, elem) in elems.iter().enumerate() {
                        match elem {
                            Expr::Literal(Literal::Int(v, _)) => {
                                assert_eq!(*v, (i as i64) + 1);
                            }
                            other => panic!("expected int at index {}, got {:?}", i, other),
                        }
                    }
                }
                other => panic!("expected array, got {:?}", other),
            }
        }
        other => panic!("expected assignment, got {:?}", other),
    }
}

// ---------------------------------------------------------------------------
// 7. Dict literal
// ---------------------------------------------------------------------------

#[test]
fn dict_literal_two_entries() {
    let source = "\
::main
  %user = { name \"alice\" age 30 }
";
    let prog = parse_torq(source);

    assert_eq!(prog.blocks[0].body.len(), 1);
    match &prog.blocks[0].body[0] {
        Statement::Assignment(a) => {
            assert_eq!(a.target.sigil, Sigil::Dict);
            assert_eq!(a.target.name, "user");
            match a.value.as_ref() {
                Expr::Dict(entries, _) => {
                    assert_eq!(entries.len(), 2);

                    assert_eq!(entries[0].key, "name");
                    match &entries[0].value {
                        Expr::Literal(Literal::String(s, _)) => assert_eq!(s, "alice"),
                        other => panic!("expected string 'alice', got {:?}", other),
                    }

                    assert_eq!(entries[1].key, "age");
                    match &entries[1].value {
                        Expr::Literal(Literal::Int(30, _)) => {}
                        other => panic!("expected int 30, got {:?}", other),
                    }
                }
                other => panic!("expected dict, got {:?}", other),
            }
        }
        other => panic!("expected assignment, got {:?}", other),
    }
}

// ---------------------------------------------------------------------------
// 8. Match expression with comparison arms and wildcard
// ---------------------------------------------------------------------------

#[test]
fn match_expression_with_comparison_and_wildcard() {
    let source = "\
::main
  $age | match
    >= 21 -> print \"adult\"
    >= 13 -> print \"teen\"
    _ -> print \"child\"
";
    let prog = parse_torq(source);

    assert_eq!(prog.blocks[0].body.len(), 1);
    match &prog.blocks[0].body[0] {
        Statement::Pipeline(p) => {
            assert_eq!(p.stages.len(), 2);

            // First stage is $age
            match &p.stages[0] {
                Expr::Variable(v) => assert_eq!(v.name, "age"),
                other => panic!("expected variable $age, got {:?}", other),
            }

            // Second stage is match
            match &p.stages[1] {
                Expr::Match(m) => {
                    assert_eq!(m.arms.len(), 3);

                    // First arm: >= 21
                    match &m.arms[0].pattern {
                        Pattern::Comparison(ComparisonOp::GtEq, val) => match val.as_ref() {
                            Expr::Literal(Literal::Int(21, _)) => {}
                            other => panic!("expected 21, got {:?}", other),
                        },
                        other => panic!("expected >= comparison, got {:?}", other),
                    }

                    // Second arm: >= 13
                    match &m.arms[1].pattern {
                        Pattern::Comparison(ComparisonOp::GtEq, val) => match val.as_ref() {
                            Expr::Literal(Literal::Int(13, _)) => {}
                            other => panic!("expected 13, got {:?}", other),
                        },
                        other => panic!("expected >= comparison, got {:?}", other),
                    }

                    // Third arm: wildcard
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

// ---------------------------------------------------------------------------
// 9. Loop with condition and body
// ---------------------------------------------------------------------------

#[test]
fn loop_with_condition() {
    let source = "\
::main
  $count = 0
  loop $count < 100
    $count = $count + 1
";
    let prog = parse_torq(source);

    // Should have 2 statements: assignment + loop
    assert_eq!(prog.blocks[0].body.len(), 2);

    match &prog.blocks[0].body[1] {
        Statement::Loop(lp) => {
            // Loop has a condition
            assert!(lp.condition.is_some());
            match lp.condition.as_ref().unwrap().as_ref() {
                Expr::BinOp(op) => {
                    assert_eq!(op.op, BinOpKind::Lt);
                    match op.left.as_ref() {
                        Expr::Variable(v) => assert_eq!(v.name, "count"),
                        other => panic!("expected $count, got {:?}", other),
                    }
                    match op.right.as_ref() {
                        Expr::Literal(Literal::Int(100, _)) => {}
                        other => panic!("expected 100, got {:?}", other),
                    }
                }
                other => panic!("expected binop condition, got {:?}", other),
            }

            // Loop has 1 statement in body
            assert_eq!(lp.body.len(), 1);
        }
        other => panic!("expected loop, got {:?}", other),
    }
}

// ---------------------------------------------------------------------------
// 10. Block call with args
// ---------------------------------------------------------------------------

#[test]
fn block_call_with_string_args() {
    let source = "\
::main
  ::greet \"alice\" \"hello\"
";
    let prog = parse_torq(source);

    assert_eq!(prog.blocks[0].body.len(), 1);
    match &prog.blocks[0].body[0] {
        Statement::Expression(Expr::BlockCall(bc)) => {
            assert_eq!(bc.name, "greet");
            assert_eq!(bc.args.len(), 2);
            match &bc.args[0] {
                Expr::Literal(Literal::String(s, _)) => assert_eq!(s, "alice"),
                other => panic!("expected string 'alice', got {:?}", other),
            }
            match &bc.args[1] {
                Expr::Literal(Literal::String(s, _)) => assert_eq!(s, "hello"),
                other => panic!("expected string 'hello', got {:?}", other),
            }
        }
        other => panic!("expected block call, got {:?}", other),
    }
}

// ---------------------------------------------------------------------------
// 11. Arrow assignment from expression
// ---------------------------------------------------------------------------

#[test]
fn arrow_assignment_binop() {
    let source = "\
::main
  $a + $b -> $result
";
    let prog = parse_torq(source);

    assert_eq!(prog.blocks[0].body.len(), 1);
    match &prog.blocks[0].body[0] {
        Statement::Assignment(a) => {
            assert_eq!(a.target.sigil, Sigil::Scalar);
            assert_eq!(a.target.name, "result");
            match a.value.as_ref() {
                Expr::BinOp(op) => {
                    assert_eq!(op.op, BinOpKind::Add);
                    match op.left.as_ref() {
                        Expr::Variable(v) => assert_eq!(v.name, "a"),
                        other => panic!("expected $a, got {:?}", other),
                    }
                    match op.right.as_ref() {
                        Expr::Variable(v) => assert_eq!(v.name, "b"),
                        other => panic!("expected $b, got {:?}", other),
                    }
                }
                other => panic!("expected binop, got {:?}", other),
            }
        }
        other => panic!("expected assignment, got {:?}", other),
    }
}

// ---------------------------------------------------------------------------
// 12. Doc comments attached to the next block
// ---------------------------------------------------------------------------

#[test]
fn doc_comments_attached_to_block() {
    let source = "\
## This block does important work
## It has two doc comment lines
::worker
  print \"working\"
";
    let prog = parse_torq(source);

    assert_eq!(prog.blocks.len(), 1);
    assert_eq!(prog.blocks[0].name, "worker");
    assert_eq!(prog.blocks[0].doc_comments.len(), 2);
    assert_eq!(
        prog.blocks[0].doc_comments[0],
        "This block does important work"
    );
    assert_eq!(
        prog.blocks[0].doc_comments[1],
        "It has two doc comment lines"
    );
}

// ---------------------------------------------------------------------------
// 13. Fibonacci example (full parse)
// ---------------------------------------------------------------------------

#[test]
fn parse_fibonacci_example() {
    let source = "\
## Fibonacci \u{2014} demonstrating recursion and computation

::fibonacci $n
  $n | match
    0 -> 0
    1 -> 1
    _ -> ::fibonacci ($n - 1) + ::fibonacci ($n - 2)

::main
  range 1 20 | each $n sequential
    ::fibonacci $n | print
";
    let prog = parse_torq(source);

    // Two blocks: fibonacci and main
    assert_eq!(prog.blocks.len(), 2);

    // -- fibonacci block --
    let fib = &prog.blocks[0];
    assert_eq!(fib.name, "fibonacci");
    assert_eq!(fib.params.len(), 1);
    assert_eq!(fib.params[0].sigil, Sigil::Scalar);
    assert_eq!(fib.params[0].name, "n");
    assert_eq!(fib.doc_comments.len(), 1);

    // fibonacci body has one statement: $n | match ...
    assert_eq!(fib.body.len(), 1);
    match &fib.body[0] {
        Statement::Pipeline(p) => {
            assert_eq!(p.stages.len(), 2);
            // First stage: $n
            match &p.stages[0] {
                Expr::Variable(v) => assert_eq!(v.name, "n"),
                other => panic!("expected variable $n, got {:?}", other),
            }
            // Second stage: match with 3 arms
            match &p.stages[1] {
                Expr::Match(m) => {
                    assert_eq!(m.arms.len(), 3);
                    // arm 0: literal 0
                    match &m.arms[0].pattern {
                        Pattern::Literal(Literal::Int(0, _)) => {}
                        other => panic!("expected literal 0, got {:?}", other),
                    }
                    // arm 1: literal 1
                    match &m.arms[1].pattern {
                        Pattern::Literal(Literal::Int(1, _)) => {}
                        other => panic!("expected literal 1, got {:?}", other),
                    }
                    // arm 2: wildcard
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

    // -- main block --
    let main_block = &prog.blocks[1];
    assert_eq!(main_block.name, "main");
    assert_eq!(main_block.params.len(), 0);
    // main body has 1 statement: `range 1 20 | each $n sequential`
    assert_eq!(main_block.body.len(), 1);
    match &main_block.body[0] {
        Statement::Each(each) => {
            assert_eq!(each.binding.name, "n");
            assert!(each.sequential);
            // each body has 1 statement
            assert_eq!(each.body.len(), 1);
        }
        other => panic!("expected each statement, got {:?}", other),
    }
}

// ---------------------------------------------------------------------------
// 14. Simplified web server snippet (respond, route calls)
// ---------------------------------------------------------------------------

#[test]
fn parse_web_server_snippet() {
    let source = "\
::health
  respond 200 %{status \"ok\"}

::main
  http.listen 8080 | route \"get\" \"/health\"
";
    let prog = parse_torq(source);

    // Two blocks: health and main
    assert_eq!(prog.blocks.len(), 2);

    // -- health block --
    let health = &prog.blocks[0];
    assert_eq!(health.name, "health");
    assert_eq!(health.params.len(), 0);
    assert_eq!(health.body.len(), 1);
    match &health.body[0] {
        Statement::Expression(Expr::Call(call)) => {
            assert_eq!(call.name, "respond");
            assert_eq!(call.args.len(), 2);
            // First arg: 200
            match &call.args[0] {
                Expr::Literal(Literal::Int(200, _)) => {}
                other => panic!("expected int 200, got {:?}", other),
            }
            // Second arg: dict %{status "ok"}
            match &call.args[1] {
                Expr::Dict(entries, _) => {
                    assert_eq!(entries.len(), 1);
                    assert_eq!(entries[0].key, "status");
                    match &entries[0].value {
                        Expr::Literal(Literal::String(s, _)) => assert_eq!(s, "ok"),
                        other => panic!("expected string 'ok', got {:?}", other),
                    }
                }
                other => panic!("expected dict, got {:?}", other),
            }
        }
        other => panic!("expected call expression, got {:?}", other),
    }

    // -- main block --
    let main_block = &prog.blocks[1];
    assert_eq!(main_block.name, "main");
    assert_eq!(main_block.params.len(), 0);
    // main body: pipeline `http.listen 8080 | route "get" "/health"`
    assert_eq!(main_block.body.len(), 1);
    match &main_block.body[0] {
        Statement::Pipeline(p) => {
            assert_eq!(p.stages.len(), 2);
            // First stage: http.listen call with arg 8080
            match &p.stages[0] {
                Expr::Call(call) => {
                    assert_eq!(call.name, "http.listen");
                    assert_eq!(call.args.len(), 1);
                    match &call.args[0] {
                        Expr::Literal(Literal::Int(8080, _)) => {}
                        other => panic!("expected int 8080, got {:?}", other),
                    }
                }
                other => panic!("expected call 'http.listen', got {:?}", other),
            }
            // Second stage: route call with string args
            match &p.stages[1] {
                Expr::Call(call) => {
                    assert_eq!(call.name, "route");
                    assert_eq!(call.args.len(), 2);
                    match &call.args[0] {
                        Expr::Literal(Literal::String(s, _)) => assert_eq!(s, "get"),
                        other => panic!("expected string 'get', got {:?}", other),
                    }
                    match &call.args[1] {
                        Expr::Literal(Literal::String(s, _)) => assert_eq!(s, "/health"),
                        other => panic!("expected string '/health', got {:?}", other),
                    }
                }
                other => panic!("expected call 'route', got {:?}", other),
            }
        }
        other => panic!("expected pipeline, got {:?}", other),
    }
}
