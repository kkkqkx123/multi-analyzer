//! C++ base module
//! Provides shared types and parsing logic for C++ compilers

pub mod clang;
pub mod clang_check;
pub mod clang_format;
pub mod clang_tidy;
pub mod cmake;
pub mod gcc;
pub mod iwyu;
pub mod msvc;
pub mod parser;

// Note: CppParser and CompilerType are available via cpp::parser module directly
pub use clang::ClangAnalyzer;
pub use clang_check::ClangCheckAnalyzer;
pub use clang_format::ClangFormatAnalyzer;
pub use clang_tidy::ClangTidyAnalyzer;
pub use cmake::CMakeAnalyzer;
pub use gcc::GccAnalyzer;
pub use iwyu::IwyuAnalyzer;
pub use msvc::MsvcAnalyzer;
