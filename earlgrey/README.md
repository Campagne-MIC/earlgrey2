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
