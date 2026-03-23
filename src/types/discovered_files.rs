use std::collections::HashMap;

use super::file::DiscoveredFile;

/// Collection of discovered files, keyed by their source absolute path.
///
/// This is a simple `HashMap` wrapper. Unlike `DiscoveredSymbols`, this does not
/// need concurrent access since file discovery typically happens in a single pass
/// before parallel parsing begins.
#[derive(Debug, Clone)]
pub struct DiscoveredFiles {
    files: HashMap<String, DiscoveredFile>,
}

impl Default for DiscoveredFiles {
    fn default() -> Self {
        Self::new()
    }
}

impl DiscoveredFiles {
    /// Create a new empty collection.
    pub fn new() -> Self {
        Self {
            files: HashMap::new(),
        }
    }

    /// Add a file to the collection, keyed by its source absolute path.
    pub fn add(&mut self, file: DiscoveredFile) {
        let key = file.get_source_path().to_string();
        self.files.insert(key, file);
    }

    /// Get all files as a reference to the underlying map.
    pub fn get_files(&self) -> &HashMap<String, DiscoveredFile> {
        &self.files
    }

    /// Get a mutable reference to all files.
    pub fn get_files_mut(&mut self) -> &mut HashMap<String, DiscoveredFile> {
        &mut self.files
    }

    /// Look up a file by its source absolute path.
    pub fn get_file(&self, source_absolute_path: &str) -> Option<&DiscoveredFile> {
        self.files.get(source_absolute_path)
    }

    /// Get a mutable reference to a file by its source absolute path.
    pub fn get_file_mut(&mut self, source_absolute_path: &str) -> Option<&mut DiscoveredFile> {
        self.files.get_mut(source_absolute_path)
    }

    /// Sort the files by key (source path). Consumes and rebuilds the internal map
    /// using a `BTreeMap`-like ordering approach.
    ///
    /// Note: Since `HashMap` does not maintain order, this replaces the internal map
    /// with entries in sorted key order. Iteration order after calling this method
    /// is not guaranteed by `HashMap` itself; if ordered iteration is needed,
    /// use `get_files_sorted()` instead.
    pub fn sort(&mut self) {
        let mut entries: Vec<(String, DiscoveredFile)> = self.files.drain().collect();
        entries.sort_by(|(a, _), (b, _)| a.cmp(b));
        self.files = entries.into_iter().collect();
    }

    /// Get all files sorted by their source path key.
    pub fn get_files_sorted(&self) -> Vec<(&String, &DiscoveredFile)> {
        let mut entries: Vec<_> = self.files.iter().collect();
        entries.sort_by_key(|(key, _)| key.clone());
        entries
    }

    /// Number of files in the collection.
    pub fn len(&self) -> usize {
        self.files.len()
    }

    /// Whether the collection is empty.
    pub fn is_empty(&self) -> bool {
        self.files.is_empty()
    }

    /// Iterate over all files.
    pub fn iter(&self) -> impl Iterator<Item = (&String, &DiscoveredFile)> {
        self.files.iter()
    }

    /// Iterate mutably over all files.
    pub fn iter_mut(&mut self) -> impl Iterator<Item = (&String, &mut DiscoveredFile)> {
        self.files.iter_mut()
    }

    /// Get all files as a Vec reference (convenience for pipeline code).
    pub fn files(&self) -> Vec<&DiscoveredFile> {
        self.files.values().collect()
    }

    /// Get all files as mutable Vec (convenience for pipeline code).
    pub fn files_mut(&mut self) -> Vec<&mut DiscoveredFile> {
        self.files.values_mut().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn make_file(path: &str) -> DiscoveredFile {
        DiscoveredFile::new(PathBuf::from(path), path.to_string())
    }

    #[test]
    fn test_add_and_get() {
        let mut files = DiscoveredFiles::new();
        files.add(make_file("/project/src/Foo.php"));
        files.add(make_file("/project/src/Bar.php"));

        assert_eq!(files.len(), 2);
        assert!(!files.is_empty());

        let foo = files.get_file("/project/src/Foo.php");
        assert!(foo.is_some());
        assert_eq!(foo.unwrap().vendor_relative_path, "/project/src/Foo.php");

        let missing = files.get_file("/project/src/Missing.php");
        assert!(missing.is_none());
    }

    #[test]
    fn test_overwrite_on_duplicate_key() {
        let mut files = DiscoveredFiles::new();
        let mut file1 = make_file("/project/src/Foo.php");
        file1.do_prefix = false;

        let mut file2 = make_file("/project/src/Foo.php");
        file2.do_prefix = true;

        files.add(file1);
        files.add(file2);

        assert_eq!(files.len(), 1);
        let foo = files.get_file("/project/src/Foo.php").unwrap();
        assert!(foo.do_prefix); // second add wins
    }

    #[test]
    fn test_get_files_sorted() {
        let mut files = DiscoveredFiles::new();
        files.add(make_file("/z/file.php"));
        files.add(make_file("/a/file.php"));
        files.add(make_file("/m/file.php"));

        let sorted = files.get_files_sorted();
        assert_eq!(sorted.len(), 3);
        assert_eq!(sorted[0].0, "/a/file.php");
        assert_eq!(sorted[1].0, "/m/file.php");
        assert_eq!(sorted[2].0, "/z/file.php");
    }

    #[test]
    fn test_empty() {
        let files = DiscoveredFiles::new();
        assert!(files.is_empty());
        assert_eq!(files.len(), 0);
    }

    #[test]
    fn test_get_file_mut() {
        let mut files = DiscoveredFiles::new();
        files.add(make_file("/project/src/Foo.php"));

        let foo = files.get_file_mut("/project/src/Foo.php").unwrap();
        foo.do_prefix = true;
        foo.did_update = true;

        let foo = files.get_file("/project/src/Foo.php").unwrap();
        assert!(foo.do_prefix);
        assert!(foo.did_update);
    }

    #[test]
    fn test_iter() {
        let mut files = DiscoveredFiles::new();
        files.add(make_file("/a.php"));
        files.add(make_file("/b.php"));

        let count = files.iter().count();
        assert_eq!(count, 2);
    }
}
