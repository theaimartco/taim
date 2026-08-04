//! Language descriptions used by the [`Stream`] tokenizer.
//!
//! [`Stream`]: crate::Stream

/// A lexical description of a programming language.
///
/// This is deliberately *not* a full grammar. It is the amount of
/// structure a line-oriented, allocation-free tokenizer needs to colour
/// comments, strings, numbers, keywords, types, and call sites correctly
/// for the fenced code blocks of a document. Anything that needs a parse
/// tree (semantic types, macro expansion, template languages) is out of
/// scope by construction; see the crate documentation.
#[derive(Debug, Clone, Copy)]
pub struct Syntax {
    /// The tokens that select this [`Syntax`], lowercase.
    ///
    /// The first entry is the canonical name.
    pub names: &'static [&'static str],
    /// Prefixes that comment out the rest of the line.
    pub line_comments: &'static [&'static str],
    /// The opening and closing delimiters of a block comment, if any.
    pub block_comment: Option<(&'static str, &'static str)>,
    /// Whether block comments may nest (as in Rust).
    pub nested_block_comments: bool,
    /// Delimiters that open and close a single-line string literal.
    pub strings: &'static [char],
    /// Delimiters that open and close a string literal spanning lines.
    pub multiline_strings: &'static [&'static str],
    /// The character that escapes the next one inside a string literal.
    pub escape: Option<char>,
    /// Words rendered with the keyword colour.
    pub keywords: &'static [&'static str],
    /// Words rendered with the type colour.
    pub types: &'static [&'static str],
    /// The character that introduces an attribute/annotation/preprocessor
    /// token, such as `#` in Rust or `@` in Java and Python.
    pub attribute_sigils: &'static [char],
}

impl Syntax {
    /// The [`Syntax`] used when a language is unknown or absent.
    ///
    /// It still colours numbers and the common `#`/`//` comment prefixes,
    /// which is a strictly better default than a flat block of text.
    pub const PLAIN: Syntax = Syntax {
        names: &["plain", "text", "txt", ""],
        line_comments: &[],
        block_comment: None,
        nested_block_comments: false,
        strings: &[],
        multiline_strings: &[],
        escape: None,
        keywords: &[],
        types: &[],
        attribute_sigils: &[],
    };

    /// Returns the [`Syntax`] registered under the given token.
    ///
    /// Matching is case-insensitive and ignores a leading `.`, so both
    /// `Rust` and `.rs` resolve. Unknown tokens fall back to
    /// [`Syntax::PLAIN`].
    pub fn find(token: &str) -> &'static Syntax {
        let token = token.trim().trim_start_matches('.').to_ascii_lowercase();

        ALL.iter()
            .find(|syntax| syntax.names.contains(&token.as_str()))
            .unwrap_or(&Syntax::PLAIN)
    }

    /// Returns `true` if `word` is a keyword of this [`Syntax`].
    pub fn is_keyword(&self, word: &str) -> bool {
        self.keywords.contains(&word)
    }

    /// Returns `true` if `word` names a built-in type of this [`Syntax`].
    pub fn is_type(&self, word: &str) -> bool {
        self.types.contains(&word)
    }
}

/// Every [`Syntax`] known to this crate, in lookup order.
pub static ALL: &[Syntax] = &[
    RUST,
    C,
    CPP,
    JAVASCRIPT,
    PYTHON,
    GO,
    JAVA,
    JSON,
    TOML,
    YAML,
    MARKUP,
    CSS,
    SHELL,
    SQL,
    Syntax::PLAIN,
];

const C_LIKE_BLOCK: Option<(&str, &str)> = Some(("/*", "*/"));

/// Rust.
pub const RUST: Syntax = Syntax {
    names: &["rust", "rs"],
    line_comments: &["//"],
    block_comment: C_LIKE_BLOCK,
    nested_block_comments: true,
    strings: &['"', '\''],
    multiline_strings: &[],
    escape: Some('\\'),
    keywords: &[
        "as", "async", "await", "break", "const", "continue", "crate", "dyn", "else", "enum",
        "extern", "false", "fn", "for", "if", "impl", "in", "let", "loop", "match", "mod", "move",
        "mut", "pub", "ref", "return", "self", "Self", "static", "struct", "super", "trait",
        "true", "type", "union", "unsafe", "use", "where", "while", "yield",
    ],
    types: &[
        "bool", "char", "f32", "f64", "i8", "i16", "i32", "i64", "i128", "isize", "str", "u8",
        "u16", "u32", "u64", "u128", "usize", "String", "Vec", "Option", "Result", "Box", "Rc",
        "Arc", "HashMap", "HashSet", "BTreeMap", "Cow",
    ],
    attribute_sigils: &['#'],
};

