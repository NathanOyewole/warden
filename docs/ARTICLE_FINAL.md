# Warden: Building a Minimal Ownership Checker to Understand Why Rust's Is Hard

*A stripped-down ownership checker, built from scratch in Rust, to isolate the one idea in Rust that's actually simple — and find the exact edge where it stops being simple.*

Rust has a reputation for being brutally hard to learn, and almost all of that reputation traces back to one subsystem: the borrow checker. But "the borrow checker" is actually solving two distinct problems bundled together — **ownership** (every value has exactly one owner; use it after that owner gives it up, and it's a compile error) and **borrowing** (temporarily letting other code look at or modify a value without taking it, governed by lifetimes and aliasing rules).

Nearly everything people find painful about early Rust is the second one, not the first. Ownership alone is a genuinely simple, learnable idea. So I built it in isolation — no borrowing, no lifetimes, just the bare skeleton — to see whether stripping it down would actually make it click, and whether hitting the wall where the easy version breaks would explain why the full version is hard.

That's Warden: a small language with a real compiler pipeline — lexer, parser, static ownership checker, interpreter — written in Rust, whose entire type system is numbers, structs, and functions, plus one rule layered on top of all of it.

```
let a = 5;
let b = a;        // a moved into b
print(a);         // REJECTED: use of moved value `a`
```

## The pipeline

```
 source text
     │
     ▼
 ┌─────────┐  characters → tokens
 │  LEXER  │
 └─────────┘
     │
     ▼
 ┌─────────┐  tokens → tree (AST)
 │ PARSER  │
 └─────────┘
     │
     ▼
 ┌─────────┐  walks the tree, tracks ownership state per variable —
 │ CHECKER │  the actual thesis of this project
 └─────────┘
     │  only proceeds if the checker approved it
     ▼
 ┌─────────────┐  walks the same tree again, computes real values
 │ INTERPRETER │
 └─────────────┘
```

Four stages, each blind to everything except the one immediately before it. The lexer never thinks about grammar. The parser never thinks about ownership. The checker never thinks about actual runtime values. That separation of concerns — more than any individual algorithm — is the foundational idea of compiler architecture.

## Tokenizing Warden

The lexer's only job is turning characters into a flat list of tokens — it doesn't know that `let` must be followed by an identifier, only that certain characters form certain kinds of tokens.

```rust
pub enum Token {
    Ident(String),
    Number(f64),
    Let, Fn, Print, Struct, If, Else, While,
    Equals, Semicolon, LParen, RParen, LBrace, RBrace, Comma, Dot, Colon,
    Eof,
}
```

The scanner is built from two primitives: `peek()` looks at the current character without consuming it, `advance()` consumes and moves forward. Nearly every "read while this condition holds" loop — reading a number, reading an identifier — is the same shape: peek, check a condition, advance or stop.

One detail worth calling out: keywords aren't recognized as their own lexer rule. `let`, `fn`, and a variable named `letter` all start with a letter — you can't tell them apart until you've read the whole word. So the lexer always reads the full identifier first, then checks whether that text happens to be reserved. Every real lexer does keyword recognition this way.

## From tokens to trees

The parser is hand-written recursive descent — one function per grammar rule, each consuming exactly the tokens for that rule. I chose this over a parser generator specifically because it reads *as* the grammar: `parse_let` mirrors the `let` rule almost line for line. That transparency matters when the entire point of the exercise is to understand what's happening, not hide it behind a macro — and it's also what rustc itself uses, for the same reason: full control over error messages.

```rust
fn parse_let(&mut self) -> Result<Stmt, String> {
    self.expect(Token::Let)?;
    let name = self.expect_ident()?;
    self.expect(Token::Equals)?;
    let value = self.parse_expr()?;
    self.expect(Token::Semicolon)?;
    Ok(Stmt::Let { name, value })
}
```

The single most interesting piece of the parser is lookahead. When it sees an identifier, it doesn't yet know if that's a bare variable (`a`), a function call (`consume(x)`), or a struct literal (`Point { x: 1 }`) — all three start identically. So it peeks one token further: `(` commits to a call, `{` commits to a struct literal, anything else means it was just a variable reference. This "peek one more token to disambiguate" trick is the most common pattern in recursive-descent parsing, and real Rust has to solve a much gnarlier version of it — telling apart `if x {` (a block) from `if x { y: 1 }` (a struct literal) in expression position.

## Why structs break the easy version

The first version of ownership checking — move a variable, mark it used up — is almost too simple to be interesting. It gets genuinely interesting the moment you add structs, because now a *part* of a value can move while the rest stays valid:

```rust
struct Point { x, y }

let p = Point { x: 1, y: 2 };
let px = p.x;
print(p.y);   // fine — y was never touched
print(p);     // rejected — p as a whole is missing a piece
```

This forces a third state onto what was a simple binary owned/moved distinction:

```
        let px = p.x;
Owned ─────────────────► PartiallyMoved({x})
                                │              │
                          print(p.y) → OK   print(p) → ERROR
                          (y untouched)     (whole value, x missing)
```

Most toy ownership-checker writeups skip this case entirely and stick to whole-value moves, which is exactly why it felt like the section worth building carefully.

## Building the checker

This is the actual thesis of the project. Every variable lives in one of three states — `Owned`, `Moved`, or `PartiallyMoved(fields)` — tracked in a lookup table (`name → state`) threaded through the whole tree walk.

Two functions do all the work. `check_use` asks "can I legally read this right now?" without changing anything. `apply_move` runs immediately after a successful check and marks whatever was just read as used up. Both are recursive, one match arm per expression kind — the function's shape mirrors the AST's shape, which is the same pattern you see in every stage of this project.

```rust
match scope.get(name) {
    Some(VarState::Owned) => Ok(()),
    Some(VarState::Moved) => Err(format!("use of moved value `{}`", name)),
    Some(VarState::PartiallyMoved(fields)) => {
        Err(format!("use of partially moved value `{}` (missing: {:?})", name, fields))
    }
    None => Err(format!("use of undeclared variable `{}`", name)),
}
```

Four of the checker's five behaviors fall directly out of this model with no special-case code: use-after-move and double-move are the same code path (a second move attempt is just another failed `check_use`), partial-move tracking is the `Field` arm operating on a field-name set, and move-then-reassign works because a `let` statement *always* inserts a fresh `Owned` entry — even overwriting a `Moved` one — which is exactly the mechanism that makes shadowing "un-stick" a moved variable.

## Where it gets hard

The fifth behavior — how ownership interacts with control flow — is where the easy version actually breaks, and where building this taught me the most.

**Branching.** If a variable is moved on one path of an `if`/`else` but not the other, what's its state afterward?

```rust
let a = 5;
if (a) {
    let b = a;      // a moved on this path only
} else {
    print(0);
}
print(a);            // rejected — even though only one path moved it
```

Real Rust is path-sensitive here: it can prove the disagreement away by tracking which branch actually ran. Warden's checker can't — it clones the scope, checks each branch independently, then merges: any disagreement between branches is conservatively treated as `Moved`, full stop. That's strictly *more* restrictive than necessary. I chose it deliberately, because it's exactly the shortcut you reach for the first time you try to solve this, and hitting its limitation directly — needing a shadow-reassign to un-stick a variable that real Rust wouldn't require one for — was more instructive than reading about path-sensitivity in the abstract.

**Loops.** A variable that existed before a loop can't be moved from inside the loop body:

```rust
let a = 1;
while (a) {
    let b = a;    // rejected — a existed before the loop
}
```

The checker doesn't attempt to prove how many times a loop will actually execute — that's undecidable in general. It just conservatively assumes "could be more than once" and blocks the move outright. Variables freshly bound *inside* the loop body are exempt, since they're conceptually new on every pass.

Both of these are the same kind of decision, made twice: when precise analysis is hard, fall back to a conservative rule that never accepts something unsound, even if it sometimes rejects something that would actually be fine. That's not a cop-out — it's the same trade-off real static analysis makes constantly, just visible here at a scale small enough to see clearly.

## Making it run

Once a program passes the checker, running it is the easy part: a tree-walking interpreter that evaluates the AST directly, no bytecode, no compilation step. It maintains an environment (`HashMap<String, Value>`) structurally identical to the checker's scope, just holding real values instead of ownership states — same idea, later stage.

Building this exposed a gap the checker never would have: Warden has `let` (always a fresh binding) but no plain reassignment for an existing variable. Which means a `while` loop's condition can never actually change over its lifetime — it's either always true (infinite loop) or always false (zero iterations). There's no way, today, to write a loop in Warden that runs a finite, nonzero number of times. Fixing it means adding real assignment, which immediately reopens the exact aliasing question ownership-checking exists to answer in the first place — a good example of how a limitation in one stage can surface only once you try to actually execute the language, not just parse and check it.

## What we didn't build

Every simplification above is Warden skipping one specific piece of what real Rust does:

| Feature | Why Warden skips it | Why it's genuinely hard |
|---|---|---|
| Borrowing (`&`, `&mut`) | Everything moves instead | Requires tracking a *set* of active borrows, not a 3-state enum |
| Lifetimes | No borrows to bound | Region inference — proving no reference outlives its data |
| Non-lexical lifetimes | N/A | Real control-flow analysis, not just scope-based rules |
| Aliasing rules | No two names can ever refer to the same value | This is the actual rule the whole borrow checker exists to enforce |

The honest throughline: Warden gets to skip *all* of this specifically because it never lets two names refer to the same value at the same time. The moment real Rust allows borrowing — looking at a value without taking it — it inherits every row in that table simultaneously. Move-checking alone, which is what Warden implements, is real ownership checking. It's also the easy 20%.

## Closing thesis

Building Warden didn't make Rust's borrow checker feel simple. It did something more useful: it drew a precise line between the part that's actually simple (ownership) and the part that's actually hard (borrowing, lifetimes, aliasing) — and let me feel exactly where that line sits, by hitting it twice, once in branching and once in loops. Every conservative shortcut the checker takes stands in for a harder analysis Rust's real implementation has to do precisely. Knowing exactly which shortcut you're taking, and why, is a different kind of understanding than knowing the borrow checker is "hard."

---

**Try it yourself:** the full source, test suite, and a 12-program example set (6 accepted, 6 rejected, each isolating one rule) are on GitHub — [link]. If you build your own stripped-down version of a hard part of a language you use, I'd genuinely like to see it.
