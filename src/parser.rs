//! Recursive descent parser: one function per grammar rule, each function
//! consumes exactly the tokens for its rule and returns the AST node for
//! it. This mirrors the grammar closely enough that you can read the
//! functions as the grammar — that's the whole appeal of hand-writing
//! a parser instead of generating one from a grammar file.
//!
//! Grammar (informal):
//!   program   := stmt* EOF
//!   stmt      := let_stmt | print_stmt | fn_def | expr_stmt
//!   let_stmt  := "let" IDENT "=" expr ";"
//!   print_stmt:= "print" "(" expr ")" ";"
//!   fn_def    := "fn" IDENT "(" params? ")" "{" stmt* "}"
//!   expr_stmt := expr ";"
//!   expr      := IDENT | NUMBER | call
//!   call      := IDENT "(" args? ")"

use crate::ast::{Expr, Program, Stmt};
use crate::lexer::Token;

pub struct Parser {
    tokens: Vec<Token>,
    pos: usize,
}

impl Parser {
    pub fn new(tokens: Vec<Token>) -> Self {
        Parser { tokens, pos: 0 }
    }

    fn peek(&self) -> &Token {
        &self.tokens[self.pos]
    }

    fn advance(&mut self) -> Token {
        let t = self.tokens[self.pos].clone();
        self.pos += 1;
        t
    }

    /// Consume the next token only if it exactly matches `expected`,
    /// otherwise return a descriptive error. Every grammar rule that
    /// requires a specific token (like the `;` at the end of a
    /// statement) goes through this, so error messages stay consistent.
    fn expect(&mut self, expected: Token) -> Result<(), String> {
        let actual = self.advance();
        if actual == expected {
            Ok(())
        } else {
            Err(format!("expected {:?}, found {:?}", expected, actual))
        }
    }

    fn expect_ident(&mut self) -> Result<String, String> {
        match self.advance() {
            Token::Ident(name) => Ok(name),
            other => Err(format!("expected identifier, found {:?}", other)),
        }
    }

    pub fn parse_program(&mut self) -> Result<Program, String> {
        let mut stmts = Vec::new();
        while *self.peek() != Token::Eof {
            stmts.push(self.parse_stmt()?);
        }
        Ok(stmts)
    }

    fn parse_stmt(&mut self) -> Result<Stmt, String> {
        match self.peek() {
            Token::Let => self.parse_let(),
            Token::Print => self.parse_print(),
            Token::Fn => self.parse_fn_def(),
            Token::Struct => self.parse_struct_def(),
            Token::If => self.parse_if(),
            Token::While => self.parse_while(),
            _ => self.parse_expr_stmt(),
        }
    }

    fn parse_let(&mut self) -> Result<Stmt, String> {
        self.expect(Token::Let)?;
        let name = self.expect_ident()?;
        self.expect(Token::Equals)?;
        let value = self.parse_expr()?;
        self.expect(Token::Semicolon)?;
        Ok(Stmt::Let { name, value })
    }

    fn parse_print(&mut self) -> Result<Stmt, String> {
        self.expect(Token::Print)?;
        self.expect(Token::LParen)?;
        let value = self.parse_expr()?;
        self.expect(Token::RParen)?;
        self.expect(Token::Semicolon)?;
        Ok(Stmt::Print(value))
    }

    fn parse_fn_def(&mut self) -> Result<Stmt, String> {
        self.expect(Token::Fn)?;
        let name = self.expect_ident()?;
        self.expect(Token::LParen)?;

        let mut params = Vec::new();
        if *self.peek() != Token::RParen {
            params.push(self.expect_ident()?);
            while *self.peek() == Token::Comma {
                self.advance();
                params.push(self.expect_ident()?);
            }
        }
        self.expect(Token::RParen)?;
        let body = self.parse_block()?;

        Ok(Stmt::FnDef { name, params, body })
    }

    fn parse_struct_def(&mut self) -> Result<Stmt, String> {
        self.expect(Token::Struct)?;
        let name = self.expect_ident()?;
        self.expect(Token::LBrace)?;

        let mut fields = Vec::new();
        if *self.peek() != Token::RBrace {
            fields.push(self.expect_ident()?);
            while *self.peek() == Token::Comma {
                self.advance();
                fields.push(self.expect_ident()?);
            }
        }
        self.expect(Token::RBrace)?;
        Ok(Stmt::StructDef { name, fields })
    }

    /// Shared by fn bodies, if/else branches, and while bodies:
    /// "{" stmt* "}"
    fn parse_block(&mut self) -> Result<Vec<Stmt>, String> {
        self.expect(Token::LBrace)?;
        let mut stmts = Vec::new();
        while *self.peek() != Token::RBrace {
            stmts.push(self.parse_stmt()?);
        }
        self.expect(Token::RBrace)?;
        Ok(stmts)
    }

    fn parse_if(&mut self) -> Result<Stmt, String> {
        self.expect(Token::If)?;
        self.expect(Token::LParen)?;
        let cond = self.parse_expr()?;
        self.expect(Token::RParen)?;
        let then_branch = self.parse_block()?;

        let else_branch = if *self.peek() == Token::Else {
            self.advance();
            Some(self.parse_block()?)
        } else {
            None
        };

        Ok(Stmt::If { cond, then_branch, else_branch })
    }

    fn parse_while(&mut self) -> Result<Stmt, String> {
        self.expect(Token::While)?;
        self.expect(Token::LParen)?;
        let cond = self.parse_expr()?;
        self.expect(Token::RParen)?;
        let body = self.parse_block()?;
        Ok(Stmt::While { cond, body })
    }

