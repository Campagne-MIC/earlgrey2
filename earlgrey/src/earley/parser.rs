#![deny(warnings)]

use super::grammar::{Grammar, Symbol};
use super::spans::{Span, SpanSource};
use std::collections::HashSet;
use std::fmt::Debug;
use std::rc::Rc;

pub struct EarleyParser {
    pub grammar: Grammar,
}

#[derive(Debug)]
pub struct ParseTrees(pub Vec<Rc<Span>>);

/// Structured recognition failure from [`EarleyParser::parse_with_diagnostics`].
/// Positions index input tokens, not bytes or characters.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct ParseDiagnostic {
    /// Zero-based index of the first token that could not advance the chart,
    /// or the input token count when a valid prefix ended too soon.
    pub token_index: usize,
    /// Sorted, deduplicated names of terminals expected at that position.
    pub expected: Vec<String>,
    /// Whether the prefix before that token already completed the start rule.
    pub expected_end_of_input: bool,
}

impl std::fmt::Display for ParseDiagnostic {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut expected = self.expected.clone();
        if self.expected_end_of_input {
            expected.push("end of input".to_string());
        }
        write!(
            f,
            "Parse Error: No Rule completes at token {}; expected: {}",
            self.token_index,
            expected.join(", ")
        )
    }
}

impl std::error::Error for ParseDiagnostic {}

///////////////////////////////////////////////////////////////////////////////

impl EarleyParser {
    pub fn new(grammar: Grammar) -> EarleyParser {
        EarleyParser { grammar }
    }

