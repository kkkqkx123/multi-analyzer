//! MSVC Output Parser
//! Parses Microsoft Visual C++ compiler output

use crate::core::{Issue, OutputParser, ParseResult};
use crate::plugins::cpp::parser::CppParser;

pub struct MsvcParser {
    inner: CppParser,
}

impl MsvcParser {
    pub fn new() -> Self {
        Self {
            inner: CppParser::with_msvc(),
        }
    }
}

impl Default for MsvcParser {
    fn default() -> Self {
        Self::new()
    }
}

impl OutputParser for MsvcParser {
    fn parse(&self, output: &str) -> ParseResult<Vec<Issue>> {
        // Use OutputParser::parse explicitly to avoid ambiguity
        <CppParser as OutputParser>::parse(&self.inner, output)
    }
}


