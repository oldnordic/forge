use std::path::{Path, PathBuf};
use std::sync::Arc;

use sqlitegraph::backend::NodeSpec;
use sqlitegraph::config::{open_graph, GraphConfig};

use crate::error::{ForgeError, Result};
use crate::types::{Language, Location, Reference, ReferenceKind, Symbol, SymbolId, SymbolKind};

use super::store::{StoredReference, UnifiedGraphStore};
use super::BackendKind;

impl UnifiedGraphStore {
    pub async fn insert_symbol(&self, symbol: &Symbol) -> Result<SymbolId> {
        let config = match self.backend_kind {
            BackendKind::SQLite => GraphConfig::sqlite(),
            BackendKind::NativeV3 => GraphConfig::native(),
        };
        let backend = open_graph(&self.db_path, &config)
            .map_err(|e| ForgeError::DatabaseError(format!("Failed to open graph: {}", e)))?;

        let kind = match symbol.kind {
            SymbolKind::Function | SymbolKind::Method => "fn",
            SymbolKind::Struct => "struct",
            SymbolKind::Enum => "enum",
            SymbolKind::Trait => "trait",
            SymbolKind::Impl => "impl",
            SymbolKind::Module => "module",
            SymbolKind::TypeAlias => "type",
            SymbolKind::Constant | SymbolKind::Static => "const",
            SymbolKind::Parameter | SymbolKind::LocalVariable | SymbolKind::Field => "variable",
            SymbolKind::Macro => "macro",
            SymbolKind::Use => "use",
        };

        let node = NodeSpec {
            kind: kind.to_string(),
            name: symbol.name.to_string(),
            file_path: Some(symbol.location.file_path.to_string_lossy().into_owned()),
            data: symbol.metadata.clone(),
        };

        let id = backend
            .insert_node(node)
            .map_err(|e| ForgeError::DatabaseError(format!("Insert node failed: {}", e)))?;

        Ok(SymbolId(id))
    }

    pub async fn insert_reference(&self, reference: &Reference) -> Result<()> {
        if self.backend_kind == BackendKind::NativeV3 {
            let mut refs = self
                .references
                .lock()
                .expect("invariant: references mutex not poisoned");

            let to_symbol = format!("sym_{}", reference.to.0);

            refs.push(StoredReference {
                to_symbol,
                kind: reference.kind,
                file_path: reference.location.file_path.clone(),
                line_number: reference.location.line_number,
            });
        }
        Ok(())
    }

    pub async fn query_symbols(&self, name: &str) -> Result<Vec<Symbol>> {
        let conn = rusqlite::Connection::open(&self.db_path)
            .map_err(|e| ForgeError::DatabaseError(format!("Open db failed: {}", e)))?;

        let pattern = format!("%{}%", name);
        let mut stmt = conn
            .prepare(
                "SELECT id, kind, name, file_path FROM graph_entities WHERE name LIKE ?1 LIMIT 50",
            )
            .map_err(|e| ForgeError::DatabaseError(format!("Prepare failed: {}", e)))?;

        let symbols = stmt
            .query_map(rusqlite::params![pattern], |row| {
                let id: i64 = row.get(0)?;
                let sym_name: String = row.get(2)?;
                let file_path: Option<String> = row.get(3)?;
                Ok((id, sym_name, file_path))
            })
            .map_err(|e| ForgeError::DatabaseError(format!("Query failed: {}", e)))?
            .flatten()
            .map(|(id, sym_name, file_path)| Symbol {
                id: SymbolId(id),
                name: Arc::from(sym_name.as_str()),
                fully_qualified_name: Arc::from(sym_name.as_str()),
                kind: SymbolKind::Function,
                language: Language::Rust,
                location: Location {
                    file_path: file_path
                        .map(PathBuf::from)
                        .unwrap_or_else(|| PathBuf::from("")),
                    byte_start: 0,
                    byte_end: 0,
                    line_number: 0,
                },
                parent_id: None,
                metadata: serde_json::Value::Null,
            })
            .collect();

        Ok(symbols)
    }

