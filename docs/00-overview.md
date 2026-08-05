# Warden - Project Overview

Warden is a minimal ownership checker, built to understand *why* Rust's
real borrow checker is hard, by building the simplest possible version
of just one piece of it: single-owner, move-only value tracking, with
no lifetimes and no borrowing.

## The pipeline

```
 source text ("let a = 5; let b = a; print(a);")
     │
     ▼
 ┌─────────┐   turns characters into a flat list of tokens
 │  LEXER  │   [Let, Ident("a"), Equals, Number(5.0), Semicolon, ...]
 └─────────┘
     │  Vec<Token>
     ▼
 ┌─────────┐   turns tokens into a tree that mirrors program structure
 │ PARSER  │   Stmt::Let { name: "a", value: Expr::Number(5.0) }
 └─────────┘
     │  AST (Program = Vec<Stmt>)
     ▼
 ┌─────────┐   walks the tree, tracks Owned / Moved / PartiallyMoved
 │ CHECKER │   per variable, rejects unsafe programs before they run
 └─────────┘
     │  Ok(()) — only proceeds if the checker approves
     ▼
 ┌─────────────┐   walks the SAME tree again, computes real values,
 │ INTERPRETER │   executes print statements
 └─────────────┘
```

Four stages. Each one only knows about the stage immediately before
it - the lexer never thinks about grammar, the parser never thinks
about ownership, the checker never thinks about actual runtime values.
That separation of concerns is the single most important idea in
compiler architecture — more important than any individual algorithm.

## Reading order

1. [`01-lexer.md`](./01-lexer.md) - characters → tokens
2. [`02-parser.md`](./02-parser.md) - tokens → AST
3. [`03-checker.md`](./03-checker.md) - the ownership analysis (the thesis of the project)
4. [`04-interpreter.md`](./04-interpreter.md) - AST → running program

Each doc walks the corresponding `src/*.rs` file section by section,
explains the Rust syntax used (not just the compiler-design ideas), and
ends with a short "why this matters for real Rust" note.
