//! The lexer turns raw Warden source text into a flat stream of `Token`s.
//! It knows nothing about grammar — it doesn't know that `let` must be
//! followed by an identifier. It only knows "these characters form this
//! kind of token." That separation is what makes the parser (next step)
//! simple: it only has to think about structure, never about characters.

#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    // Literals
    Ident(String),
    Number(f64),

    // Keywords
    Let,
    Fn,
    Print,
    Struct,
    If,
    Else,
    While,

    // Symbols
    Equals,     // =
    Semicolon,  // ;
    LParen,     // (
    RParen,     // )
    LBrace,     // {
    RBrace,     // }
    Comma,      // ,
    Dot,        // .
    Colon,      // :

    Eof,
}

pub struct Lexer {
    chars: Vec<char>,
    pos: usize,
}

impl Lexer {
    pub fn new(source: &str) -> Self {
        Lexer {
            chars: source.chars().collect(),
            pos: 0,
        }
    }

    fn peek(&self) -> Option<char> {
        self.chars.get(self.pos).copied()
    }

    fn advance(&mut self) -> Option<char> {
        let c = self.peek();
        self.pos += 1;
        c
    }

    fn skip_whitespace_and_comments(&mut self) {
        loop {
            match self.peek() {
                Some(c) if c.is_whitespace() => {
                    self.advance();
                }
                Some('/') if self.chars.get(self.pos + 1) == Some(&'/') => {
                    // Line comment: skip to end of line (or end of file).
                    while let Some(c) = self.peek() {
                        if c == '\n' {
                            break;
                        }
                        self.advance();
                    }
                }
                _ => break,
            }
        }
    }

    /// Turn the entire source into a Vec<Token>, ending with Eof.
    /// We tokenize everything up front (rather than lazily, token-by-token)
    /// because it keeps the parser simple: it can peek ahead freely.
    pub fn tokenize(&mut self) -> Result<Vec<Token>, String> {
        let mut tokens = Vec::new();

        loop {
            self.skip_whitespace_and_comments();
            let c = match self.peek() {
                Some(c) => c,
                None => break,
            };

            let token = match c {
                '=' => { self.advance(); Token::Equals }
                ';' => { self.advance(); Token::Semicolon }
                '(' => { self.advance(); Token::LParen }
                ')' => { self.advance(); Token::RParen }
                '{' => { self.advance(); Token::LBrace }
                '}' => { self.advance(); Token::RBrace }
                ',' => { self.advance(); Token::Comma }
                '.' => { self.advance(); Token::Dot }
                ':' => { self.advance(); Token::Colon }

                c if c.is_ascii_digit() => self.read_number(),
                c if c.is_alphabetic() || c == '_' => self.read_ident_or_keyword(),

                other => return Err(format!("unexpected character: '{}'", other)),
            };

            tokens.push(token);
        }

        tokens.push(Token::Eof);
        Ok(tokens)
    }

    fn read_number(&mut self) -> Token {
        let start = self.pos;
        while let Some(c) = self.peek() {
            if c.is_ascii_digit() || c == '.' {
                self.advance();
            } else {
                break;
            }
        }
        let text: String = self.chars[start..self.pos].iter().collect();
        Token::Number(text.parse().unwrap())
    }

    fn read_ident_or_keyword(&mut self) -> Token {
        let start = self.pos;
        while let Some(c) = self.peek() {
            if c.is_alphanumeric() || c == '_' {
                self.advance();
            } else {
                break;
            }
        }
        let text: String = self.chars[start..self.pos].iter().collect();
        match text.as_str() {
            "let" => Token::Let,
            "fn" => Token::Fn,
            "print" => Token::Print,
            "struct" => Token::Struct,
            "if" => Token::If,
            "else" => Token::Else,
            "while" => Token::While,
            _ => Token::Ident(text),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokenizes_let_binding() {
        let mut lexer = Lexer::new("let a = 5;");
        let tokens = lexer.tokenize().unwrap();
        assert_eq!(
            tokens,
            vec![
                Token::Let,
                Token::Ident("a".to_string()),
                Token::Equals,
                Token::Number(5.0),
                Token::Semicolon,
                Token::Eof,
            ]
        );
    }

    #[test]
    fn tokenizes_function_call() {
        let mut lexer = Lexer::new("consume(s);");
        let tokens = lexer.tokenize().unwrap();
        assert_eq!(
            tokens,
            vec![
                Token::Ident("consume".to_string()),
                Token::LParen,
                Token::Ident("s".to_string()),
                Token::RParen,
                Token::Semicolon,
                Token::Eof,
            ]
        );
    }

    #[test]
    fn rejects_unknown_character() {
        let mut lexer = Lexer::new("let a = @;");
        assert!(lexer.tokenize().is_err());
    }
}