    pub async fn get_symbol(&self, _id: SymbolId) -> Result<Symbol> {
        Err(ForgeError::SymbolNotFound("Not implemented".to_string()))
    }

    pub async fn symbol_exists(&self, id: SymbolId) -> Result<bool> {
        let conn = rusqlite::Connection::open(&self.db_path)
            .map_err(|e| ForgeError::DatabaseError(format!("Open db failed: {}", e)))?;
        let exists: i64 = conn
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM graph_entities WHERE id = ?1)",
                rusqlite::params![id.0],
                |row| row.get(0),
            )
            .map_err(|e| ForgeError::DatabaseError(format!("Query failed: {}", e)))?;
        Ok(exists > 0)
    }

    pub async fn query_references(&self, symbol_id: SymbolId) -> Result<Vec<Reference>> {
        if self.backend_kind == BackendKind::NativeV3 {
            let refs = self
                .references
                .lock()
                .expect("invariant: references mutex not poisoned");
            let target_symbol = format!("sym_{}", symbol_id.0);

            let mut result = Vec::new();
            for stored in refs.iter() {
                if stored.to_symbol == target_symbol {
                    result.push(Reference {
                        from: SymbolId(0),
                        to: symbol_id,
                        from_name: None,
                        to_name: None,
                        kind: stored.kind,
                        location: Location {
                            file_path: stored.file_path.clone(),
                            byte_start: 0,
                            byte_end: 0,
                            line_number: stored.line_number,
                        },
                    });
                }
            }
            return Ok(result);
        }

        Ok(Vec::new())
    }

    pub async fn get_all_symbols(&self) -> Result<Vec<Symbol>> {
        let conn = rusqlite::Connection::open(&self.db_path)
            .map_err(|e| ForgeError::DatabaseError(format!("Open db failed: {}", e)))?;
        let mut stmt = conn
            .prepare("SELECT id, kind, name, file_path FROM graph_entities LIMIT 1000")
            .map_err(|e| ForgeError::DatabaseError(format!("Prepare failed: {}", e)))?;
        let symbols = stmt
            .query_map([], |row| {
                let id: i64 = row.get(0)?;
                let sym_name: String = row.get(2)?;
                let file_path: Option<String> = row.get(3)?;
                Ok((id, sym_name, file_path))
            })
            .map_err(|e| ForgeError::DatabaseError(format!("Query failed: {}", e)))?
            .flatten()
            .map(|(id, sym_name, file_path)| Symbol {
                id: SymbolId(id),
                name: Arc::from(sym_name.as_str()),
                fully_qualified_name: Arc::from(sym_name.as_str()),
                kind: SymbolKind::Function,
                language: Language::Rust,
                location: Location {
                    file_path: file_path
                        .map(PathBuf::from)
                        .unwrap_or_else(|| PathBuf::from("")),
                    byte_start: 0,
                    byte_end: 0,
                    line_number: 0,
                },
                parent_id: None,
                metadata: serde_json::Value::Null,
            })
            .collect();
        Ok(symbols)
    }

    pub async fn symbol_count(&self) -> Result<usize> {
        let conn = rusqlite::Connection::open(&self.db_path)
            .map_err(|e| ForgeError::DatabaseError(format!("Open db failed: {}", e)))?;
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM graph_entities", [], |row| row.get(0))
            .map_err(|e| ForgeError::DatabaseError(format!("Query failed: {}", e)))?;
        Ok(count as usize)
    }

    pub async fn index_cross_file_references(&self) -> Result<usize> {
        if self.backend_kind != BackendKind::NativeV3 {
            return Ok(0);
        }

        self.legacy_index_cross_file_references().await
    }

    async fn legacy_index_cross_file_references(&self) -> Result<usize> {
        use regex::Regex;
        use tokio::fs;

        let mut symbols: std::collections::HashMap<String, (PathBuf, usize)> =
            std::collections::HashMap::new();
        self.collect_symbols_recursive(&self.codebase_path, &mut symbols)
            .await?;

        let reference_pattern = Regex::new(r"\b([a-zA-Z_][a-zA-Z0-9_]*)\s*\(")
            .expect("invariant: static regex pattern is valid");

        {
            let mut refs = self
                .references
                .lock()
                .expect("invariant: references mutex not poisoned");
            refs.clear();
        }

        let mut found_refs: Vec<StoredReference> = Vec::new();

        for (symbol_name, (_file_path, _)) in &symbols {
            for (target_file, _) in symbols.values() {
                if let Ok(content) = fs::read_to_string(target_file).await {
                    for (line_num, line) in content.lines().enumerate() {
                        if line.contains("fn ") || line.contains("struct ") {
                            continue;
                        }

                        for cap in reference_pattern.captures_iter(line) {
                            if let Some(matched) = cap.get(1) {
                                if matched.as_str() == symbol_name {
                                    found_refs.push(StoredReference {
                                        to_symbol: format!("sym_{}", symbol_name),
                                        kind: ReferenceKind::Call,
                                        file_path: target_file.clone(),
                                        line_number: line_num + 1,
                                    });
                                }
                            }
                        }
                    }
                }
            }
        }

        let ref_count = found_refs.len();
        self.references
            .lock()
            .expect("invariant: references mutex not poisoned")
            .extend(found_refs);

        Ok(ref_count)
    }

    async fn collect_symbols_recursive(
        &self,
        dir: &Path,
        symbols: &mut std::collections::HashMap<String, (PathBuf, usize)>,
    ) -> Result<()> {
        use tokio::fs;

        let mut entries = fs::read_dir(dir)
            .await
            .map_err(|e| ForgeError::DatabaseError(format!("Failed to read dir: {}", e)))?;

        while let Some(entry) = entries
            .next_entry()
            .await
            .map_err(|e| ForgeError::DatabaseError(format!("Failed to read entry: {}", e)))?
        {
            let path = entry.path();
            if path.is_dir() {
                Box::pin(self.collect_symbols_recursive(&path, symbols)).await?;
            } else if path.extension().map(|e| e == "rs").unwrap_or(false) {
                if let Ok(content) = fs::read_to_string(&path).await {
                    for (line_num, line) in content.lines().enumerate() {
                        if let Some(fn_pos) = line.find("fn ") {
                            let after_fn = &line[fn_pos + 3..];
                            if let Some(end_pos) =
                                after_fn.find(|c: char| c.is_whitespace() || c == '(')
                            {
                                let name = after_fn[..end_pos].trim().to_string();
                                if !name.is_empty() {
                                    symbols.insert(name, (path.clone(), line_num + 1));
                                }
                            }
                        }
                        if let Some(struct_pos) = line.find("struct ") {
                            let after_struct = &line[struct_pos + 7..];
                            if let Some(end_pos) = after_struct
                                .find(|c: char| c.is_whitespace() || c == '{' || c == ';')
                            {
                                let name = after_struct[..end_pos].trim().to_string();
                                if !name.is_empty() {
                                    symbols.insert(name, (path.clone(), line_num + 1));
                                }
                            }
                        }
                    }
                }
            }
        }

        Ok(())
    }

    pub async fn query_references_for_symbol(&self, symbol_name: &str) -> Result<Vec<Reference>> {
        if self.backend_kind != BackendKind::NativeV3 {
            return Ok(Vec::new());
        }

        let refs = self
            .references
            .lock()
            .expect("invariant: references mutex not poisoned");
        let mut result = Vec::new();

        for stored in refs.iter() {
            if stored.to_symbol == format!("sym_{}", symbol_name)
                || stored.to_symbol.contains(symbol_name)
            {
                result.push(Reference {
                    from: SymbolId(0),
                    to: SymbolId(0),
                    from_name: None,
                    to_name: None,
                    kind: stored.kind,
                    location: Location {
                        file_path: stored.file_path.clone(),
                        byte_start: 0,
                        byte_end: 0,
                        line_number: stored.line_number,
                    },
                });
            }
        }

        Ok(result)
    }
}
