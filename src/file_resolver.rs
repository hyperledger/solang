// SPDX-License-Identifier: Apache-2.0

use crate::sema::ast;
use itertools::Itertools;
use normalize_path::NormalizePath;
use solang_parser::pt::Loc;
use std::collections::HashMap;
use std::ffi::{OsStr, OsString};
use std::fs::File;
use std::io::prelude::*;
use std::path::{Path, PathBuf};
use std::sync::Arc;

#[derive(Clone, Default)]
pub struct FileResolver {
    /// Set of import paths search for imports
    import_paths: Vec<(Option<OsString>, PathBuf)>,
    /// List file by path
    cached_paths: HashMap<PathBuf, usize>,
    /// The actual file contents
    files: Vec<ResolvedFile>,
}

/// When we resolve a file, we need to know its base compared to the import so
/// we can resolve the next import, and the full path on the filesystem.
/// Since the same filename can exists in multiple imports, we need to tell the
/// user exactly which file has errors/warnings.
#[derive(Clone, Debug)]
pub struct ResolvedFile {
    /// Original name used on cli or import statement
    pub path: OsString,
    /// Full path on the filesystem
    pub full_path: PathBuf,
    /// Which import path was used, if any
    pub import_no: Option<usize>,
    /// The actual file contents
    pub contents: Arc<str>,
}

impl FileResolver {
    /// Add import path
    pub fn add_import_path(&mut self, path: &Path) {
        assert!(!self.import_paths.contains(&(None, path.to_path_buf())));

        self.import_paths.push((None, path.to_path_buf()));
    }

    /// Add import map
    pub fn add_import_map(&mut self, map: OsString, path: PathBuf) {
        let map = Some(map);

        if let Some((_, e)) = self.import_paths.iter_mut().find(|(k, _)| *k == map) {
            *e = path;
        } else {
            self.import_paths.push((map, path));
        }
    }

    /// Get the import path and the optional mapping corresponding to `import_no`.
    pub fn get_import_path(&self, import_no: usize) -> Option<&(Option<OsString>, PathBuf)> {
        self.import_paths.get(import_no)
    }

    /// Get the import paths
    pub fn get_import_paths(&self) -> &[(Option<OsString>, PathBuf)] {
        self.import_paths.as_slice()
    }

    /// Get the import path corresponding to a map
    pub fn get_import_map(&self, map: &OsString) -> Option<&PathBuf> {
        self.import_paths
            .iter()
            .find(|(m, _)| m.as_ref() == Some(map))
            .map(|(_, pb)| pb)
    }

    /// Update the cache for the filename with the given contents
    pub fn set_file_contents(&mut self, path: &str, contents: String) {
        let pos = self.files.len();

        let pathbuf = PathBuf::from(path);

        self.files.push(ResolvedFile {
            path: path.into(),
            full_path: pathbuf.clone(),
            contents: Arc::from(contents),
            import_no: None,
        });

        self.cached_paths.insert(pathbuf, pos);
    }

    /// Get the file contents of `file_no`th file if it exists
    pub fn get_contents_of_file_no(&self, file_no: usize) -> Option<Arc<str>> {
        self.files.get(file_no).map(|f| f.contents.clone())
    }

    /// Get file with contents. This must be a file which was previously
    /// add to the cache
    pub fn get_file_contents_and_number(&self, file: &Path) -> (Arc<str>, usize) {
        let file_no = self.cached_paths[file];

        (self.files[file_no].contents.clone(), file_no)
    }

    /// Atempt to resolve a file, either from the cache or from the filesystem.
    /// Returns Ok(Some(..)) if the file is found and loaded
    /// Returns Ok(None) if no file by this path can be found.
    /// Returns Err(..) if a file was found but could not be read.
    fn try_file(
        &mut self,
        filename: &OsStr,
        path: &Path,
        import_no: Option<usize>,
    ) -> Result<Option<ResolvedFile>, String> {
        // For accessing the cache, remove "." and ".." path components
        let cache_path = path.normalize();

        if let Some(cache) = self.cached_paths.get(&cache_path) {
            let mut file = self.files[*cache].clone();
            file.import_no = import_no;
            return Ok(Some(file));
        }

        if let Ok(full_path) = path.canonicalize() {
            let file = self.load_file(filename, &full_path, import_no)?;
            return Ok(Some(file.clone()));
        }

        Ok(None)
    }