/// C.
pub const C: Syntax = Syntax {
    names: &["c", "h"],
    line_comments: &["//"],
    block_comment: C_LIKE_BLOCK,
    nested_block_comments: false,
    strings: &['"', '\''],
    multiline_strings: &[],
    escape: Some('\\'),
    keywords: &[
        "auto", "break", "case", "const", "continue", "default", "do", "else", "enum", "extern",
        "for", "goto", "if", "inline", "register", "restrict", "return", "sizeof", "static",
        "struct", "switch", "typedef", "union", "volatile", "while",
    ],
    types: &[
        "bool", "char", "double", "float", "int", "long", "short", "signed", "unsigned", "void",
        "size_t", "ssize_t", "uint8_t", "uint16_t", "uint32_t", "uint64_t", "int8_t", "int16_t",
        "int32_t", "int64_t",
    ],
    attribute_sigils: &['#'],
};

/// C++.
pub const CPP: Syntax = Syntax {
    names: &["cpp", "c++", "cc", "hpp", "cxx"],
    line_comments: &["//"],
    block_comment: C_LIKE_BLOCK,
    nested_block_comments: false,
    strings: &['"', '\''],
    multiline_strings: &[],
    escape: Some('\\'),
    keywords: &[
        "auto",
        "break",
        "case",
        "catch",
        "class",
        "concept",
        "const",
        "constexpr",
        "continue",
        "co_await",
        "co_return",
        "co_yield",
        "default",
        "delete",
        "do",
        "else",
        "enum",
        "explicit",
        "export",
        "extern",
        "false",
        "for",
        "friend",
        "if",
        "inline",
        "namespace",
        "new",
        "noexcept",
        "operator",
        "private",
        "protected",
        "public",
        "requires",
        "return",
        "sizeof",
        "static",
        "struct",
        "switch",
        "template",
        "this",
        "throw",
        "true",
        "try",
        "typedef",
        "typename",
        "union",
        "using",
        "virtual",
        "volatile",
        "while",
    ],
    types: &[
        "bool",
        "char",
        "double",
        "float",
        "int",
        "long",
        "short",
        "signed",
        "unsigned",
        "void",
        "size_t",
        "string",
        "vector",
        "map",
        "set",
        "optional",
        "unique_ptr",
        "shared_ptr",
    ],
    attribute_sigils: &['#'],
};

/// JavaScript, TypeScript, and their JSX/TSX dialects.
pub const JAVASCRIPT: Syntax = Syntax {
    names: &[
        "javascript",
        "js",
        "typescript",
        "ts",
        "jsx",
        "tsx",
        "mjs",
        "cjs",
    ],
    line_comments: &["//"],
    block_comment: C_LIKE_BLOCK,
    nested_block_comments: false,
    strings: &['"', '\''],
    multiline_strings: &["`"],
    escape: Some('\\'),
    keywords: &[
        "as",
        "async",
        "await",
        "break",
        "case",
        "catch",
        "class",
        "const",
        "continue",
        "debugger",
        "default",
        "delete",
        "do",
        "else",
        "export",
        "extends",
        "false",
        "finally",
        "for",
        "function",
        "from",
        "if",
        "import",
        "in",
        "instanceof",
        "interface",
        "let",
        "new",
        "null",
        "of",
        "return",
        "static",
        "super",
        "switch",
        "this",
        "throw",
        "true",
        "try",
        "type",
        "typeof",
        "undefined",
        "var",
        "void",
        "while",
        "yield",
    ],
    types: &[
        "Array", "Boolean", "Date", "Error", "Map", "Number", "Object", "Promise", "RegExp", "Set",
        "String", "Symbol", "any", "boolean", "never", "number", "string", "unknown",
    ],
    attribute_sigils: &['@'],
};