    /// Build new `Completion` items based on `trigger` item having completed.
    /// When an item is completed it advances all items in the same starting
    /// StateSet whose next symbol matches its rule name.
    fn completions<'r>(
        starting_stateset: impl Iterator<Item = &'r Rc<Span>> + 'r,
        trigger: &'r Rc<Span>,
        complete_pos: usize,
    ) -> Box<dyn Iterator<Item = Span> + 'r> {
        assert!(
            trigger.complete(),
            "Incomplete `trigger` used for completions"
        );
        Box::new(
            starting_stateset
                .filter(move |span| match span.next_symbol() {
                    Some(Symbol::NonTerm(name)) => name == &trigger.rule.head,
                    _ => false,
                })
                .map(move |span| {
                    Span::extend(
                        SpanSource::Completion(span.clone(), trigger.clone()),
                        complete_pos,
                    )
                }),
        )
    }

    /// Build new `Scan` items for items in the current stateset whose next
    /// symbol is a Terminal that matches the input lexeme ahead in the stream.
    fn scans<'r>(
        current_stateset: impl Iterator<Item = &'r Rc<Span>> + 'r,
        lexeme: &'r str,
        end: usize,
    ) -> impl Iterator<Item = Rc<Span>> + 'r {
        current_stateset
            // check span's next symbol is a temrinal that scans lexeme
            .filter(move |span| span.next_symbol().is_some_and(|s| s.matches(lexeme)))
            .map(move |span| {
                Rc::new(Span::extend(
                    SpanSource::Scan(span.clone(), lexeme.to_string()),
                    end,
                ))
            })
    }

    pub fn parse<T>(&self, tokenizer: T) -> Result<ParseTrees, String>
    where
        T: Iterator,
        T::Item: Debug + AsRef<str>,
    {
        self.parse_with_diagnostics(tokenizer)
            .map_err(|_| "Parse Error: No Rule completes".to_string())
    }

    /// Parse with the same recognition and forest construction as [`Self::parse`],
    /// retaining the failed chart position and its expected terminal names.
    pub fn parse_with_diagnostics<T>(&self, mut tokenizer: T) -> Result<ParseTrees, ParseDiagnostic>
    where
        T: Iterator,
        T::Item: Debug + AsRef<str>,
    {
        // Populate S0, add items for each rule matching the start symbol
        let s0: HashSet<_> = self
            .grammar
            .rules
            .iter()
            .filter(|rule| rule.head == self.grammar.start)
            .map(|rule| Rc::new(Span::new(rule, 0)))
            .collect();

        let mut statesets = vec![s0];
        let mut first_failure = None;

        // New statesets are generated from input stream (Scans)
        for idx in 0.. {
            // Predict/Complete until no new Spans are added to the StateSet
            // Instead of looping we could pre-populate completions of nullable symbols
            loop {
                let new_items: Vec<_> = statesets[idx]
                    .iter()
                    .flat_map(|trigger| {
                        let next_sym = trigger.next_symbol();
                        if let Some(Symbol::NonTerm(next_terminal)) = next_sym {
                            // Prediction: Build new items from `next_terminal` of some Symbol
                            Box::new(
                                self.grammar
                                    .rules
                                    .iter()
                                    .filter(|rule| rule.head == *next_terminal)
                                    .map(move |rule| Span::new(rule, idx)),
                            )
                        } else if trigger.complete() {
                            assert!(next_sym.is_none(), "Expected next symbol to be None");
                            EarleyParser::completions(statesets[trigger.start].iter(), trigger, idx)
                        } else {
                            // Scan items populate next stateset only when done with current state
                            assert!(matches!(next_sym, Some(&Symbol::Term(_, _))));
                            Box::new(std::iter::empty())
                        }
                    })
                    .collect();
                let stateset = statesets.get_mut(idx).unwrap();
                let prev_len = stateset.len();
                // Add new items to the current stateset merging existing ones
                for new_item in new_items {
                    if let Some(existent) = stateset.get(&new_item) {
                        existent.merge_sources(new_item);
                    } else {
                        stateset.insert(Rc::new(new_item));
                    }
                }
                // do precitions/completions until expansions are exhausted
                if prev_len == stateset.len() {
                    break;
                }
            }
            // Build Si+1 with items in the current state that accept the next token
            if let Some(lexeme) = tokenizer.next() {
                let next: HashSet<_> =
                    EarleyParser::scans(statesets[idx].iter(), lexeme.as_ref(), idx + 1).collect();
                if next.is_empty() && first_failure.is_none() {
                    first_failure = Some(self.diagnostic(&statesets[idx], idx));
                }
                statesets.push(next);
            } else {
                break;
            }
        }

        // debug StateSets
        if cfg!(feature = "debug") {
            for (idx, stateset) in statesets.iter().enumerate() {
                eprintln!("=== StateSet {} ===", idx);
                stateset
                    .iter()
                    .inspect(|item| {
                        let src = item
                            .sources()
                            .iter()
                            .map(|bp| format!("{:?}", bp))
                            .collect::<Vec<_>>()
                            .join(", ");
                        eprintln!("{:?} -- SRC: {}", item, src);
                    })
                    .count();
            }
        }

        // Check that at least one item is a. complete, b. starts at the idx 0,
        // and c. the name of the rule matches the starting symbol.
        // It spans the whole input because we search at the last stateset
        let final_index = statesets.len() - 1;
        let final_stateset = statesets.pop().expect("No Statesets (even s0)");
        let parse_trees: Vec<_> = final_stateset
            .iter()
            .filter(|item| {
                item.start == 0 && item.complete() && item.rule.head == self.grammar.start
            })
            .cloned()
            .collect();
        if parse_trees.is_empty() {
            return Err(
                first_failure.unwrap_or_else(|| self.diagnostic(&final_stateset, final_index))
            );
        }
        if cfg!(feature = "debug") {
            eprintln!("=== Parse Chart ===");
            for t in &parse_trees {
                eprintln!("{}", t.stringify(0));
            }
        }
        Ok(ParseTrees(parse_trees))
    }

    fn diagnostic(&self, stateset: &HashSet<Rc<Span>>, token_index: usize) -> ParseDiagnostic {
        let mut expected: Vec<_> = stateset
            .iter()
            .filter_map(|item| item.next_symbol())
            .filter(|symbol| symbol.is_terminal())
            .map(|symbol| symbol.name().to_string())
            .collect();
        expected.sort();
        expected.dedup();
        ParseDiagnostic {
            token_index,
            expected,
            expected_end_of_input: stateset.iter().any(|item| {
                item.start == 0 && item.complete() && item.rule.head == self.grammar.start
            }),
        }
    }
}
