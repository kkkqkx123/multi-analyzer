//! plug-in module
//! Analyzer implementations containing various technology stacks

pub mod cargo;
pub mod cpp;
pub mod dotnet;
pub mod go;
pub mod java;
pub mod npm;
pub mod python;
pub mod ruby;

use crate::core::PluginRegistry;

/// Create and configure the plug-in registry
pub fn create_registry() -> PluginRegistry {
    let mut registry = PluginRegistry::new();

    // Register Cargo Analyzer
    registry.register(Box::new(cargo::CargoAnalyzer::new()));

    // Register NPM Analyzer
    registry.register(Box::new(npm::NpmAnalyzer::npm()));
    registry.register(Box::new(npm::NpmAnalyzer::pnpm()));
    registry.register(Box::new(npm::NpmAnalyzer::yarn()));

    // Registering the Python Analyzers
    registry.register(Box::new(python::MypyAnalyzer::new()));
    registry.register(Box::new(python::PytestAnalyzer::new()));

    // Registering the Java Analyzers
    registry.register(Box::new(java::MavenAnalyzer::new()));
    registry.register(Box::new(java::GradleAnalyzer::new()));

    // Registering the Go Analyzer
    registry.register(Box::new(go::GoAnalyzer::new()));

    // Registering the .NET Analyzer
    registry.register(Box::new(dotnet::DotnetAnalyzer::new()));

    // Registering the Ruby Analyzer
    registry.register(Box::new(ruby::RubyAnalyzer::new()));

    // Registering the C++ Analyzers
    registry.register(Box::new(cpp::CMakeAnalyzer::new()));
    registry.register(Box::new(cpp::GccAnalyzer::new()));
    registry.register(Box::new(cpp::ClangAnalyzer::new()));
    registry.register(Box::new(cpp::MsvcAnalyzer::new()));

    registry
}
