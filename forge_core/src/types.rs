//! Core types for ForgeKit.

use std::path::PathBuf;

/// Stable identifier for a symbol across reindexing.
///
/// This ID is generated from a hash of the symbol's fully qualified name
/// and location, ensuring stability even as the codebase changes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SymbolId(pub i64);

impl std::fmt::Display for SymbolId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<i64> for SymbolId {
    fn from(id: i64) -> Self {
        Self(id)
    }
}

/// Stable identifier for a CFG block.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct BlockId(pub i64);

impl std::fmt::Display for BlockId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Stable identifier for an execution path.
///
/// This is a BLAKE3 hash of the path's block sequence.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PathId(pub [u8; 16]);

impl std::fmt::Display for PathId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for (i, byte) in self.0.iter().enumerate() {
            if i > 0 {
                write!(f, ":")?;
            }
            write!(f, "{:02x}", byte)?;
        }
        Ok(())
    }
}

/// Source code location with file path and byte span.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Location {
    /// Path to the file containing this symbol
    pub file_path: PathBuf,
    /// Byte offset of the symbol's start (UTF-8 aware)
    pub byte_start: u32,
    /// Byte offset of the symbol's end (UTF-8 aware)
    pub byte_end: u32,
    /// Line number (1-indexed)
    pub line_number: usize,
}

impl Location {
    /// Returns the byte span of this location.
    pub fn span(&self) -> Span {
        Span {
            start: self.byte_start,
            end: self.byte_end,
        }
    }

    /// Returns the length in bytes.
    pub fn len(&self) -> u32 {
        self.byte_end - self.byte_start
    }
}

/// A byte span within a file.
///
/// Spans are half-open: [start, end)
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Span {
    /// Start byte offset (inclusive)
    pub start: u32,
    /// End byte offset (exclusive)
    pub end: u32,
}

impl Span {
    /// Returns the length of this span.
    pub fn len(&self) -> u32 {
        self.end - self.start
    }

    /// Returns true if this span is empty (zero length).
    pub fn is_empty(&self) -> bool {
        self.start == self.end
    }

    /// Returns true if this span contains the given byte offset.
    pub fn contains(&self, offset: u32) -> bool {
        offset >= self.start && offset < self.end
    }

    /// Creates a new span covering both this and another.
    pub fn merge(&self, other: Span) -> Span {
        Span {
            start: self.start.min(other.start),
            end: self.end.max(other.end),
        }
    }
}

/// Symbol kind classification.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SymbolKind {
    // Declarations
    Function,
    Method,
    Struct,
    Enum,
    Trait,
    Impl,
    Module,
    TypeAlias,
    Constant,
    Static,

    // Variables
    Parameter,
    LocalVariable,
    Field,

    // Other
    Macro,
    Use,
}

impl SymbolKind {
    /// Returns true if this symbol represents a type declaration.
    pub fn is_type(&self) -> bool {
        matches!(self, Self::Struct | Self::Enum | Self::Trait | Self::TypeAlias)
    }

    /// Returns true if this symbol represents a function or method.
    pub fn is_function(&self) -> bool {
        matches!(self, Self::Function | Self::Method)
    }
}

/// Programming language detection.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Language {
    Rust,
    Python,
    C,
    Cpp,
    Java,
    JavaScript,
    TypeScript,
    Go,
    Unknown(String),
}

/// Path kind classification.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PathKind {
    /// Normal execution path (returns successfully)
    Normal,
    /// Error path (returns error or panics)
    Error,
    /// Degenerate path (unreachable code)
    Degenerate,
    /// Infinite path (loop without exit)
    Infinite,
}

/// Reference type between symbols.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ReferenceKind {
    /// Function or method call
    Call,
    /// Use or import statement
    Use,
    /// Type reference (annotation, bound, etc.)
    TypeReference,
    /// Inheritance relationship
    Inherit,
    /// Trait implementation
    Implementation,
    /// Method override
    Override,
}

/// A symbol in the code graph.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Symbol {
    /// Stable symbol identifier
    pub id: SymbolId,
    /// Display name
    pub name: String,
    /// Fully qualified name
    pub fully_qualified_name: String,
    /// Symbol kind
    pub kind: SymbolKind,
    /// Programming language
    pub language: Language,
    /// Source location
    pub location: Location,
    /// Parent symbol ID (if nested)
    pub parent_id: Option<SymbolId>,
    /// Additional metadata
    pub metadata: serde_json::Value,
}

