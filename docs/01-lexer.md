# The Lexer — `src/lexer.rs`

**Job:** turn raw source text into a flat list of `Token`s. Knows
nothing about grammar — doesn't know `let` must be followed by an
identifier, only that certain characters form certain kinds of tokens.

## Diagram

```
"let a = 5;"
   │
   ├─ 'l','e','t' ──────────► Token::Let       (keyword match)
   ├─ 'a' ──────────────────► Token::Ident("a")
   ├─ '=' ──────────────────► Token::Equals
   ├─ '5' ──────────────────► Token::Number(5.0)
   └─ ';' ──────────────────► Token::Semicolon
                                       │
                                       ▼
        [Let, Ident("a"), Equals, Number(5.0), Semicolon, Eof]
```

## Rust concepts used here

- **`enum` with data** — `Token::Number(f64)` isn't just a label, it
  *carries* the actual float value inside the variant. Compare to
  `Token::Let`, which carries nothing — it doesn't need to.
- **`#[derive(Debug, Clone, PartialEq)]`** — an attribute that asks the
  compiler to auto-generate: printable-for-debugging (`Debug`),
  copyable (`Clone`), and comparable with `==` (`PartialEq`). Seen
  above almost every type in this project.
- **`Option<T>`** — Rust has no `null`. `peek()` returns
  `Option<char>`: `Some(c)` if there's a character there, `None` if
  we've run off the end. Every "is there more input?" check in the
  lexer goes through this.
- **`&self` vs `&mut self`** — `peek(&self)` only reads the struct.
  `advance(&mut self)` is allowed to mutate it (bump `pos` forward).
  Rust enforces this distinction at compile time.

## Section by section

**`struct Lexer { chars: Vec<char>, pos: usize }`**
Two fields: the full source as a list of characters, and the current
read position. `Vec<char>` (rather than indexing the raw `&str`
directly) sidesteps a real Rust gotcha — strings are UTF-8 bytes, and
byte-indexing into the middle of a multi-byte character panics.
Collecting into `Vec<char>` up front means every index is a whole,
safe character.

**`peek()` / `advance()`**
The two primitives everything else is built from. `peek` looks without
consuming; `advance` consumes and moves the cursor forward. Nearly
every "read while this condition holds" loop in the lexer is the same
shape: peek, check, advance-or-stop.

**`skip_whitespace_and_comments()`**
A `loop { match self.peek() { ... } }`. Three cases: whitespace → eat
one char, loop again. `/` followed by another `/` → we've found a line
comment, eat characters until `\n`. Anything else → `break`, we've
landed on real content.

**`tokenize()`**
The main driver. Repeatedly: skip junk, look at the next character,
`match` it against every symbol Warden recognizes. Single-character
symbols (`=`, `;`, `(`, `)`, `{`, `}`, `,`, `.`, `:`) each get their own
match arm. Digits and letters hand off to `read_number` and
`read_ident_or_keyword`, which run their own consume-while-matching
loop and then build the right token from the collected text.

**`read_ident_or_keyword()` — why keywords are handled here, not as
separate rules**
`let`, `fn`, and a variable named `letter` all start with a letter —
you can't tell them apart until you've read the whole word. So the
lexer always reads the *full* identifier first, then checks: is this
text one of Warden's reserved words? `match text.as_str() { "let" =>
Token::Let, ... _ => Token::Ident(text) }`. Every real lexer does
keyword recognition this way.

**What the lexer deliberately does *not* do**
Decide whether a program is valid Warden — `let let let ;` tokenizes
just fine (three `Token::Let`s and a semicolon); it's nonsense as a
*program*, but not as a *token stream*. That distinction — "valid
tokens" vs. "valid grammar" — is exactly why the parser exists as a
separate stage.

## Why this matters for real Rust

rustc's actual lexer works the same way, just with a much larger
token set (string literals, raw strings, lifetimes like `'a`, macros,
etc.) and much better error recovery. The core technique — a
peek/advance cursor over characters, keyword lookup after reading full
identifiers — doesn't change.
