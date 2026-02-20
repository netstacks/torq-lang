use logos::Logos;
use std::fmt;

use crate::token::Token;

/// Extended token type that includes indent/dedent markers.
#[derive(Debug, Clone, PartialEq)]
pub enum LexToken {
    /// A regular token produced by the logos lexer.
    Token(Token),
    /// Indentation increased by one level.
    Indent,
    /// Indentation decreased by one level.
    Dedent,
    /// End of file.
    Eof,
}

/// A token annotated with its source location.
#[derive(Debug, Clone, PartialEq)]
pub struct SpannedToken {
    pub token: LexToken,
    pub line: usize,
    pub col: usize,
}

/// An error encountered during lexing.
#[derive(Debug, Clone)]
pub struct LexError {
    pub file: String,
    pub line: usize,
    pub col: usize,
    pub message: String,
}

impl fmt::Display for LexError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}:{}:{}: {}",
            self.file, self.line, self.col, self.message
        )
    }
}

pub struct Lexer;

impl Lexer {
    /// Tokenize the given source string, producing a sequence of spanned tokens
    /// with indentation tracking.
    ///
    /// The algorithm processes the source line by line, tracks indentation via
    /// an indent stack, strips prompt comments (`#` but not `##`), and emits
    /// `Indent`/`Dedent` tokens at indentation boundaries.
    pub fn tokenize(source: &str, file: &str) -> Result<Vec<SpannedToken>, LexError> {
        let mut tokens: Vec<SpannedToken> = Vec::new();
        let mut indent_stack: Vec<usize> = vec![0];

        for (line_idx, line) in source.lines().enumerate() {
            let line_num = line_idx + 1;

            // Skip blank lines (lines that are empty or contain only whitespace).
            if line.trim().is_empty() {
                continue;
            }

            // Measure leading whitespace (number of space characters).
            let indent = line.len() - line.trim_start().len();
            let content = &line[indent..];

            // Strip prompt comments: lines starting with `#` but not `##`.
            // We check the trimmed content.
            if content.starts_with('#') && !content.starts_with("##") {
                continue;
            }

            // Handle indentation changes.
            let current_indent = *indent_stack.last().unwrap();

            if indent > current_indent {
                // Indentation increased.
                indent_stack.push(indent);
                tokens.push(SpannedToken {
                    token: LexToken::Indent,
                    line: line_num,
                    col: 1,
                });
            } else if indent < current_indent {
                // Indentation decreased — may need multiple Dedent tokens.
                while *indent_stack.last().unwrap() > indent {
                    indent_stack.pop();
                    tokens.push(SpannedToken {
                        token: LexToken::Dedent,
                        line: line_num,
                        col: 1,
                    });
                }

                if *indent_stack.last().unwrap() != indent {
                    return Err(LexError {
                        file: file.to_string(),
                        line: line_num,
                        col: indent + 1,
                        message: format!(
                            "inconsistent indentation: found {} spaces, expected {}",
                            indent,
                            indent_stack.last().unwrap()
                        ),
                    });
                }
            }

            // Tokenize the line content using logos.
            let mut lexer = Token::lexer(content);
            while let Some(result) = lexer.next() {
                match result {
                    Ok(tok) => {
                        // Skip prompt comments (# but not ##) within a line.
                        if tok == Token::PromptComment {
                            continue;
                        }

                        let span = lexer.span();
                        let col = indent + span.start + 1;

                        tokens.push(SpannedToken {
                            token: LexToken::Token(tok),
                            line: line_num,
                            col,
                        });
                    }
                    Err(()) => {
                        let span = lexer.span();
                        let col = indent + span.start + 1;
                        return Err(LexError {
                            file: file.to_string(),
                            line: line_num,
                            col,
                            message: format!(
                                "unexpected character: '{}'",
                                &content[span.start..span.end]
                            ),
                        });
                    }
                }
            }

            // Emit a Newline token at the end of each processed line.
            // Use column = indent + content length + 1 (just past the content).
            tokens.push(SpannedToken {
                token: LexToken::Token(Token::Newline),
                line: line_num,
                col: indent + content.len() + 1,
            });
        }

        // At EOF, emit Dedent tokens for any remaining indentation levels.
        let final_line = source.lines().count() + 1;
        while indent_stack.len() > 1 {
            indent_stack.pop();
            tokens.push(SpannedToken {
                token: LexToken::Dedent,
                line: final_line,
                col: 1,
            });
        }

        // End with Eof token.
        tokens.push(SpannedToken {
            token: LexToken::Eof,
            line: final_line,
            col: 1,
        });

        Ok(tokens)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tokenize(source: &str) -> Vec<SpannedToken> {
        Lexer::tokenize(source, "test.torq").expect("tokenize failed")
    }

    fn token_types(source: &str) -> Vec<LexToken> {
        tokenize(source).into_iter().map(|st| st.token).collect()
    }

    #[test]
    fn simple_line() {
        let types = token_types("print");
        assert_eq!(
            types,
            vec![
                LexToken::Token(Token::Ident),
                LexToken::Token(Token::Newline),
                LexToken::Eof,
            ]
        );
    }

    #[test]
    fn indent_dedent() {
        let source = "each $x\n  print";
        let types = token_types(source);
        assert_eq!(
            types,
            vec![
                LexToken::Token(Token::Each),
                LexToken::Token(Token::ScalarVar),
                LexToken::Token(Token::Newline),
                LexToken::Indent,
                LexToken::Token(Token::Ident),
                LexToken::Token(Token::Newline),
                LexToken::Dedent,
                LexToken::Eof,
            ]
        );
    }

    #[test]
    fn multiple_dedents() {
        let source = "a\n  b\n    c\nd";
        let types = token_types(source);
        assert_eq!(
            types,
            vec![
                // a
                LexToken::Token(Token::Ident),
                LexToken::Token(Token::Newline),
                // indent, b
                LexToken::Indent,
                LexToken::Token(Token::Ident),
                LexToken::Token(Token::Newline),
                // indent, c
                LexToken::Indent,
                LexToken::Token(Token::Ident),
                LexToken::Token(Token::Newline),
                // dedent x2, d
                LexToken::Dedent,
                LexToken::Dedent,
                LexToken::Token(Token::Ident),
                LexToken::Token(Token::Newline),
                LexToken::Eof,
            ]
        );
    }

    #[test]
    fn blank_lines_skipped() {
        let source = "a\n\n  \n\nb";
        let types = token_types(source);
        assert_eq!(
            types,
            vec![
                LexToken::Token(Token::Ident),
                LexToken::Token(Token::Newline),
                LexToken::Token(Token::Ident),
                LexToken::Token(Token::Newline),
                LexToken::Eof,
            ]
        );
    }

    #[test]
    fn prompt_comments_stripped() {
        let source = "# this is a comment\nprint";
        let types = token_types(source);
        assert_eq!(
            types,
            vec![
                LexToken::Token(Token::Ident),
                LexToken::Token(Token::Newline),
                LexToken::Eof,
            ]
        );
    }

    #[test]
    fn doc_comments_preserved() {
        let source = "## this is a doc comment\nprint";
        let types = token_types(source);
        assert_eq!(
            types,
            vec![
                LexToken::Token(Token::DocComment),
                LexToken::Token(Token::Newline),
                LexToken::Token(Token::Ident),
                LexToken::Token(Token::Newline),
                LexToken::Eof,
            ]
        );
    }

    #[test]
    fn inline_prompt_comment_stripped() {
        let source = "print # inline comment";
        let types = token_types(source);
        assert_eq!(
            types,
            vec![
                LexToken::Token(Token::Ident),
                LexToken::Token(Token::Newline),
                LexToken::Eof,
            ]
        );
    }

    #[test]
    fn eof_dedents() {
        let source = "a\n  b\n    c";
        let types = token_types(source);
        assert_eq!(
            types,
            vec![
                LexToken::Token(Token::Ident),
                LexToken::Token(Token::Newline),
                LexToken::Indent,
                LexToken::Token(Token::Ident),
                LexToken::Token(Token::Newline),
                LexToken::Indent,
                LexToken::Token(Token::Ident),
                LexToken::Token(Token::Newline),
                LexToken::Dedent,
                LexToken::Dedent,
                LexToken::Eof,
            ]
        );
    }

    #[test]
    fn column_numbers() {
        let source = "  $x";
        let tokens = tokenize(source);
        // $x starts at position 2 in the content (0-indexed), indent is 2
        // col = indent + span.start + 1 = 2 + 0 + 1 = 3
        assert_eq!(tokens[0].token, LexToken::Indent);
        assert_eq!(tokens[0].col, 1);
        assert_eq!(tokens[1].token, LexToken::Token(Token::ScalarVar));
        assert_eq!(tokens[1].col, 3);
        assert_eq!(tokens[1].line, 1);
    }

    #[test]
    fn empty_source() {
        let types = token_types("");
        assert_eq!(types, vec![LexToken::Eof]);
    }
}
