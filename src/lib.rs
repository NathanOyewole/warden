pub mod ast;
pub mod checker;
pub mod interpreter;
pub mod lexer;
pub mod parser;

use checker::Checker;
use interpreter::Interpreter;
use lexer::Lexer;
use parser::Parser;

/// Runs a Warden program end-to-end: lex → parse → check → interpret.
/// This is the single entry point tests and `main.rs` both go through,
/// so "does this program run" always means the same thing everywhere.
pub fn run_source(source: &str) -> Result<(), String> {
    let tokens = Lexer::new(source).tokenize().map_err(|e| format!("lex error: {}", e))?;
    let program = Parser::new(tokens)
        .parse_program()
        .map_err(|e| format!("parse error: {}", e))?;
    Checker::new()
        .check_program(&program)
        .map_err(|e| format!("ownership error: {}", e))?;
    Interpreter::new()
        .run(&program)
        .map_err(|e| format!("runtime error: {}", e))
}

/// Like `run_source`, but stops after the checker and never executes the
/// program. Used by the "reject" example tests, where we want to prove
/// the checker rejects a program without worrying about whether it
/// would also run correctly (it's expected not to compile at all).
pub fn check_source(source: &str) -> Result<(), String> {
    let tokens = Lexer::new(source).tokenize().map_err(|e| format!("lex error: {}", e))?;
    let program = Parser::new(tokens)
        .parse_program()
        .map_err(|e| format!("parse error: {}", e))?;
    Checker::new()
        .check_program(&program)
        .map_err(|e| format!("ownership error: {}", e))
}
