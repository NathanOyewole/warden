# Warden — Roadmap

*Building a minimal ownership checker to understand why Rust's is hard.*

## Phase 1 — Lexer ✅
- [x] Token enum (literals, keywords, symbols)
- [x] Hand-rolled scanner (`peek`/`advance`/`skip_whitespace`)
- [x] Number + identifier/keyword reading
- [x] Unit tests (let binding, function call, unknown-char rejection)
- [x] Article: "The premise" + "Tokenizing Warden"

## Phase 2 — Parser (AST) ✅
- [x] AST node types: `Let`, `Call`, `FnDef`, `Ident`, `Number`, `ExprStmt`
- [x] Recursive descent parser over the token stream
- [x] Parser tests (let binding, fn def + call, move-then-use, missing semicolon)
- [x] Article: "From tokens to trees"

## Phase 3 — Structs + partial moves ✅
- [x] Minimal `struct` syntax (lexer + parser extensions)
- [x] Field access (`a.x`) in the AST, chainable
- [x] `if`/`else` and `while` added (needed for Phase 4's conditional/loop move rules)
- [x] Article: "Why structs break the easy version"

## Phase 4 — The ownership checker (thesis chapter) ✅
- [x] Static analysis pass over the AST
- [x] Move-state tracking per scope (Owned / Moved / PartiallyMoved)
- [x] Rules: use-after-move, double-move, move-in-loop, partial-move, move-then-reassign
- [x] 11 checker tests, each isolating one rule
- [x] Article: "Building the checker" + "Where it gets hard"

## Phase 5 — Tree-walking interpreter ✅
- [x] Evaluator for checked programs (Value::Number, Value::Struct)
- [x] Full pipeline wired: lex → parse → check → interpret
- [x] Article: "Making it run"

## Phase 6 — Test suite + example programs ✅
- [x] 6 programs in examples/accept/ that should compile + run
- [x] 6 programs in examples/reject/ that should be rejected (each isolating one rule)
- [x] Integration test (tests/examples.rs) verifying every example against the real checker
- [x] Lexer gained `//` comment support along the way (examples needed it)
- [x] Article: worked examples woven through the checker section

## Phase 7 — What we didn't build (written-only)
- [ ] Lifetimes, `&`/`&mut`, aliasing rules, non-lexical lifetimes
- [ ] Explicit connection back to real Rust internals

## Phase 8 — Closing thesis + polish
- [ ] Diagrams: AST shape, ownership state transitions
- [ ] Final read-through / edit pass
- [ ] Decide: single long-form piece vs. 4–5 part series
- [ ] Package for publication