    /// Populate the cache with absolute file path
    fn load_file(
        &mut self,
        filename: &OsStr,
        path: &Path,
        import_no: Option<usize>,
    ) -> Result<&ResolvedFile, String> {
        let path_filename = PathBuf::from(filename);
        if let Some(cache) = self.cached_paths.get(&path_filename) {
            if self.files[*cache].import_no == import_no {
                return Ok(&self.files[*cache]);
            }
        }

        // read the file
        let mut f = match File::open(path) {
            Err(err_info) => {
                return Err(format!(
                    "cannot open file '{}': {}",
                    path.display(),
                    err_info
                ));
            }
            Ok(file) => file,
        };

        let mut contents = String::new();
        if let Err(e) = f.read_to_string(&mut contents) {
            return Err(format!("failed to read file '{}': {}", path.display(), e));
        }

        let pos = self.files.len();

        self.files.push(ResolvedFile {
            path: filename.into(),
            full_path: path.to_path_buf(),
            import_no,
            contents: Arc::from(contents),
        });

        self.cached_paths.insert(path.to_path_buf(), pos);

        Ok(&self.files[pos])
    }

    /// Walk the import path to search for a file. If no import path is set up,
    /// return. Check each import path if the file can be found in a subdirectory
    /// of that path, and return the canonicalized path.
    pub fn resolve_file(
        &mut self,
        parent: Option<&ResolvedFile>,
        filename: &OsStr,
    ) -> Result<ResolvedFile, String> {
        let path_filename = PathBuf::from(filename);

        // See https://docs.soliditylang.org/en/v0.8.17/path-resolution.html
        let mut result: Vec<ResolvedFile> = vec![];

        // Only when the path starts with ./ or ../ are relative paths considered; this means
        // that `import "b.sol";` will check the import paths for b.sol, while `import "./b.sol";`
        // will only the path relative to the current file.
        if path_filename.starts_with("./") || path_filename.starts_with("../") {
            if let Some(ResolvedFile {
                import_no,
                full_path,
                ..
            }) = parent
            {
                let curdir = PathBuf::from(".");
                let base = full_path.parent().unwrap_or(&curdir);
                let path = base.join(&path_filename);

                if let Some(file) = self.try_file(filename, &path, *import_no)? {
                    // No ambiguity possible, so just return
                    return Ok(file);
                }
            }

            return Err(format!("file not found '{}'", path_filename.display()));
        }

        if parent.is_none() {
            if let Some(file) = self.try_file(filename, &path_filename, None)? {
                return Ok(file);
            } else if path_filename.is_absolute() {
                return Err(format!("file not found '{}'", path_filename.display()));
            }
        }

        // first check maps
        let mut best_match: Option<(&OsString, &PathBuf, &Path)> = None;

        for (mapping, target) in &self.import_paths {
            let Some(mapping) = mapping else {
                continue;
            };

            let Ok(relpath) = path_filename.strip_prefix(mapping) else {
                continue;
            };

            if best_match
                .as_ref()
                .is_none_or(|(best_mapping, _, _)| mapping.len() >= best_mapping.len())
            {
                best_match = Some((mapping, target, relpath));
            }
        }

        let path = best_match
            .map(|(_, target, relpath)| target.join(relpath))
            .unwrap_or(path_filename.clone());

        // walk over the import paths until we find one that resolves
        for import_no in 0..self.import_paths.len() {
            if let (None, import_path) = &self.import_paths[import_no] {
                let path = import_path.join(&path);

                if let Some(file) = self.try_file(filename, &path, Some(import_no))? {
                    result.push(file);
                }
            }
        }

        // If there was no defined import path, then try the file directly. See
        // https://docs.soliditylang.org/en/v0.8.17/path-resolution.html#base-path-and-include-paths
        // "By default the base path is empty, which leaves the source unit name unchanged."
        if !self.import_paths.iter().any(|(m, _)| m.is_none()) {
            if let Some(file) = self.try_file(filename, &path, None)? {
                result.push(file);
            }
        }

        match result.len() {
            0 => Err(format!("file not found '{}'", path_filename.display())),
            1 => Ok(result.pop().unwrap()),
            _ => Err(format!(
                "found multiple files matching '{}': {}",
                path_filename.display(),
                result
                    .iter()
                    .map(|f| format!("'{}'", f.full_path.display()))
                    .join(", ")
            )),
        }
    }

