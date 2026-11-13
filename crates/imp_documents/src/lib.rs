//! Multi-document session store for Imp Lab hosts.
//!
//! Tracks open documents (tab order, active selection, path identity, dirty/path
//! lifecycle). Payload type `P` is host-defined so product data (Aura notation
//! embeddings, Imp graphs, JSON tips, …) stays outside this crate.

use std::path::{Path, PathBuf};

/// Stable identity for an open document within a session.
///
/// Session-local only — do not persist as a durable document id.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DocumentId(u64);

impl DocumentId {
    /// Creates an id from a raw value (tests, diagnostics, UI keys).
    pub fn from_raw(id: u64) -> Self {
        Self(id)
    }

    /// Numeric value for tests, diagnostics, and host UI keys.
    pub fn get(self) -> u64 {
        self.0
    }
}

/// An open document held in the session.
#[derive(Debug, Clone)]
pub struct SessionDocument<P> {
    /// Session-local identity.
    pub id: DocumentId,
    /// Absolute or user-supplied path when saved/opened; `None` for untitled.
    pub path: Option<PathBuf>,
    /// Tab title (file stem when available, or Untitled…).
    pub title: String,
    /// Document-type ids from root to tip (host-defined strings).
    pub type_chain: Vec<String>,
    /// Parallel optional view roles for each chain entry.
    pub roles: Vec<Option<String>>,
    /// Host tip payload.
    pub payload: P,
    /// Whether the in-memory tip differs from the last saved/opened bytes.
    pub dirty: bool,
}

/// Ordered collection of open documents plus the active tab.
#[derive(Debug, Clone)]
pub struct DocumentStore<P> {
    documents: Vec<SessionDocument<P>>,
    active: Option<DocumentId>,
    next_id: u64,
    next_untitled: u32,
}

impl<P> Default for DocumentStore<P> {
    fn default() -> Self {
        Self {
            documents: Vec::new(),
            active: None,
            next_id: 0,
            next_untitled: 0,
        }
    }
}

impl<P> DocumentStore<P> {
    /// Creates an empty store.
    pub fn new() -> Self {
        Self::default()
    }

    /// Open documents in tab order.
    pub fn documents(&self) -> &[SessionDocument<P>] {
        &self.documents
    }

    /// Currently active document id, if any.
    pub fn active_id(&self) -> Option<DocumentId> {
        self.active
    }

    /// Active document, if any.
    pub fn active_document(&self) -> Option<&SessionDocument<P>> {
        let id = self.active?;
        self.documents.iter().find(|doc| doc.id == id)
    }

    /// Mutable active document, if any.
    pub fn active_document_mut(&mut self) -> Option<&mut SessionDocument<P>> {
        let id = self.active?;
        self.documents.iter_mut().find(|doc| doc.id == id)
    }

    /// Finds an open document by filesystem path (lexical equality).
    pub fn find_by_path(&self, path: &Path) -> Option<&SessionDocument<P>> {
        self.documents
            .iter()
            .find(|doc| doc.path.as_deref() == Some(path))
    }

    /// Opens a document at `path`, or activates an existing tab for the same path.
    ///
    /// Returns the document id that is active afterward.
    pub fn open_or_activate(
        &mut self,
        path: PathBuf,
        type_chain: Vec<String>,
        roles: Vec<Option<String>>,
        payload: P,
    ) -> DocumentId {
        if let Some(existing) = self
            .documents
            .iter()
            .find(|doc| doc.path.as_deref() == Some(path.as_path()))
        {
            let id = existing.id;
            self.active = Some(id);
            return id;
        }

        let id = DocumentId(self.next_id);
        self.next_id += 1;
        let title = title_for_path(&path);
        self.documents.push(SessionDocument {
            id,
            path: Some(path),
            title,
            type_chain,
            roles,
            payload,
            dirty: false,
        });
        self.active = Some(id);
        id
    }

    /// Allocates the next untitled title index (host builds the full title string).
    pub fn take_untitled_index(&mut self) -> u32 {
        let index = self.next_untitled;
        self.next_untitled += 1;
        index
    }

    /// Inserts an untitled document and activates it.
    pub fn insert_untitled(
        &mut self,
        title: String,
        type_chain: Vec<String>,
        roles: Vec<Option<String>>,
        payload: P,
    ) -> DocumentId {
        let id = DocumentId(self.next_id);
        self.next_id += 1;
        self.documents.push(SessionDocument {
            id,
            path: None,
            title,
            type_chain,
            roles,
            payload,
            dirty: true,
        });
        self.active = Some(id);
        id
    }

