# Example

```rust
// Full code at examples/ebnftree.rs
fn main() {
  let grammar = r#"
    expr   := expr ('+'|'-') term | term ;
    term   := term ('*'|'/') factor | factor ;
    factor := '-' factor | power ;
    power  := ufact '^' factor | ufact ;
    ufact  := ufact '!' | group ;
    group  := num | '(' expr ')' ;
  "#;

  use std::str::FromStr;
  let grammar = earlgrey::EbnfGrammarParser::new(grammar, "expr")
      .plug_terminal("num", |n| f64::from_str(n).is_ok())
      .into_grammar()
      .unwrap();

  let parser = earlgrey::sexpr_parser(grammar).unwrap();

  for tree in parser(tokenizer(input.chars()))? {
      println!("{}", tree.print());
  }
}
```

`EarleyParser::parse_with_diagnostics` builds the same parse forest as
`EarleyParser::parse`, but returns a `ParseDiagnostic` on recognition failure.
Its zero-based `token_index` identifies the first token that could not advance
(or the token count at a premature end of input); `expected` contains sorted,
unique terminal names from that chart position. `expected_end_of_input` records
whether the preceding prefix already completed the start rule. Callers retain
source spans to translate token positions into line/column locations. The
existing `parse` signature and failure string remain unchanged.

### Sharing grammars across workers

`Grammar` clones share immutable rules and symbols through `Arc`. Terminal
predicates must implement `Fn(&str) -> bool + Send + Sync + 'static`; use
thread-safe captured values when constructing a terminal. `Grammar`, `Rule`
and `Symbol` are `Send + Sync`. Each parser invocation still creates its own
local chart and parse forest; those are not shared between workers.

Rust source compatibility: direct rule/symbol construction uses `Arc` in
place of `Rc`, and both terminal builder methods and `EbnfGrammarParser::plug_terminal` require the new predicate
bounds. Recognition, diagnostics, rule equality and forest traversal keep
their existing behavior.


### Prefix recognition

`EarleyParser::recognize_prefix(tokens)` reports one closed chart position per
accepted token boundary, including the empty prefix. Each position owns sorted,
unique expected terminal names, predicted nonterminal names, completed
`(head, start)` pairs and dotted-rule keys (`rule`, `dot`, `start`). These facts
let a client map its own semantic metadata without retaining a chart or forest.
`accepts` means that the start rule is complete at that position. An incomplete
but viable prefix is not a failure; `stopped` records the first token that cannot
advance, using the same diagnostic as `parse_with_diagnostics`. Recognition
consumes that failed token but does not request the remaining iterator tail.

Parsing and prefix recognition share predict/complete/scan closure. Parsing
still consumes its whole input and builds the same forest; prefix recognition
returns only owned data (`Send + Sync`), never chart nodes or terminal predicates.
The names are parser metadata, not user-facing completion text.
