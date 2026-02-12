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