    /// Get line and the target symbol's offset from loc
    pub fn get_line_and_offset_from_loc(
        &self,
        file: &ast::File,
        loc: &Loc,
    ) -> (String, usize, usize, usize) {
        let (start, end) = if let Loc::File(_, start, end) = loc {
            (start, end)
        } else {
            unreachable!();
        };
        let cache_no = file.cache_no.unwrap();
        let (begin_line, mut begin_column) = file.offset_to_line_column(*start);
        let (end_line, mut end_column) = file.offset_to_line_column(*end);

        let mut full_line = self.files[cache_no]
            .contents
            .lines()
            .nth(begin_line)
            .unwrap()
            .to_owned();

        // If the loc spans across multiple lines, we concatenate them
        if begin_line != end_line {
            for i in begin_line + 1..=end_line {
                let line = self.files[cache_no].contents.lines().nth(i).unwrap();
                if i == end_line {
                    end_column += full_line.len();
                }
                full_line.push_str(line);
            }
        }

        let old_size = full_line.len();
        full_line = full_line.trim_start().parse().unwrap();

        // Calculate the size of the symbol we want to highlight
        let size = end_column - begin_column;

        // Update the offset after trimming the line
        begin_column -= old_size - full_line.len();

        (full_line, begin_line, begin_column, size)
    }
}

#[cfg(test)]
mod tests {
    use super::FileResolver;
    use std::ffi::OsStr;
    use std::fs;

    fn resolver_with_remappings(short_first: bool) -> (tempfile::TempDir, FileResolver) {
        let tempdir = tempfile::tempdir().unwrap();
        let base_target = tempdir.path().join("vendor/base/math");
        let math_target = tempdir.path().join("vendor/math");

        fs::create_dir_all(&base_target).unwrap();
        fs::create_dir_all(&math_target).unwrap();
        fs::write(base_target.join("Calc.sol"), "base").unwrap();
        fs::write(math_target.join("Calc.sol"), "math").unwrap();

        let mut resolver = FileResolver::default();
        let short_target = tempdir.path().join("vendor/base");
        let long_target = tempdir.path().join("vendor/math");

        if short_first {
            resolver.add_import_map("@lib/".into(), short_target);
            resolver.add_import_map("@lib/math/".into(), long_target);
        } else {
            resolver.add_import_map("@lib/math/".into(), long_target);
            resolver.add_import_map("@lib/".into(), short_target);
        }

        (tempdir, resolver)
    }

    fn assert_longest_remapping_wins(short_first: bool) {
        let (_tempdir, mut resolver) = resolver_with_remappings(short_first);
        let file = resolver
            .resolve_file(None, OsStr::new("@lib/math/Calc.sol"))
            .unwrap();

        assert_eq!(file.contents.as_ref(), "math");
    }

    #[test]
    fn longest_remapping_wins_when_short_prefix_is_first() {
        assert_longest_remapping_wins(true);
    }

    #[test]
    fn longest_remapping_wins_when_long_prefix_is_first() {
        assert_longest_remapping_wins(false);
    }

    #[test]
    fn duplicate_remapping_prefix_uses_last_target() {
        let tempdir = tempfile::tempdir().unwrap();
        let first_target = tempdir.path().join("vendor/first");
        let second_target = tempdir.path().join("vendor/second");

        fs::create_dir_all(&first_target).unwrap();
        fs::create_dir_all(&second_target).unwrap();
        fs::write(first_target.join("Calc.sol"), "first").unwrap();
        fs::write(second_target.join("Calc.sol"), "second").unwrap();

        let mut resolver = FileResolver::default();
        resolver.add_import_map("@lib/".into(), first_target);
        resolver.add_import_map("@lib/".into(), second_target);

        let file = resolver
            .resolve_file(None, OsStr::new("@lib/Calc.sol"))
            .unwrap();

        assert_eq!(file.contents.as_ref(), "second");
    }
}
