use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Span — source location information
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Span {
    pub file: String,
    pub line: usize,
    pub col: usize,
}

// ---------------------------------------------------------------------------
// Program — top-level compilation unit
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Program {
    pub blocks: Vec<Block>,
}

// ---------------------------------------------------------------------------
// Block — a named block (function / task)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Block {
    pub name: String,
    pub params: Vec<Param>,
    pub body: Vec<Statement>,
    pub doc_comments: Vec<String>,
    pub span: Span,
}

// ---------------------------------------------------------------------------
// Param & Sigil
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Param {
    pub sigil: Sigil,
    pub name: String,
    pub span: Span,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum Sigil {
    Scalar,
    Array,
    Dict,
    Error,
    Regex,
    Shared,
    BlockRef,
}

// ---------------------------------------------------------------------------
// Statement
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum Statement {
    Pipeline(Pipeline),
    Assignment(Assignment),
    Each(Each),
    Loop(Loop),
    Expression(Expr),
}

// ---------------------------------------------------------------------------
// Pipeline
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Pipeline {
    pub stages: Vec<Expr>,
    pub span: Span,
}

// ---------------------------------------------------------------------------
// Assignment
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Assignment {
    pub target: Variable,
    pub value: Box<Expr>,
    pub span: Span,
}

// ---------------------------------------------------------------------------
// Each
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Each {
    pub iterable: Box<Expr>,
    pub binding: Variable,
    pub sequential: bool,
    pub body: Vec<Statement>,
    pub span: Span,
}

// ---------------------------------------------------------------------------
// Loop
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Loop {
    pub condition: Option<Box<Expr>>,
    pub body: Vec<Statement>,
    pub span: Span,
}

// ---------------------------------------------------------------------------
// Expr — expression variants
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum Expr {
    Literal(Literal),
    Variable(Variable),
    Array(Vec<Expr>, Span),
    Dict(Vec<DictEntry>, Span),
    BlockCall(BlockCall),
    BlockRef(String, Span),
    Call(Call),
    Match(Match),
    BinOp(BinOp),
    MemberAccess(MemberAccess),
    StringInterp(Vec<StringPart>, Span),
    Ternary(Ternary),
    Pipeline(Pipeline),
    Break(Span),
    Group(Box<Expr>, Span),
}

// ---------------------------------------------------------------------------
// Variable
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Variable {
    pub sigil: Sigil,
    pub name: String,
    pub span: Span,
}

// ---------------------------------------------------------------------------
// Literal
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum Literal {
    Int(i64, Span),
    Float(f64, Span),
    String(String, Span),
    Bool(bool, Span),
    Null(Span),
    Duration(Duration, Span),
}

// ---------------------------------------------------------------------------
// Duration & DurationUnit
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Duration {
    pub value: u64,
    pub unit: DurationUnit,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum DurationUnit {
    Milliseconds,
    Seconds,
    Minutes,
    Hours,
    Days,
}

// ---------------------------------------------------------------------------
// DictEntry
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DictEntry {
    pub key: String,
    pub value: Expr,
    pub span: Span,
}

// ---------------------------------------------------------------------------
// BlockCall
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BlockCall {
    pub name: String,
    pub args: Vec<Expr>,
    pub span: Span,
}

// ---------------------------------------------------------------------------
// Call
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Call {
    pub name: String,
    pub args: Vec<Expr>,
    pub span: Span,
}

// ---------------------------------------------------------------------------
// Match & MatchArm
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Match {
    pub arms: Vec<MatchArm>,
    pub span: Span,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MatchArm {
    pub pattern: Pattern,
    pub body: Box<Expr>,
    pub span: Span,
}

// ---------------------------------------------------------------------------
// Pattern
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum Pattern {
    Literal(Literal),
    Comparison(ComparisonOp, Box<Expr>),
    Variable(Variable),
    FieldMatch(FieldMatch),
    And(Vec<Pattern>),
    Wildcard,
    WithCaptures(Box<Pattern>, Vec<Variable>),
}

// ---------------------------------------------------------------------------
// FieldMatch
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FieldMatch {
    pub field: String,
    pub op: ComparisonOp,
    pub value: Box<Expr>,
}

// ---------------------------------------------------------------------------
// ComparisonOp
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ComparisonOp {
    Eq,
    NotEq,
    Gt,
    Lt,
    GtEq,
    LtEq,
}

// ---------------------------------------------------------------------------
// BinOp & BinOpKind
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum BinOpKind {
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Pow,
    Gt,
    Lt,
    GtEq,
    LtEq,
    Eq,
    NotEq,
    And,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BinOp {
    pub left: Box<Expr>,
    pub op: BinOpKind,
    pub right: Box<Expr>,
    pub span: Span,
}

// ---------------------------------------------------------------------------
// MemberAccess
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MemberAccess {
    pub object: Box<Expr>,
    pub field: String,
    pub span: Span,
}

// ---------------------------------------------------------------------------
// StringPart (for string interpolation)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum StringPart {
    Literal(String),
    Interpolation(Variable),
}

// ---------------------------------------------------------------------------
// Ternary
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Ternary {
    pub condition: Box<Expr>,
    pub then_expr: Box<Expr>,
    pub else_expr: Box<Expr>,
    pub span: Span,
}
