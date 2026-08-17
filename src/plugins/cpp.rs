//! C++ base module
//! Provides shared types and parsing logic for C++ compilers

pub mod clang;
pub mod clang_format;
pub mod cmake;
pub mod gcc;
pub mod msvc;
pub mod parser;

// Note: CppParser and CompilerType are available via cpp::parser module directly
pub use clang::ClangAnalyzer;
pub use clang_format::ClangFormatAnalyzer;
pub use cmake::CMakeAnalyzer;
pub use gcc::GccAnalyzer;
pub use msvc::MsvcAnalyzer;
