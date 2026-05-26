//! Clang Output Parser
//! Parses Clang compiler output

use crate::core::{Issue, OutputParser, ParseResult};
use crate::plugins::cpp::parser::CppParser;

pub struct ClangParser {
    inner: CppParser,
}

impl ClangParser {
    pub fn new() -> Self {
        Self {
            inner: CppParser::with_clang(),
        }
    }
}

impl Default for ClangParser {
    fn default() -> Self {
        Self::new()
    }
}

impl OutputParser for ClangParser {
    fn parse(&self, output: &str) -> ParseResult<Vec<Issue>> {
        // Use OutputParser::parse explicitly to avoid ambiguity
        <CppParser as OutputParser>::parse(&self.inner, output)
    }
}


