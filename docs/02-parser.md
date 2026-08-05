# The Parser — `src/parser.rs` and `src/ast.rs`

**Job:** turn the flat token list into a tree (the AST — Abstract
Syntax Tree) that mirrors the actual structure of the program. This is
what the checker and interpreter will operate on; neither of them ever
looks at tokens or characters again.

## Diagram

```
tokens: [Let, Ident("a"), Equals, Number(5.0), Semicolon, Eof]

              parse_program()
                    │
                    ▼
              parse_stmt() ── sees `Let` → parse_let()
                    │
      ┌─────────────┼──────────────┐
      │ expect(Let)  │ expect_ident │  parse_expr() → Number(5.0)
      │              │  → "a"       │  expect(Semicolon)
      └─────────────┴──────────────┘
                    │
                    ▼
     Stmt::Let { name: "a", value: Expr::Number(5.0) }
```

For an expression like `p.x`, the tree nests:

```
Expr::Field {
    base: Box::new(Expr::Ident("p")),
    field: "x",
}
```

## Rust concepts used here

- **`match self.peek() { Token::Let => ..., Token::If => ..., _ => ... }`**
  — Rust's `match` is exhaustive: every arm of an enum must be
  accounted for (or covered by `_`). This is how `parse_stmt` decides
  which grammar rule to hand off to, just by looking at the next
  token.
- **`Result<T, String>` and `?`** — every `parse_*` function returns
  `Result<Something, String>`. The `?` operator after a call like
  `self.expect(Token::Semicolon)?;` means "if this failed, stop and
  bubble the error up immediately." That's why the parser functions
  read as a flat sequence of steps even though any step could fail.
- **`Box<Expr>`** — `Expr::Field { base: Box<Expr>, field: String }`.
  Rust needs to know the exact size of every type at compile time, but
  an `Expr` that can contain another `Expr` inside itself has no fixed
  size (it could nest forever). `Box` puts that inner value on the
  heap and stores just a pointer to it inline — the standard fix for
  self-referential / recursive data structures in Rust.
- **`Vec<Stmt>`** — a function body, an `if` branch, and a `while` body
  are all just `Vec<Stmt>` — an ordered list of statements. Reusing one
  type for "a block of code" everywhere is why `parse_block()` exists
  as a single shared helper.

## Section by section

**`ast.rs` — `Expr` and `Stmt`**
Two enums define the entire shape of a Warden program. `Expr` is
anything that produces a value (`Ident`, `Number`, `Call`, `Field`,
`StructLit`). `Stmt` is anything that's a full instruction
(`Let`, `Print`, `If`, `While`, `FnDef`, `StructDef`,
`ExprStmt` — a bare expression used for its side effect, like
`consume(a);`).

**`peek()` / `advance()` / `expect()`**
Same peek/advance pattern as the lexer, but over tokens instead of
characters. `expect(Token::Semicolon)` is new: it consumes the next
token *only if* it matches exactly what's required, otherwise returns
a descriptive error. Every place the grammar demands a specific token
— the `;` ending a statement, the `)` closing a call — goes through
`expect`, so every "you forgot something" error has the same shape.

**`parse_stmt()` — the dispatcher**
Looks at the very next token and routes to the matching rule:
`Let` → `parse_let`, `Struct` → `parse_struct_def`, `If` → `parse_if`,
etc. Anything that doesn't match a keyword falls through to
`parse_expr_stmt` — a bare expression statement like a function call.

**`parse_block()` — the one function reused everywhere**
`"{" stmt* "}"` — parse statements until you hit a closing brace. Used
identically for function bodies, `if`/`else` branches, and `while`
bodies. Writing it once and calling it four times is exactly the kind
of grammar reuse a hand-written recursive descent parser makes easy.

**`parse_expr()` and `parse_primary()` — the interesting part**
`parse_expr` calls `parse_primary` to get a base expression, then
loops: *"is the next token a `.`? If so, this is a field access —
wrap what I have so far in `Expr::Field` and keep looking."* That loop
is what makes `a.x.y` parse as nested `Field`s.

`parse_primary` is where **lookahead** happens. When it sees an
identifier, it doesn't yet know if that's a bare variable (`a`), a
function call (`consume(x)`), or a struct literal (`Point { x: 1 }`)
— all three start with the same token. So it peeks *one token past*
the identifier: `(` means commit to parsing a call, `{` means commit
to a struct literal, anything else means it was just a variable. This
"peek one more token to disambiguate" trick is the single most common
pattern in recursive-descent parsing.

## What's deliberately not here

No operator precedence (no `+`, `-`, `*` — Warden doesn't need
arithmetic to tell the ownership story), and no expressions that
themselves branch (`if` is a statement, not something you can assign
from). Keeping the grammar this small is what makes the checker
readable in the next doc.

## Why this matters for real Rust

rustc's parser is hand-written recursive descent too (not generated
from a grammar file), for exactly the reason this one is: it gives
full control over error messages and recovery. The lookahead trick for
disambiguating `ident`, `ident(...)`, and `ident { ... }` shows up
constantly in real language grammars — Rust itself has to solve a
much gnarlier version of this problem to tell apart `if x {` (block)
from `if x { y: 1 }` (struct literal) in expression position.
