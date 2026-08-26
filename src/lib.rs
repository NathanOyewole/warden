pub mod ast;
pub mod checker;
pub mod interpreter;
pub mod lexer;
pub mod parser;

use checker::Checker;
use interpreter::Interpreter;
use lexer::Lexer;
use parser::Parser;

/// Lex → parse → check → interpret.
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

/// Lex → parse → check only (no execution).
pub fn check_source(source: &str) -> Result<(), String> {
    let tokens = Lexer::new(source).tokenize().map_err(|e| format!("lex error: {}", e))?;
    let program = Parser::new(tokens)
        .parse_program()
        .map_err(|e| format!("parse error: {}", e))?;
    Checker::new()
        .check_program(&program)
        .map_err(|e| format!("ownership error: {}", e))
}
