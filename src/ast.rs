//! The AST is the tree-shaped representation of a Warden program that
//! everything downstream — the checker and the interpreter — actually
//! operates on. The parser's only job is building this tree correctly;
//! it doesn't care about ownership rules at all, same way the lexer
//! didn't care about grammar. Each stage does exactly one job.

#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    Ident(String),
    Number(f64),
    Call { callee: String, args: Vec<Expr> },
    /// a.x  — field access, chainable (a.x.y wraps Field around Field)
    Field { base: Box<Expr>, field: String },
    /// Point { x: 1, y: 2 }
    StructLit { name: String, fields: Vec<(String, Expr)> },
}

#[derive(Debug, Clone, PartialEq)]
pub enum Stmt {
    /// let <name> = <expr>;
    Let { name: String, value: Expr },
    /// print(<expr>);
    Print(Expr),
    /// A bare expression used for its side effect, e.g. `consume(s);`
    ExprStmt(Expr),
    /// fn <name>(<params>) { <body> }
    FnDef {
        name: String,
        params: Vec<String>,
        body: Vec<Stmt>,
    },
    /// struct <name> { field1, field2 }
    StructDef { name: String, fields: Vec<String> },
    /// if (<cond>) { <then> } else { <else> }
    If {
        cond: Expr,
        then_branch: Vec<Stmt>,
        else_branch: Option<Vec<Stmt>>,
    },
    /// while (<cond>) { <body> }
    While { cond: Expr, body: Vec<Stmt> },
}

pub type Program = Vec<Stmt>;