/// Python.
pub const PYTHON: Syntax = Syntax {
    names: &["python", "py", "python3"],
    line_comments: &["#"],
    block_comment: None,
    nested_block_comments: false,
    strings: &['"', '\''],
    multiline_strings: &["\"\"\"", "'''"],
    escape: Some('\\'),
    keywords: &[
        "and", "as", "assert", "async", "await", "break", "class", "continue", "def", "del",
        "elif", "else", "except", "False", "finally", "for", "from", "global", "if", "import",
        "in", "is", "lambda", "None", "nonlocal", "not", "or", "pass", "raise", "return", "True",
        "try", "while", "with", "yield",
    ],
    types: &[
        "bool",
        "bytes",
        "dict",
        "float",
        "frozenset",
        "int",
        "list",
        "object",
        "set",
        "str",
        "tuple",
        "type",
    ],
    attribute_sigils: &['@'],
};

/// Go.
pub const GO: Syntax = Syntax {
    names: &["go", "golang"],
    line_comments: &["//"],
    block_comment: C_LIKE_BLOCK,
    nested_block_comments: false,
    strings: &['"', '\''],
    multiline_strings: &["`"],
    escape: Some('\\'),
    keywords: &[
        "break",
        "case",
        "chan",
        "const",
        "continue",
        "default",
        "defer",
        "else",
        "fallthrough",
        "for",
        "func",
        "go",
        "goto",
        "if",
        "import",
        "interface",
        "map",
        "package",
        "range",
        "return",
        "select",
        "struct",
        "switch",
        "type",
        "var",
        "nil",
        "true",
        "false",
    ],
    types: &[
        "bool",
        "byte",
        "complex64",
        "complex128",
        "error",
        "float32",
        "float64",
        "int",
        "int8",
        "int16",
        "int32",
        "int64",
        "rune",
        "string",
        "uint",
        "uint8",
        "uint16",
        "uint32",
        "uint64",
        "uintptr",
    ],
    attribute_sigils: &[],
};

/// Java and Kotlin.
pub const JAVA: Syntax = Syntax {
    names: &["java", "kotlin", "kt"],
    line_comments: &["//"],
    block_comment: C_LIKE_BLOCK,
    nested_block_comments: false,
    strings: &['"', '\''],
    multiline_strings: &["\"\"\""],
    escape: Some('\\'),
    keywords: &[
        "abstract",
        "assert",
        "break",
        "case",
        "catch",
        "class",
        "companion",
        "const",
        "continue",
        "data",
        "default",
        "do",
        "else",
        "enum",
        "extends",
        "final",
        "finally",
        "for",
        "fun",
        "if",
        "implements",
        "import",
        "instanceof",
        "interface",
        "internal",
        "is",
        "native",
        "new",
        "object",
        "open",
        "override",
        "package",
        "private",
        "protected",
        "public",
        "return",
        "sealed",
        "static",
        "super",
        "switch",
        "synchronized",
        "this",
        "throw",
        "throws",
        "transient",
        "try",
        "val",
        "var",
        "volatile",
        "when",
        "while",
    ],
    types: &[
        "boolean", "byte", "char", "double", "float", "int", "long", "short", "void", "Boolean",
        "Double", "Float", "Int", "Integer", "List", "Long", "Map", "Set", "String", "Unit",
    ],
    attribute_sigils: &['@'],
};

/// JSON.
pub const JSON: Syntax = Syntax {
    names: &["json", "jsonc"],
    line_comments: &["//"],
    block_comment: C_LIKE_BLOCK,
    nested_block_comments: false,
    strings: &['"'],
    multiline_strings: &[],
    escape: Some('\\'),
    keywords: &["true", "false", "null"],
    types: &[],
    attribute_sigils: &[],
};

/// TOML and INI.
pub const TOML: Syntax = Syntax {
    names: &["toml", "ini", "cfg"],
    line_comments: &["#", ";"],
    block_comment: None,
    nested_block_comments: false,
    strings: &['"', '\''],
    multiline_strings: &["\"\"\"", "'''"],
    escape: Some('\\'),
    keywords: &["true", "false"],
    types: &[],
    attribute_sigils: &[],
};

/// YAML.
pub const YAML: Syntax = Syntax {
    names: &["yaml", "yml"],
    line_comments: &["#"],
    block_comment: None,
    nested_block_comments: false,
    strings: &['"', '\''],
    multiline_strings: &[],
    escape: Some('\\'),
    keywords: &["true", "false", "null", "yes", "no", "on", "off"],
    types: &[],
    attribute_sigils: &[],
};

