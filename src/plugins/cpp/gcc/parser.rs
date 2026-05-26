//! GCC Output Parser
//! Parses GCC compiler output

use crate::core::{Issue, OutputParser, ParseResult};
use crate::plugins::cpp::parser::CppParser;

pub struct GccParser {
    inner: CppParser,
}

impl GccParser {
    pub fn new() -> Self {
        Self {
            inner: CppParser::with_gcc(),
        }
    }
}

impl Default for GccParser {
    fn default() -> Self {
        Self::new()
    }
}

impl OutputParser for GccParser {
    fn parse(&self, output: &str) -> ParseResult<Vec<Issue>> {
        // Use OutputParser::parse explicitly to avoid ambiguity
        <CppParser as OutputParser>::parse(&self.inner, output)
    }
}


