// Copyright © 2026 Campagne Machine Intelligence Consulting LLC.
// Distributed under the MIT license; see LICENSE.

use super::{EarleyParser, GrammarBuilder, PrefixRecognition};

fn parser() -> EarleyParser {
    EarleyParser::new(
        GrammarBuilder::default()
            .nonterm("Expr")
            .nonterm("Atom")
            .terminal("number", |s| {
                !s.is_empty() && s.chars().all(|c| c.is_ascii_digit())
            })
            .terminal("plus", |s| s == "+")
            .terminal("open", |s| s == "(")
            .terminal("close", |s| s == ")")
            .rule("Expr", &["Expr", "plus", "Atom"])
            .rule("Expr", &["Atom"])
            .rule("Atom", &["number"])
            .rule("Atom", &["open", "Expr", "close"])
            .into_grammar("Expr")
            .unwrap(),
    )
}

#[test]
fn every_prefix_exposes_the_closed_chart_and_owned_semantics() {
    let p = parser();
    let r = p.recognize_prefix(["(", "1", "+", "2"].into_iter());
    assert!(r.stopped.is_none());
    assert_eq!(r.positions.len(), 5);
    assert_eq!(r.positions[0].expected, ["number", "open"]);
    assert!(r.positions[0].predicted.contains(&"Atom".to_owned()));
    assert!(!r.positions[0].accepts);
    assert_eq!(r.positions[4].expected, ["close", "plus"]);
    assert!(!r.positions[4].accepts);
    assert!(r.positions[2].completed.contains(&("Atom".into(), 1)));
    assert!(
        r.positions[3]
            .active_rules
            .iter()
            .any(|r| r.rule == "Expr -> Expr plus Atom" && r.dot == 2 && r.start == 1)
    );
    assert!(p.recognize_prefix(["1"].into_iter()).positions[1].accepts);
    fn owned_send_sync<T: Send + Sync + 'static>() {}
    owned_send_sync::<PrefixRecognition>();
}

#[test]
fn first_failure_matches_parser_and_does_not_consume_the_tail() {
    let p = parser();
    let mut tokens = ["1", "+", ")", "unread"].into_iter();
    let r = p.recognize_prefix(tokens.by_ref());
    assert_eq!(tokens.next(), Some("unread"));
    let stopped = r.stopped.unwrap();
    assert_eq!(stopped.token_index, 2);
    assert_eq!(r.positions.len(), 3);
    let error = p
        .parse_with_diagnostics(["1", "+", ")", "unread"].into_iter())
        .unwrap_err();
    assert_eq!(stopped, error);
    let mut full_parse_tokens = ["1", "+", ")", "consumed"].into_iter();
    assert!(
        p.parse_with_diagnostics(full_parse_tokens.by_ref())
            .is_err()
    );
    assert_eq!(full_parse_tokens.next(), None);
}

#[test]
fn every_mistyped_fixture_prefix_agrees_with_the_existing_failure_diagnostic() {
    let p = parser();
    for input in [vec!["1", "+", "2"], vec!["(", "1", "+", "2", ")"]] {
        for i in 0..=input.len() {
            let mut bad = input[..i].to_vec();
            bad.push("?");
            let r = p.recognize_prefix(bad.iter());
            let e = p.parse_with_diagnostics(bad.iter()).unwrap_err();
            assert_eq!(r.stopped, Some(e));
        }
    }
}

#[test]
fn nullable_recursive_and_ambiguous_rules_have_deterministic_unique_facts() {
    let p = EarleyParser::new(
        GrammarBuilder::default()
            .nonterm("S")
            .nonterm("A")
            .nonterm("B")
            .terminal("x", |s| s == "x")
            .rule("S", &["A"])
            .rule("S", &["B"])
            .rule("A", &["A", "x"])
            .rule("A", &[])
            .rule("B", &["x"])
            .into_grammar("S")
            .unwrap(),
    );
    let baseline = p.recognize_prefix(["x"].into_iter());
    assert!(baseline.positions[0].accepts);
    assert!(baseline.positions[1].accepts);
    assert!(baseline.positions[0].completed.contains(&("A".into(), 0)));
    for _ in 0..20 {
        assert_eq!(p.recognize_prefix(["x"].into_iter()), baseline);
    }
    for position in &baseline.positions {
        assert!(position.active_rules.windows(2).all(|r| r[0] < r[1]));
        assert!(position.expected.windows(2).all(|r| r[0] < r[1]));
    }
    assert!(p.parse_with_diagnostics(["x"].into_iter()).is_ok());
}