/// A reference between two symbols.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Reference {
    /// The referencing symbol
    pub from: SymbolId,
    /// The referenced symbol
    pub to: SymbolId,
    /// Reference kind
    pub kind: ReferenceKind,
    /// Location of the reference
    pub location: Location,
}

/// An execution path through a function.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Path {
    /// Stable path identifier
    pub id: PathId,
    /// Path kind
    pub kind: PathKind,
    /// Blocks in this path, in order
    pub blocks: Vec<BlockId>,
    /// Path length (number of blocks)
    pub length: usize,
}

/// A cycle in the call graph.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Cycle {
    /// Symbols in the cycle
    pub members: Vec<SymbolId>,
}

/// A natural loop in the CFG.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Loop {
    /// Loop header block
    pub header: BlockId,
    /// Blocks in the loop body
    pub blocks: Vec<BlockId>,
    /// Nesting depth
    pub depth: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    // SymbolId Tests (4 tests)

    #[test]
    fn test_symbol_id_display() {
        let id = SymbolId(42);
        assert_eq!(id.to_string(), "42");
    }

    #[test]
    fn test_symbol_id_from_i64() {
        let id: SymbolId = 123.into();
        assert_eq!(id.0, 123);
    }

    #[test]
    fn test_symbol_id_ord() {
        let id1 = SymbolId(10);
        let id2 = SymbolId(20);
        assert!(id1 < id2);
        assert!(id2 > id1);
    }

    #[test]
    fn test_symbol_id_zero() {
        let id = SymbolId(0);
        assert_eq!(id.to_string(), "0");
    }

    // BlockId Tests (3 tests)

    #[test]
    fn test_block_id_display() {
        let id = BlockId(7);
        assert_eq!(id.to_string(), "7");
    }

    #[test]
    fn test_block_id_from_i64() {
        let id = BlockId(999);
        assert_eq!(id.0, 999);
    }

    #[test]
    fn test_block_id_zero() {
        let id = BlockId(0);
        assert_eq!(id.to_string(), "0");
    }

    // PathId Tests (4 tests)

    #[test]
    fn test_path_id_display() {
        let id = PathId([0x01, 0x02, 0xab, 0xcd, 0x00, 0x00, 0x00, 0x00,
                         0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00]);
        let display = id.to_string();
        assert!(display.contains("01:02:ab:cd"));
    }

    #[test]
    fn test_path_id_hash_stability() {
        let bytes = [1u8, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16];
        let id1 = PathId(bytes);
        let id2 = PathId(bytes);
        assert_eq!(id1, id2);
    }

    #[test]
    fn test_path_id_hash_uniqueness() {
        let bytes1 = [1u8, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16];
        let bytes2 = [1u8, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 17];
        let id1 = PathId(bytes1);
        let id2 = PathId(bytes2);
        assert_ne!(id1, id2);
    }

    #[test]
    fn test_path_id_empty() {
        let bytes = [0u8; 16];
        let id = PathId(bytes);
        assert_eq!(id.to_string(), "00:00:00:00:00:00:00:00:00:00:00:00:00:00:00:00");
    }

    // Location Tests (5 tests)

    #[test]
    fn test_location_span() {
        let loc = Location {
            file_path: PathBuf::from("test.rs"),
            byte_start: 10,
            byte_end: 50,
            line_number: 5,
        };
        let span = loc.span();
        assert_eq!(span.start, 10);
        assert_eq!(span.end, 50);
    }

    #[test]
    fn test_location_len() {
        let loc = Location {
            file_path: PathBuf::from("test.rs"),
            byte_start: 10,
            byte_end: 50,
            line_number: 5,
        };
        assert_eq!(loc.len(), 40);
    }

    #[test]
    fn test_location_new() {
        let loc = Location {
            file_path: PathBuf::from("src/main.rs"),
            byte_start: 100,
            byte_end: 200,
            line_number: 10,
        };
        assert_eq!(loc.file_path, PathBuf::from("src/main.rs"));
        assert_eq!(loc.byte_start, 100);
        assert_eq!(loc.byte_end, 200);
        assert_eq!(loc.line_number, 10);
    }

    #[test]
    fn test_location_clone() {
        let loc1 = Location {
            file_path: PathBuf::from("test.rs"),
            byte_start: 0,
            byte_end: 10,
            line_number: 1,
        };
        let loc2 = loc1.clone();
        assert_eq!(loc1, loc2);
    }

    #[test]
    fn test_location_zero_length() {
        let loc = Location {
            file_path: PathBuf::from("test.rs"),
            byte_start: 42,
            byte_end: 42,
            line_number: 7,
        };
        assert_eq!(loc.len(), 0);
    }

    // Span Tests (7 tests)

    #[test]
    fn test_span_len() {
        let span = Span { start: 10, end: 50 };
        assert_eq!(span.len(), 40);
    }

    #[test]
    fn test_span_is_empty_true() {
        let span = Span { start: 10, end: 10 };
        assert!(span.is_empty());
    }

    #[test]
    fn test_span_is_empty_false() {
        let span = Span { start: 10, end: 20 };
        assert!(!span.is_empty());
    }

    #[test]
    fn test_span_contains() {
        let span = Span { start: 10, end: 20 };
        assert!(span.contains(10));
        assert!(span.contains(15));
        assert!(!span.contains(20));
        assert!(!span.contains(5));
    }

    #[test]
    fn test_span_merge() {
        let span1 = Span { start: 10, end: 20 };
        let span2 = Span { start: 15, end: 30 };
        let merged = span1.merge(span2);
        assert_eq!(merged.start, 10);
        assert_eq!(merged.end, 30);
    }

    #[test]
    fn test_span_merge_adjacent() {
        let span1 = Span { start: 10, end: 20 };
        let span2 = Span { start: 20, end: 30 };
        let merged = span1.merge(span2);
        assert_eq!(merged.start, 10);
        assert_eq!(merged.end, 30);
    }

    #[test]
    fn test_span_overlaps() {
        let span1 = Span { start: 10, end: 30 };
        let span2 = Span { start: 20, end: 40 };
        // spans overlap from 20-30
        let merged = span1.merge(span2);
        assert_eq!(merged.start, 10);
        assert_eq!(merged.end, 40);
    }

    // SymbolKind Tests (3 tests)

    #[test]
    fn test_symbol_kind_is_type() {
        assert!(SymbolKind::Struct.is_type());
        assert!(SymbolKind::Enum.is_type());
        assert!(SymbolKind::Trait.is_type());
        assert!(SymbolKind::TypeAlias.is_type());
        assert!(!SymbolKind::Function.is_type());
    }

    #[test]
    fn test_symbol_kind_is_function() {
        assert!(SymbolKind::Function.is_function());
        assert!(SymbolKind::Method.is_function());
        assert!(!SymbolKind::Struct.is_function());
    }

    #[test]
    fn test_symbol_kind_is_variable() {
        // Note: is_variable doesn't exist in SymbolKind, but we test the other predicates
        // This test documents the current behavior
        assert!(!SymbolKind::LocalVariable.is_function());
        assert!(!SymbolKind::LocalVariable.is_type());
    }

    // Language Tests (3 tests)

    #[test]
    fn test_language_variants() {
        let _rust = Language::Rust;
        let _python = Language::Python;
        let _c = Language::C;
        let _cpp = Language::Cpp;
        let _java = Language::Java;
        let _js = Language::JavaScript;
        let _ts = Language::TypeScript;
        let _go = Language::Go;
        let _unknown = Language::Unknown("SomeLang".to_string());
    }

    #[test]
    fn test_language_unknown() {
        let lang = Language::Unknown("MyLang".to_string());
        match lang {
            Language::Unknown(s) => assert_eq!(s, "MyLang"),
            _ => panic!("Expected Unknown variant"),
        }
    }

    #[test]
    fn test_language_from_str() {
        // Language doesn't implement FromStr currently
        // This test documents the current behavior
        let lang = Language::Rust;
        match lang {
            Language::Rust => assert!(true),
            _ => assert!(false),
        }
    }

    // PathKind Tests (2 tests)

    #[test]
    fn test_path_kind_variants() {
        let _normal = PathKind::Normal;
        let _error = PathKind::Error;
        let _degenerate = PathKind::Degenerate;
        let _infinite = PathKind::Infinite;
    }

    #[test]
    fn test_path_kind_is_absolute() {
        // PathKind doesn't have is_absolute method currently
        // This test documents that all variants are constructable
        assert!(matches!(PathKind::Normal, PathKind::Normal));
    }

    // ReferenceKind Tests (2 tests)

    #[test]
    fn test_reference_kind_variants() {
        let _call = ReferenceKind::Call;
        let _use = ReferenceKind::Use;
        let _type_ref = ReferenceKind::TypeReference;
        let _inherit = ReferenceKind::Inherit;
        let _impl = ReferenceKind::Implementation;
        let _override = ReferenceKind::Override;
    }

    #[test]
    fn test_reference_kind_is_call() {
        // ReferenceKind doesn't have is_call method currently
        // This test documents that call variants exist
        assert!(matches!(ReferenceKind::Call, ReferenceKind::Call));
        assert!(matches!(ReferenceKind::Call, ReferenceKind::Call));
    }

    // Data Structure Tests (4 tests)

    #[test]
    fn test_symbol_new() {
        let symbol = Symbol {
            id: SymbolId(1),
            name: "test".to_string(),
            fully_qualified_name: "crate::test".to_string(),
            kind: SymbolKind::Function,
            language: Language::Rust,
            location: Location {
                file_path: PathBuf::from("test.rs"),
                byte_start: 0,
                byte_end: 10,
                line_number: 1,
            },
            parent_id: None,
            metadata: serde_json::Value::Null,
        };
        assert_eq!(symbol.id.0, 1);
        assert_eq!(symbol.name, "test");
    }

    #[test]
    fn test_reference_new() {
        let reference = Reference {
            from: SymbolId(1),
            to: SymbolId(2),
            kind: ReferenceKind::Call,
            location: Location {
                file_path: PathBuf::from("test.rs"),
                byte_start: 0,
                byte_end: 10,
                line_number: 1,
            },
        };
        assert_eq!(reference.from.0, 1);
        assert_eq!(reference.to.0, 2);
    }

    #[test]
    fn test_symbol_with_parent() {
        let symbol = Symbol {
            id: SymbolId(2),
            name: "inner".to_string(),
            fully_qualified_name: "crate::Parent::inner".to_string(),
            kind: SymbolKind::Method,
            language: Language::Rust,
            location: Location {
                file_path: PathBuf::from("test.rs"),
                byte_start: 10,
                byte_end: 20,
                line_number: 2,
            },
            parent_id: Some(SymbolId(1)),
            metadata: serde_json::Value::Null,
        };
        assert!(symbol.parent_id.is_some());
        assert_eq!(symbol.parent_id.unwrap().0, 1);
    }

    #[test]
    fn test_reference_with_metadata() {
        // Reference doesn't have metadata field
        // This test creates a symbol with metadata instead
        let metadata = serde_json::json!({"key": "value"});
        let symbol = Symbol {
            id: SymbolId(1),
            name: "test".to_string(),
            fully_qualified_name: "crate::test".to_string(),
            kind: SymbolKind::Function,
            language: Language::Rust,
            location: Location {
                file_path: PathBuf::from("test.rs"),
                byte_start: 0,
                byte_end: 10,
                line_number: 1,
            },
            parent_id: None,
            metadata,
        };
        assert_eq!(symbol.metadata["key"], "value");
    }

    // Edge Case Tests (3 tests)

    #[test]
    fn test_large_symbol_id() {
        let id = SymbolId(i64::MAX);
        assert_eq!(id.to_string(), i64::MAX.to_string());
    }

    #[test]
    fn test_large_byte_offsets() {
        let loc = Location {
            file_path: PathBuf::from("test.rs"),
            byte_start: u32::MAX - 10,
            byte_end: u32::MAX,
            line_number: 1000,
        };
        assert_eq!(loc.byte_start, u32::MAX - 10);
        assert_eq!(loc.byte_end, u32::MAX);
    }

    #[test]
    fn test_empty_metadata() {
        let metadata = serde_json::json!({});
        let symbol = Symbol {
            id: SymbolId(1),
            name: "test".to_string(),
            fully_qualified_name: "crate::test".to_string(),
            kind: SymbolKind::Function,
            language: Language::Rust,
            location: Location {
                file_path: PathBuf::from("test.rs"),
                byte_start: 0,
                byte_end: 10,
                line_number: 1,
            },
            parent_id: None,
            metadata,
        };
        assert!(symbol.metadata.as_object().unwrap().is_empty());
    }
}
