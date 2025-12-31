//! Language-specific node type mappings for AST parsing.
//!
//! Maps tree-sitter node types to our NodeKind enum for each supported language.

use std::collections::HashMap;

use crate::ast_engine::parser::NodeKind;

/// Get the node type mappings for a language.
pub fn get_node_types(language: &str) -> HashMap<&'static str, NodeKind> {
    match language {
        "python" => python_node_types(),
        "javascript" | "jsx" => javascript_node_types(),
        "typescript" | "tsx" => typescript_node_types(),
        "go" => go_node_types(),
        "rust" => rust_node_types(),
        "java" => java_node_types(),
        "c" => c_node_types(),
        "cpp" => cpp_node_types(),
        "ruby" => ruby_node_types(),
        _ => HashMap::new(),
    }
}

/// Python node type mappings.
fn python_node_types() -> HashMap<&'static str, NodeKind> {
    [
        ("function_definition", NodeKind::Function),
        ("decorated_definition", NodeKind::Function),
        ("class_definition", NodeKind::Class),
        ("import_statement", NodeKind::Import),
        ("import_from_statement", NodeKind::Import),
        ("comment", NodeKind::Comment),
        ("module", NodeKind::Module),
    ]
    .into_iter()
    .collect()
}

/// JavaScript node type mappings.
fn javascript_node_types() -> HashMap<&'static str, NodeKind> {
    [
        ("function_declaration", NodeKind::Function),
        ("function_expression", NodeKind::Function),
        ("arrow_function", NodeKind::Function),
        ("method_definition", NodeKind::Method),
        ("generator_function_declaration", NodeKind::Function),
        ("class_declaration", NodeKind::Class),
        ("class_expression", NodeKind::Class),
        ("import_statement", NodeKind::Import),
        ("export_statement", NodeKind::Other),
        ("comment", NodeKind::Comment),
        ("lexical_declaration", NodeKind::Variable),
        ("variable_declaration", NodeKind::Variable),
    ]
    .into_iter()
    .collect()
}

/// TypeScript node type mappings (extends JavaScript).
fn typescript_node_types() -> HashMap<&'static str, NodeKind> {
    let mut types = javascript_node_types();
    types.extend([
        ("interface_declaration", NodeKind::Interface),
        ("type_alias_declaration", NodeKind::Other),
        ("enum_declaration", NodeKind::Enum),
        ("abstract_class_declaration", NodeKind::Class),
        ("module", NodeKind::Module),
        ("ambient_declaration", NodeKind::Other),
    ]);
    types
}

/// Go node type mappings.
fn go_node_types() -> HashMap<&'static str, NodeKind> {
    [
        ("function_declaration", NodeKind::Function),
        ("method_declaration", NodeKind::Method),
        ("type_declaration", NodeKind::Other),
        ("type_spec", NodeKind::Other),
        ("struct_type", NodeKind::Struct),
        ("interface_type", NodeKind::Interface),
        ("import_declaration", NodeKind::Import),
        ("import_spec", NodeKind::Import),
        ("const_declaration", NodeKind::Constant),
        ("const_spec", NodeKind::Constant),
        ("var_declaration", NodeKind::Variable),
        ("var_spec", NodeKind::Variable),
        ("comment", NodeKind::Comment),
        ("package_clause", NodeKind::Module),
    ]
    .into_iter()
    .collect()
}

/// Rust node type mappings.
fn rust_node_types() -> HashMap<&'static str, NodeKind> {
    [
        ("function_item", NodeKind::Function),
        ("impl_item", NodeKind::Impl),
        ("struct_item", NodeKind::Struct),
        ("enum_item", NodeKind::Enum),
        ("trait_item", NodeKind::Trait),
        ("mod_item", NodeKind::Module),
        ("use_declaration", NodeKind::Import),
        ("const_item", NodeKind::Constant),
        ("static_item", NodeKind::Variable),
        ("type_item", NodeKind::Other),
        ("macro_definition", NodeKind::Function),
        ("line_comment", NodeKind::Comment),
        ("block_comment", NodeKind::Comment),
        ("attribute_item", NodeKind::Decorator),
        ("inner_attribute_item", NodeKind::Decorator),
    ]
    .into_iter()
    .collect()
}

