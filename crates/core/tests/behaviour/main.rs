//! Behaviour tests of the project sync: the rules over a snapshot, the use
//! case over the in-memory Project, the reply parsers and the command line.
//! None of them needs `gh`, a network or a login.

pub mod support;

mod audit;
mod audit_rules;
mod facts;
mod gate;
mod issues_parse;
mod moves;
mod parse;
mod queries;
mod references;
mod release;
mod texts_parse;
mod transitions;
mod violation;