    /// Selects the tab for `id` when it exists.
    pub fn select(&mut self, id: DocumentId) {
        if self.documents.iter().any(|doc| doc.id == id) {
            self.active = Some(id);
        }
    }

    /// Closes the tab for `id` and selects a neighbor when needed.
    pub fn close(&mut self, id: DocumentId) {
        let Some(index) = self.documents.iter().position(|doc| doc.id == id) else {
            return;
        };
        self.documents.remove(index);

        if self.active != Some(id) {
            return;
        }

        self.active = if self.documents.is_empty() {
            None
        } else if index < self.documents.len() {
            Some(self.documents[index].id)
        } else {
            Some(self.documents[index - 1].id)
        };
    }

    /// Document by id.
    pub fn get(&self, id: DocumentId) -> Option<&SessionDocument<P>> {
        self.documents.iter().find(|doc| doc.id == id)
    }

    /// Mutable document by id.
    pub fn get_mut(&mut self, id: DocumentId) -> Option<&mut SessionDocument<P>> {
        self.documents.iter_mut().find(|doc| doc.id == id)
    }

    /// Marks the document dirty after an in-place edit.
    pub fn mark_dirty(&mut self, id: DocumentId) {
        if let Some(doc) = self.get_mut(id) {
            doc.dirty = true;
        }
    }

    /// Assigns a filesystem path after Save As and clears dirty.
    pub fn set_path_and_clean(&mut self, id: DocumentId, path: PathBuf) {
        if let Some(doc) = self.get_mut(id) {
            doc.title = title_for_path(&path);
            doc.path = Some(path);
            doc.dirty = false;
        }
    }

    /// Clears the dirty flag after a successful save to an existing path.
    pub fn mark_clean(&mut self, id: DocumentId) {
        if let Some(doc) = self.get_mut(id) {
            doc.dirty = false;
        }
    }
}

/// Tab title derived from a filesystem path.
pub fn title_for_path(path: &Path) -> String {
    path.file_stem()
        .and_then(|stem| stem.to_str())
        .map(str::to_owned)
        .or_else(|| {
            path.file_name()
                .and_then(|name| name.to_str())
                .map(str::to_owned)
        })
        .unwrap_or_else(|| path.display().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn open_or_activate_reuses_path() {
        let mut store = DocumentStore::new();
        let id1 = store.open_or_activate(
            PathBuf::from("/tmp/a.json"),
            vec!["json".into()],
            vec![Some("json".into())],
            1u32,
        );
        let id2 = store.open_or_activate(
            PathBuf::from("/tmp/a.json"),
            vec!["json".into()],
            vec![Some("json".into())],
            2u32,
        );
        assert_eq!(id1, id2);
        assert_eq!(store.documents().len(), 1);
        assert_eq!(store.active_id(), Some(id1));
        assert_eq!(store.active_document().expect("doc").payload, 1);
    }

    #[test]
    fn close_selects_neighbor() {
        let mut store = DocumentStore::new();
        let a = store.insert_untitled("A".into(), vec![], vec![], ());
        let b = store.insert_untitled("B".into(), vec![], vec![], ());
        let c = store.insert_untitled("C".into(), vec![], vec![], ());
        store.select(b);
        store.close(b);
        assert_eq!(store.active_id(), Some(c));
        store.close(c);
        assert_eq!(store.active_id(), Some(a));
        store.close(a);
        assert_eq!(store.active_id(), None);
    }

    #[test]
    fn dirty_and_path_lifecycle() {
        let mut store = DocumentStore::new();
        let id = store.insert_untitled("Untitled".into(), vec!["t".into()], vec![None], "payload");
        assert!(store.get(id).expect("doc").dirty);
        store.mark_clean(id);
        assert!(!store.get(id).expect("doc").dirty);
        store.mark_dirty(id);
        assert!(store.get(id).expect("doc").dirty);
        store.set_path_and_clean(id, PathBuf::from("/tmp/saved.json"));
        let doc = store.get(id).expect("doc");
        assert!(!doc.dirty);
        assert_eq!(doc.path.as_deref(), Some(Path::new("/tmp/saved.json")));
        assert_eq!(doc.title, "saved");
    }

    #[test]
    fn untitled_index_increments() {
        let mut store = DocumentStore::<()>::new();
        assert_eq!(store.take_untitled_index(), 0);
        assert_eq!(store.take_untitled_index(), 1);
    }
}
