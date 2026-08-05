# What We Didn't Build

Warden proves out *one* piece of Rust's ownership story: single-owner,
move-only values. This doc is honest about everything real Rust adds
on top, and why each piece is a genuinely harder problem - not just
"more code."

## 1. Borrowing (`&` and `&mut`)

Warden has no way to look at a value without taking it. Real Rust lets
you *borrow* - temporarily access a value without moving it - via `&`
(shared, read-only, any number of them at once) and `&mut` (exclusive,
read-write, only one at a time, and no shared borrows may coexist with
it).

This is a second, orthogonal tracking problem on top of ownership.
Warden's checker only ever asks "has this been moved?" Rust's checker
also has to ask, at every point in the program: "is there currently an
outstanding borrow of this value, and if so, what kind?" That's a
fundamentally different kind of state to track - not a three-state
enum like `Owned`/`Moved`/`PartiallyMoved`, but a set of *active
borrows*, each with its own scope.

## 2. Lifetimes

Once borrows exist, you need to answer: *how long is a given borrow
allowed to live?* A borrow can't outlive the value it points to (no
dangling references), and two borrows of the same data have to prove
their lifetimes don't dangerously overlap (a `&mut` can't coexist with
anything else). Rust's lifetime syntax (`'a`) is literally naming these
regions so the compiler can compare them.

This is the piece most people mean when they say "the borrow checker
is hard." It's a form of *region inference* - the compiler has to work
out, often without you writing a single explicit lifetime, how long
every reference in your program is allowed to be valid, and prove none
of them are used past their expiration.

## 3. Non-lexical lifetimes (NLL)

Early Rust tied a borrow's lifetime to its *lexical scope* - the `{ }`
block it was declared in - which was often more conservative than
necessary and rejected obviously-fine code. Modern Rust (post-2018)
uses non-lexical lifetimes: a borrow's actual lifetime is only as long
as it's *actually used*, computed via real control-flow analysis, not
just "until the closing brace." This is why some code that used to be
a compile error in old Rust just works today with no changes.

Warden doesn't need any of this, because it has no borrows to have
lifetimes in the first place - but it's worth naming as the specific
piece of engineering that made Rust's real borrow checker feel much
less hostile than its reputation from years ago.

## 4. Aliasing rules

The actual *rule* the borrow checker enforces isn't really about
"moves" at all - it's: **at any given point, a value is either
readable by any number of parties, or writable by exactly one party,
never both at once.** Ownership and moves are how Rust *avoids ever
needing to check this rule for owned values that never get borrowed* -
if you can't have two names for the same data without an explicit
borrow, most aliasing bugs are structurally impossible before the
checker even has to reason about them. Warden gets this "for free" the
same way: since it has no borrowing, there's no aliasing to reason
about, ever.

## Two smaller, more mundane gaps

- **No mutation of existing bindings.** Warden's `let` always creates
  a fresh binding; there's no `x = new_value;` for a variable that
  already exists. The practical consequence: `while` loop conditions
  can never change, so every loop in Warden is either infinite or runs
  zero times (see `docs/04-interpreter.md`). Fixing this means adding
  real assignment — which immediately reopens the aliasing question
  above ("if I assign through this name, does anything else refer to
  the same value?").
- **No function return values.** Functions exist only for `print`
  side effects. Adding real returns is mechanical (not conceptually
  hard) but was out of scope for keeping the project finishable.

## The honest takeaway

Every one of these gaps is the same shape: Warden gets to skip it
*specifically because* it never lets two names refer to the same
value at the same time. The instant you add borrowing - letting code
look at a value without taking it - you also inherit lifetimes,
non-lexical lifetime inference, and the full aliasing-rule enforcement
that makes the borrow checker what it actually is. Move-checking alone,
which is what Warden implements, is real, but it's the easy 20%.
