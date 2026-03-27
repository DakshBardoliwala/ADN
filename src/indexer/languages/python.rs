pub const IMPORT_QUERY_STR: &str = r#"
  (import_statement
    name: (dotted_name) @import.name) @import.stmt

  (import_statement
    name: (aliased_import
      name: (dotted_name) @import.name
      alias: (identifier) @import.alias)) @import.stmt

  (import_from_statement
    module_name: [(dotted_name) (relative_import)] @import.from.module
    name: (dotted_name) @import.from.name) @import.from.stmt

  (import_from_statement
    module_name: [(dotted_name) (relative_import)] @import.from.module
    name: (aliased_import
      name: (dotted_name) @import.from.name
      alias: (identifier) @import.from.alias)) @import.from.stmt

  (import_from_statement
    module_name: [(dotted_name) (relative_import)] @import.from.module
    (wildcard_import) @import.from.wildcard) @import.from.stmt
"#;