/// HTML, XML, and SVG.
pub const MARKUP: Syntax = Syntax {
    names: &["html", "xml", "svg", "xhtml"],
    line_comments: &[],
    block_comment: Some(("<!--", "-->")),
    nested_block_comments: false,
    strings: &['"', '\''],
    multiline_strings: &[],
    escape: None,
    keywords: &[],
    types: &[],
    attribute_sigils: &[],
};

/// CSS and its preprocessor dialects.
pub const CSS: Syntax = Syntax {
    names: &["css", "scss", "sass", "less"],
    line_comments: &["//"],
    block_comment: C_LIKE_BLOCK,
    nested_block_comments: false,
    strings: &['"', '\''],
    multiline_strings: &[],
    escape: Some('\\'),
    keywords: &[
        "and",
        "from",
        "important",
        "not",
        "only",
        "to",
        "inherit",
        "initial",
        "unset",
        "auto",
        "none",
    ],
    types: &[],
    attribute_sigils: &['@'],
};

/// POSIX shell dialects.
pub const SHELL: Syntax = Syntax {
    names: &["shell", "sh", "bash", "zsh", "console", "shell-session"],
    line_comments: &["#"],
    block_comment: None,
    nested_block_comments: false,
    strings: &['"', '\''],
    multiline_strings: &[],
    escape: Some('\\'),
    keywords: &[
        "case", "do", "done", "elif", "else", "esac", "export", "fi", "for", "function", "if",
        "in", "local", "read", "return", "select", "set", "shift", "then", "unset", "until",
        "while",
    ],
    types: &[],
    attribute_sigils: &['$'],
};

/// SQL.
pub const SQL: Syntax = Syntax {
    names: &["sql", "postgres", "postgresql", "mysql", "sqlite"],
    line_comments: &["--"],
    block_comment: C_LIKE_BLOCK,
    nested_block_comments: false,
    strings: &['\'', '"'],
    multiline_strings: &[],
    escape: Some('\\'),
    keywords: &[
        "ALTER",
        "AND",
        "AS",
        "ASC",
        "BY",
        "CREATE",
        "DELETE",
        "DESC",
        "DISTINCT",
        "DROP",
        "EXISTS",
        "FROM",
        "GROUP",
        "HAVING",
        "INDEX",
        "INNER",
        "INSERT",
        "INTO",
        "JOIN",
        "LEFT",
        "LIMIT",
        "NOT",
        "NULL",
        "OFFSET",
        "ON",
        "OR",
        "ORDER",
        "OUTER",
        "PRIMARY",
        "REFERENCES",
        "RIGHT",
        "SELECT",
        "SET",
        "TABLE",
        "UNION",
        "UPDATE",
        "VALUES",
        "WHERE",
        "WITH",
    ],
    types: &[
        "BIGINT",
        "BOOLEAN",
        "BYTEA",
        "DATE",
        "DECIMAL",
        "DOUBLE",
        "FLOAT",
        "INT",
        "INTEGER",
        "JSON",
        "JSONB",
        "NUMERIC",
        "REAL",
        "SERIAL",
        "SMALLINT",
        "TEXT",
        "TIMESTAMP",
        "UUID",
        "VARCHAR",
    ],
    attribute_sigils: &[],
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn find_is_case_and_extension_insensitive() {
        assert_eq!(Syntax::find("Rust").names[0], "rust");
        assert_eq!(Syntax::find(".rs").names[0], "rust");
        assert_eq!(Syntax::find("  ts  ").names[0], "javascript");
    }

    #[test]
    fn unknown_language_falls_back_to_plain() {
        assert_eq!(Syntax::find("brainfuck").names[0], "plain");
        assert_eq!(Syntax::find("").names[0], "plain");
    }

    #[test]
    fn every_syntax_is_reachable_by_its_canonical_name() {
        for syntax in ALL {
            let canonical = syntax.names[0];

            assert_eq!(
                Syntax::find(canonical).names[0],
                canonical,
                "`{canonical}` is shadowed by an earlier syntax in ALL"
            );
        }
    }

    #[test]
    fn no_token_is_claimed_by_two_syntaxes() {
        for (index, syntax) in ALL.iter().enumerate() {
            for name in syntax.names {
                for other in &ALL[index + 1..] {
                    assert!(
                        !other.names.contains(name),
                        "`{name}` is claimed by both `{}` and `{}`",
                        syntax.names[0],
                        other.names[0]
                    );
                }
            }
        }
    }
}
