#![deny(warnings)]

mod grammar;
pub use grammar::{Grammar, GrammarBuilder};

mod parser;
mod spans;
pub use parser::{EarleyParser, ParseDiagnostic, PrefixPosition, PrefixRecognition, PrefixRule};

mod trees;
pub use trees::{EarleyForest, ForestWalkError};

#[cfg(test)]
mod parser_test;

#[cfg(test)]
mod prefix_test;