/// Java node type mappings.
fn java_node_types() -> HashMap<&'static str, NodeKind> {
    [
        ("method_declaration", NodeKind::Method),
        ("constructor_declaration", NodeKind::Method),
        ("class_declaration", NodeKind::Class),
        ("interface_declaration", NodeKind::Interface),
        ("enum_declaration", NodeKind::Enum),
        ("annotation_type_declaration", NodeKind::Other),
        ("import_declaration", NodeKind::Import),
        ("field_declaration", NodeKind::Variable),
        ("constant_declaration", NodeKind::Constant),
        ("line_comment", NodeKind::Comment),
        ("block_comment", NodeKind::Comment),
        ("package_declaration", NodeKind::Module),
        ("annotation", NodeKind::Decorator),
        ("marker_annotation", NodeKind::Decorator),
    ]
    .into_iter()
    .collect()
}

/// C node type mappings.
fn c_node_types() -> HashMap<&'static str, NodeKind> {
    [
        ("function_definition", NodeKind::Function),
        ("declaration", NodeKind::Variable),
        ("struct_specifier", NodeKind::Struct),
        ("enum_specifier", NodeKind::Enum),
        ("union_specifier", NodeKind::Struct),
        ("preproc_include", NodeKind::Import),
        ("preproc_def", NodeKind::Constant),
        ("comment", NodeKind::Comment),
        ("type_definition", NodeKind::Other),
    ]
    .into_iter()
    .collect()
}

/// C++ node type mappings (extends C).
fn cpp_node_types() -> HashMap<&'static str, NodeKind> {
    let mut types = c_node_types();
    types.extend([
        ("class_specifier", NodeKind::Class),
        ("template_declaration", NodeKind::Other),
        ("namespace_definition", NodeKind::Module),
        ("using_declaration", NodeKind::Import),
        ("alias_declaration", NodeKind::Other),
    ]);
    types
}

/// Ruby node type mappings.
fn ruby_node_types() -> HashMap<&'static str, NodeKind> {
    [
        ("method", NodeKind::Method),
        ("singleton_method", NodeKind::Method),
        ("class", NodeKind::Class),
        ("module", NodeKind::Module),
        ("call", NodeKind::Other), // require, include, etc.
        ("assignment", NodeKind::Variable),
        ("comment", NodeKind::Comment),
    ]
    .into_iter()
    .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_python_mappings() {
        let types = get_node_types("python");
        assert_eq!(types.get("function_definition"), Some(&NodeKind::Function));
        assert_eq!(types.get("class_definition"), Some(&NodeKind::Class));
        assert_eq!(types.get("import_statement"), Some(&NodeKind::Import));
    }

    #[test]
    fn test_rust_mappings() {
        let types = get_node_types("rust");
        assert_eq!(types.get("function_item"), Some(&NodeKind::Function));
        assert_eq!(types.get("struct_item"), Some(&NodeKind::Struct));
        assert_eq!(types.get("impl_item"), Some(&NodeKind::Impl));
        assert_eq!(types.get("trait_item"), Some(&NodeKind::Trait));
    }

    #[test]
    fn test_typescript_extends_javascript() {
        let types = get_node_types("typescript");
        // Should have JavaScript types
        assert_eq!(types.get("function_declaration"), Some(&NodeKind::Function));
        // Plus TypeScript-specific
        assert_eq!(types.get("interface_declaration"), Some(&NodeKind::Interface));
        assert_eq!(types.get("enum_declaration"), Some(&NodeKind::Enum));
    }

    #[test]
    fn test_unknown_language() {
        let types = get_node_types("unknown");
        assert!(types.is_empty());
    }
}
