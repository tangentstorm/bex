# Bex Standard Notations (issue #10)

This document describes the implementation of the standard notation vocabulary
proposed in [issue #10](https://github.com/tangentstorm/bex/issues/10): a
common way to spell NIDs, an infix algebraic grammar and a bracket-notation
function-application syntax built on top of that vocabulary, RPN verbs for the
bex shell, and a JSON encoding of the same expressions.

(There is also an earlier exploratory design note at `doc/notation.org`. This
file documents what was actually implemented, in `src/notation.rs`, and is the
authoritative reference going forward.)

## NID vocabulary

Almost all of the nid spellings in the issue were already implemented by
`NID::from_str`/`Display` in `src/nid.rs` before this change:

| form | meaning |
|---|---|
| `O`, `I` | constant false / true |
| `xN`, `vN` | input variable / virtual variable (`N` = uppercase hex) |
| `!xN`, `!vN` | inverted literal |
| `tBBB...B` | binary truth table, `2^n` bits, `1 <= n <= 5` |
| `fN.MMM...M` | hex truth table, `N` inputs, `2^N` bits of hex payload |
| `fH` | shorthand for `f2.H` |
| `xNN.MMMM`, `vNN.MMMM` | VHL/BDD nid: branching variable + hilocache index |
| `!xNN.MMMM` | inverted VHL/BDD nid |
| `@MMMM` | AST nid at index `MMMM` |
| `!@MMMM` | inverted AST nid |
| `T{x3,x7:1110}` | named-variable table (already implemented; not in the issue text but exercised by existing tests) |

`src/notation.rs` does not re-implement any of this parsing; it wraps
`NID::from_str`/`Display` in a new `ParsedNid` type (see below) and fixes the
one real gap: `t`-notation now also accepts `O`/`I` as alternate spellings for
`0`/`1` (e.g. `tOIIO == t0110`), which the existing parser rejected.

### `ParsedNid`

`ParsedNid { ns: Option<u32>, nid: NID }` adds the numeric namespace prefix
(`N:`) from the issue ("if dealing with multiple bases, a numeric namespace
`N:` can appear as a prefix"), which nothing in the codebase parsed before.
`ParsedNid::from_str`/`Display` guarantee `parse(display(x)) == x` for every
nid form plus the namespace prefix.

## Decisions

Where the issue text was ambiguous or self-contradictory, this is what was
implemented, and why:

- **`fH` shorthand.** The issue says "`fN` is `f2.N`". Read literally, and
  it's also the only reading consistent with the existing implementation and
  tests (`fA` == `f2.A`, a 2-input table addressed by a single hex digit).

- **VHL `!` typo.** The issue lists `!xNN.MMMM` as a VHL nid *twice* ("`!xNN.MMMM`
  is a VHL NID... `!xNN.MMMM` is the same but inverted"), clearly meaning to
  give the non-inverted form (`xNN.MMMM`) first. This is what `NID::from_str`
  already implements: `xNN.MMMM`/`vNN.MMMM` for the base form, `!` as a
  prefix for the inverted form, exactly like every other nid spelling.

- **`t`-notation `O`/`I` alternates.** The issue allows `O`/`I` as alternates
  for `0`/`1` inside `tBBB...B` (e.g. `tOIIO`). This was not previously
  accepted; `src/nid.rs`'s `t` parse arm now maps `O`->`'0'` and `I`->`'1'`
  before validating/parsing the bit string, so `tOIIO == t0110`. The
  canonical `Display` form is still plain binary (`t0110`); we don't try to
  preserve the O/I spelling through a round trip, only the resulting NID
  value (`parse(display(parse(s))) == parse(s)`, not textual identity).

- **Lowercase hex.** The issue specifies uppercase hex for `xN`/`vN`/`@MMMM`/
  hex table payloads. `NID::from_str` already rejected lowercase (`xeb`,
  `f2.f`, `@1a`) with an explicit error; we kept that (rather than silently
  normalizing) and added tests for it, since normalizing would make `Display`
  and `FromStr` disagree about the canonical spelling.

- **Namespace prefix placement.** `N:` is treated as the outermost prefix,
  before any leading `!`, e.g. `2:!x0`, not `!2:x0`. `ParsedNid::from_str`
  only treats a `:` as a namespace separator when everything before it is a
  run of ASCII digits; this also lets it coexist with the named-table form
  (`T{x3,x7:1110}`), which contains a `:` that is *not* a namespace prefix,
  since the text before that `:` (`T{x3,x7`) isn't all digits.

- **`t1[x y]` example.** The issue's own bracket-notation example
  (`` `t1[x y]` is the same as `x *. y` ``) doesn't parse as a `t`-nid (a
  table nid needs 2, 4, 8, 16, or 32 bits, never a single bit). This looks
  like an editing artifact referring to `f1` (== `f2.1`, dyadic AND). All
  examples/tests here use canonical forms like `t0001[x0 x1]` or
  `f1[x0 x1]` instead.

- **Bracket notation on AST nids.** Disallowed, per the option the issue
  itself offers ("we could just disallow this operation for AST nodes until
  someone actually wants it"). `apply_bracket` returns
  `Err("bracket substitution on AST nids is not supported yet")` for any
  `@MMMM` nid. This also matches the codebase as found:
  `RawASTBase::sub` is `todo!("ast::sub")`, i.e. general substitution isn't
  implemented for ASTs yet either.

- **Bracket notation on VHL/BDD nids ("top-down" substitution).** Implemented
  generically over any `Base` impl (`apply_vhl_sub` in `src/notation.rs`) as:
  repeatedly take the *current* top branching variable (`cur.vid()`) and
  replace it with the next argument via `Base::sub`, in argument order. This
  matches the issue's description ("if you pass two arguments to a BDD
  branching on `x5`, you would generally get back the NID for a new BDD
  branching on `x3`") without needing to know in advance which variables
  exist below the top of the diagram. If there are more arguments than
  variables remaining (i.e. substitution already reduced the nid to a
  literal), the extra arguments are silently ignored rather than erroring;
  this wasn't specified either way, and ignoring keeps `f[a b c]` valid even
  when the caller doesn't know exactly how deep `f` branches.

- **Bracket notation on table nids.** Table application is a real (non-`Base`
  -dependent) computation: it recurses over `NidFun::when` bit-by-bit, from
  the topmost input position down to 0, combining with `Base::ite` at each
  step (`apply_table`). Arity must exactly equal the argument count (an
  explicit `Err`, not silent truncation/padding), since a table's shape is
  fixed and unambiguous, unlike a BDD's.

- **Assignment (`:`) and precedence.** The issue only says `:` is "probably"
  the assignment operator and gives no precedence for it relative to the
  other operators. It's implemented as the outermost, lowest-precedence
  production (lower than `?:`): `parse_expr` first checks whether the input
  is `IDENT ':' ...` (an identifier that is *not itself* a valid nid
  spelling, followed by `:`) and if so parses it as `Assign(name, expr)`,
  otherwise falls through to the `?:`-and-down precedence chain. Requiring
  the assignment target to not already parse as a nid means you can't shadow
  `x0`, `O`, etc. as a variable name — this seemed safer than allowing it.
  Assignments live in a caller-supplied `scope: HashMap<String, NID>` passed
  to `eval_expr`, not in the `Base`'s own tag table; call `Base::tag`/`get`
  separately if you want a binding to persist in the base itself.

- **`=` non-associativity.** `x0 = x1 = x2` is rejected with an explicit
  error ("`'=' is non-associative; use parentheses to chain comparisons`");
  `(x0 = x1) = x2` parses fine, since parentheses make the grouping
  unambiguous.

- **Named-table form (`T{...}`) inside the infix/RPN grammars.** Not
  supported as a *token* inside `parse_expr`'s tokenizer: its embedded `:`
  (`T{x3,x7:1110}`) would be ambiguous with assignment/ternary `:` without a
  much more context-sensitive tokenizer, and it wasn't in the issue's
  vocabulary to begin with. `T{...}` still works everywhere a bare `NID`/
  `ParsedNid` is parsed directly (i.e. via `NID::from_str`/`ParsedNid::from_str`,
  including as a single JSON string).

- **JSON representation of assignment.** Not supported (`to_json` returns
  `Err` for `Expr::Assign`); the issue's JSON section never mentions
  assignment, only expression trees, so there was nothing to standardize.

- **JSON's odd examples (`"nx8.324"`, `"X3"`).** Treated as loose
  illustrations rather than literal spellings to support, per the task brief;
  `to_json`/`from_json` only produce/accept the canonical vocabulary above.

## Infix algebraic grammar

`notation::parse_expr(&str) -> Result<Expr, String>` parses:

```
unary   !                      (highest)
        &
        %
        |
        < /
        =                      (non-associative)
        ?:                     (right-associative, lowest)
```

plus `name : expr` assignment (lower than everything above) and parentheses.
Bracket-notation application (`f[a b c]`, args are whitespace-separated
unary-level expressions) is available at every precedence level as a postfix
operation on any primary expression.

`notation::eval_expr(base: &mut impl Base, expr: &Expr, scope: &mut HashMap<String, NID>) -> Result<NID, String>`
builds the parsed expression in a `Base` using `and`/`or`/`xor`/`ite`
directly; `=`, `<`, `/` have no dedicated `Base` method and are built from
those primitives per the issue's own definitions (`x=y` is `!(x^y)`, `x<y` is
`y & !x`, `x/y` is `!x | y`).

## Bracket notation

`notation::apply_bracket(base: &mut impl Base, n: NID, args: &[NID]) -> Result<NID, String>`
implements `n[args...]`:
- table nid: apply as a function (arity must equal `args.len()`).
- VHL/BDD nid (has an index but isn't a table or AST nid): top-down
  substitution, one argument per branching level (see Decisions above).
- AST nid (`@MMMM`): `Err("bracket substitution on AST nids is not supported yet")`.
- plain variable or constant (no index to substitute into): `Err`.

## bex-shell (RPN)

`examples/shell/bex-shell.rs` already parsed every nid spelling via
`NID::from_str` (any word that doesn't match a shell verb or a stored binding
falls through to `NID::from_str`), and already spelled `!` as "not". Added
postfix verbs for the rest of the operator vocabulary, aliased alongside the
existing named words (`and`/`xor`/`or` keep working):

```
"&"  => and         "%"  => xor        "|"  => or
"="  => equal       "<"  => less-than  "/"  => implies
"?:" => if-then-else
```

`=` was checked against the existing word table first and did not collide
with anything. Bracket-notation application is *not* wired into the shell:
the issue itself flags that RPN would need "some extra sigil indicating that
the nid is meant to be applied rather than simply added to the stack," and
doesn't propose one, so this is left as a documented gap rather than guessed
at. `notation::apply_bracket` is available as a library function for any
caller (including a future shell version) that wants it.

## JSON notation

`notation::to_json(&Expr) -> Result<JsonValue, String>` /
`notation::from_json(&JsonValue) -> Result<Expr, String>` (using the `json`
crate, already a dependency) implement:
- primitive nids/names as bare JSON strings (`"x0"`, `"O"`, ...).
- compounds as `{"op": [args]}` for `op` in `| & % = < /`, and `{"?:": [i,t,e]}`.
- `~` (not) elides the list wrapper: `{"~": "x3"}`.
- a nid or name used as an operator key means bracket-notation application:
  `{"t0001": ["x0", "x1"]}`.

## Known limitations / out of scope

- Bracket-notation application is not wired into the bex shell (see above).
- The named-variable table form `T{...}` cannot appear as a token inside the
  infix/RPN grammars (only as a standalone nid literal) because of the `:`
  ambiguity described above.
- The numeric namespace prefix (`N:`) is parsed and round-tripped by
  `ParsedNid`, but nothing downstream (the grammar, `eval_expr`, JSON) acts on
  it yet — bex doesn't currently have a notion of "multiple simultaneous
  bases" for it to select between, so `Expr`/JSON drop it after parsing a
  primary nid.
- The alternate "ITE via bracket notation on a variable" form the issue
  mentions in passing (`x[g h]` as a synonym for `x ? g : h`) is not
  implemented; `?:` already covers if/then/else.