    fn parse_expr_stmt(&mut self) -> Result<Stmt, String> {
        let expr = self.parse_expr()?;
        self.expect(Token::Semicolon)?;
        Ok(Stmt::ExprStmt(expr))
    }

    /// expr := primary ("." IDENT)*
    /// The postfix loop is what lets `a.x.y` chain — each `.field` wraps
    /// the expression parsed so far in another `Expr::Field`.
    fn parse_expr(&mut self) -> Result<Expr, String> {
        let mut expr = self.parse_primary()?;
        while *self.peek() == Token::Dot {
            self.advance();
            let field = self.expect_ident()?;
            expr = Expr::Field { base: Box::new(expr), field };
        }
        Ok(expr)
    }

    fn parse_primary(&mut self) -> Result<Expr, String> {
        match self.advance() {
            Token::Number(n) => Ok(Expr::Number(n)),
            Token::Ident(name) => {
                // Lookahead: `name(` = call, `name{` = struct literal,
                // otherwise it's a bare variable reference.
                if *self.peek() == Token::LParen {
                    self.advance();
                    let mut args = Vec::new();
                    if *self.peek() != Token::RParen {
                        args.push(self.parse_expr()?);
                        while *self.peek() == Token::Comma {
                            self.advance();
                            args.push(self.parse_expr()?);
                        }
                    }
                    self.expect(Token::RParen)?;
                    Ok(Expr::Call { callee: name, args })
                } else if *self.peek() == Token::LBrace {
                    self.parse_struct_lit(name)
                } else {
                    Ok(Expr::Ident(name))
                }
            }
            other => Err(format!("unexpected token in expression: {:?}", other)),
        }
    }

    /// Point { x: 1, y: 2 }  — called after the type name has already
    /// been consumed by parse_primary.
    fn parse_struct_lit(&mut self, name: String) -> Result<Expr, String> {
        self.expect(Token::LBrace)?;
        let mut fields = Vec::new();
        if *self.peek() != Token::RBrace {
            let field_name = self.expect_ident()?;
            self.expect(Token::Colon)?;
            let value = self.parse_expr()?;
            fields.push((field_name, value));

            while *self.peek() == Token::Comma {
                self.advance();
                let field_name = self.expect_ident()?;
                self.expect(Token::Colon)?;
                let value = self.parse_expr()?;
                fields.push((field_name, value));
            }
        }
        self.expect(Token::RBrace)?;
        Ok(Expr::StructLit { name, fields })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::Lexer;

    fn parse(src: &str) -> Result<Program, String> {
        let tokens = Lexer::new(src).tokenize().unwrap();
        Parser::new(tokens).parse_program()
    }

    #[test]
    fn parses_let_binding() {
        let prog = parse("let a = 5;").unwrap();
        assert_eq!(
            prog,
            vec![Stmt::Let {
                name: "a".to_string(),
                value: Expr::Number(5.0)
            }]
        );
    }

    #[test]
    fn parses_move_then_use() {
        let prog = parse("let a = 5;\nlet b = a;\nprint(a);").unwrap();
        assert_eq!(prog.len(), 3);
        assert_eq!(
            prog[2],
            Stmt::Print(Expr::Ident("a".to_string()))
        );
    }

    #[test]
    fn parses_fn_def_and_call() {
        let prog = parse("fn consume(x) {\n  print(x);\n}\nconsume(a);").unwrap();
        assert_eq!(
            prog[0],
            Stmt::FnDef {
                name: "consume".to_string(),
                params: vec!["x".to_string()],
                body: vec![Stmt::Print(Expr::Ident("x".to_string()))],
            }
        );
        assert_eq!(
            prog[1],
            Stmt::ExprStmt(Expr::Call {
                callee: "consume".to_string(),
                args: vec![Expr::Ident("a".to_string())],
            })
        );
    }

    #[test]
    fn rejects_missing_semicolon() {
        assert!(parse("let a = 5").is_err());
    }

    #[test]
    fn parses_struct_def() {
        let prog = parse("struct Point { x, y }").unwrap();
        assert_eq!(
            prog,
            vec![Stmt::StructDef {
                name: "Point".to_string(),
                fields: vec!["x".to_string(), "y".to_string()],
            }]
        );
    }

    #[test]
    fn parses_struct_literal_and_field_access() {
        let prog = parse("let p = Point { x: 1, y: 2 };\nprint(p.x);").unwrap();
        assert_eq!(
            prog[0],
            Stmt::Let {
                name: "p".to_string(),
                value: Expr::StructLit {
                    name: "Point".to_string(),
                    fields: vec![
                        ("x".to_string(), Expr::Number(1.0)),
                        ("y".to_string(), Expr::Number(2.0)),
                    ],
                },
            }
        );
        assert_eq!(
            prog[1],
            Stmt::Print(Expr::Field {
                base: Box::new(Expr::Ident("p".to_string())),
                field: "x".to_string(),
            })
        );
    }

    #[test]
    fn parses_if_else() {
        let prog = parse("if (a) {\n  print(a);\n} else {\n  print(b);\n}").unwrap();
        assert_eq!(
            prog[0],
            Stmt::If {
                cond: Expr::Ident("a".to_string()),
                then_branch: vec![Stmt::Print(Expr::Ident("a".to_string()))],
                else_branch: Some(vec![Stmt::Print(Expr::Ident("b".to_string()))]),
            }
        );
    }

    #[test]
    fn parses_while_loop() {
        let prog = parse("while (a) {\n  print(a);\n}").unwrap();
        assert_eq!(
            prog[0],
            Stmt::While {
                cond: Expr::Ident("a".to_string()),
                body: vec![Stmt::Print(Expr::Ident("a".to_string()))],
            }
        );
    }
}
