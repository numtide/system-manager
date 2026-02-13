use im::HashMap;
use serde::{Deserialize, Serialize};
use std::cmp::Eq;
use std::iter::Peekable;
use std::path;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum FileStatus {
    Managed,
    ManagedWithBackup,
    Unmanaged,
}

impl FileStatus {
    fn merge(&self, other: &Self) -> Self {
        use FileStatus::*;

        match (self, other) {
            (Unmanaged, Unmanaged) => Unmanaged,
            (ManagedWithBackup, _) | (_, ManagedWithBackup) => ManagedWithBackup,
            _ => Managed,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileTree {
    status: FileStatus,
    pub(crate) path: PathBuf,
    // TODO directories and files are now both represented as a string associated with a nested
    // map. For files the nested map is simple empty.
    // We could potentially optimise this.
    pub(crate) nested: HashMap<String, FileTree>,
}

impl AsRef<FileTree> for FileTree {
    fn as_ref(&self) -> &FileTree {
        self
    }
}

impl Default for FileTree {
    fn default() -> Self {
        Self::root_node()
    }
}

/// Data structure to represent files that are managed by system-manager.
///
/// This data will be serialised to disk and read on the next run.
///
/// We need these basic operations:
/// 1. Create a new root structure
/// 2. Persist to a file
/// 3. Import from a file
/// 4. Add a path to the tree, that will from then on be considered as managed
/// 5.
impl FileTree {
    fn new(path: PathBuf) -> Self {
        Self::with_status(path, FileStatus::Unmanaged)
    }

    fn with_status(path: PathBuf, status: FileStatus) -> Self {
        Self {
            status,
            path,
            nested: HashMap::new(),
        }
    }

    pub fn root_node() -> Self {
        Self::new(PathBuf::from(path::MAIN_SEPARATOR_STR))
    }

    pub fn get_status<'a>(&'a self, path: &Path) -> &'a FileStatus {
        fn go<'a, 'b, C>(tree: &'a FileTree, mut components: C, path: &Path) -> &'a FileStatus
        where
            C: Iterator<Item = path::Component<'b>>,
        {
            if let Some(component) = components.next() {
                match component {
                    path::Component::Normal(name) => tree
                        .nested
                        .get(name.to_string_lossy().as_ref())
                        .map(|subtree| go(subtree, components, path))
                        .unwrap_or(&FileStatus::Unmanaged),
                    path::Component::RootDir => go(tree, components, path),
                    _ => todo!(),
                }
            } else {
                debug_assert!(tree.path == path);
                &tree.status
            }
        }
        go(self, path.components(), path)
    }

    pub fn is_managed(&self, path: &Path) -> bool {
        matches!(
            self.get_status(path),
            FileStatus::Managed | FileStatus::ManagedWithBackup
        )
    }

    // TODO is recursion OK here?
    // Should we convert to CPS and use a crate like tramp to TCO this?
    pub fn register_managed_entry(self, path: &Path) -> Self {
        self.register_entry(path, FileStatus::Managed)
    }

    pub fn register_backed_up_entry(self, path: &Path) -> Self {
        self.register_entry(path, FileStatus::ManagedWithBackup)
    }

    fn register_entry(self, path: &Path, leaf_status: FileStatus) -> Self {
        fn go<'a, C>(
            mut tree: FileTree,
            mut components: Peekable<C>,
            path: PathBuf,
            leaf_status: &FileStatus,
        ) -> FileTree
        where
            C: Iterator<Item = path::Component<'a>>,
        {
            if let Some(component) = components.next() {
                match component {
                    path::Component::Normal(name) => {
                        let new_path = path.join(component);
                        tree.nested = tree.nested.alter(
                            |maybe_subtree| {
                                Some(go(
                                    maybe_subtree.unwrap_or_else(|| {
                                        FileTree::with_status(
                                            new_path.clone(),
                                            // We only label with the leaf status the final path
                                            // entry, to label intermediate nodes as managed, we
                                            // should call this function for every one of them
                                            // separately.
                                            components.peek().map_or(leaf_status.clone(), |_| {
                                                FileStatus::Unmanaged
                                            }),
                                        )
                                    }),
                                    components,
                                    new_path,
                                    leaf_status,
                                ))
                            },
                            name.to_string_lossy().to_string(),
                        );
                        tree
                    }
                    path::Component::RootDir => go(
                        tree,
                        components,
                        path.join(path::MAIN_SEPARATOR_STR),
                        leaf_status,
                    ),
                    _ => panic!(
                        "Unsupported path provided! At path component: {:?}",
                        component
                    ),
                }
            } else {
                tree
            }
        }

        go(
            self,
            path.components().peekable(),
            PathBuf::new(),
            &leaf_status,
        )
    }

    pub fn deactivate<F>(self, delete_action: &F) -> Option<FileTree>
    where
        F: Fn(&Path, &FileStatus) -> bool,
    {
        let new_tree = self.nested.keys().fold(self.clone(), |mut new_tree, name| {
            new_tree.nested = new_tree.nested.alter(
                |subtree| subtree.and_then(|subtree| subtree.deactivate(delete_action)),
                name.to_owned(),
            );
            new_tree
        });

        // We clean up nodes that are empty and unmanaged.
        // These represent intermediate directories that already existed, so we
        // are not responsible for cleaning them up (we don't run the delete_action
        // closure on their paths).
        if new_tree.nested.is_empty() {
            if matches!(
                new_tree.status,
                FileStatus::Managed | FileStatus::ManagedWithBackup
            ) {
                if delete_action(&new_tree.path, &new_tree.status) {
                    None
                } else {
                    Some(new_tree)
                }
            } else {
                None
            }
        } else {
            Some(new_tree)
        }
    }

    pub fn update_state<F>(self, other: Self, delete_action: &F) -> Option<Self>
    where
        F: Fn(&Path, &FileStatus) -> bool,
    {
        let to_deactivate = other
            .nested
            .clone()
            .relative_complement(self.nested.clone());
        let to_merge = other.nested.intersection(self.nested.clone());

        let deactivated = to_deactivate
            .into_iter()
            .fold(self, |mut new_tree, (name, subtree)| {
                subtree
                    .deactivate(delete_action)
                    .into_iter()
                    .for_each(|subtree| {
                        new_tree.nested.insert(name.to_owned(), subtree);
                    });
                new_tree
            });

        let merged = to_merge
            .into_iter()
            .fold(deactivated, |mut new_tree, (name, other_tree)| {
                new_tree.nested = new_tree.nested.alter(
                    |subtree| {
                        subtree.and_then(|subtree| {
                            subtree.update_state(other_tree.clone(), delete_action).map(
                                |mut new_tree| {
                                    new_tree.status = new_tree.status.merge(&other_tree.status);
                                    new_tree
                                },
                            )
                        })
                    },
                    name,
                );
                new_tree
            });

        // If our invariants are properly maintained, then we should never end up
        // here with dangling unmanaged nodes.
        debug_assert!(
            !merged.nested.is_empty()
                || matches!(
                    merged.status,
                    FileStatus::Managed | FileStatus::ManagedWithBackup
                )
        );

        Some(merged)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use itertools::Itertools;

    impl FileTree {
        pub fn deactivate_managed_entry<F>(self, path: &Path, delete_action: &F) -> Self
        where
            F: Fn(&Path, &FileStatus) -> bool,
        {
            fn go<'a, C, F>(
                mut tree: FileTree,
                path: PathBuf,
                mut components: Peekable<C>,
                delete_action: &F,
            ) -> FileTree
            where
                C: Iterator<Item = path::Component<'a>>,
                F: Fn(&Path, &FileStatus) -> bool,
            {
                log::debug!("Deactivating {}", path.display());

                if let Some(component) = components.next() {
                    match component {
                        path::Component::Normal(name) => {
                            let new_path = path.join(name);
                            tree.nested = tree.nested.alter(
                                |maybe_subtree| {
                                    maybe_subtree.and_then(|subtree| {
                                        if components.peek().is_some() {
                                            Some(go(subtree, new_path, components, delete_action))
                                        } else {
                                            subtree.deactivate(delete_action)
                                        }
                                    })
                                },
                                name.to_string_lossy().to_string(),
                            );
                            tree
                        }
                        path::Component::RootDir => go(
                            tree,
                            path.join(path::MAIN_SEPARATOR.to_string()),
                            components,
                            delete_action,
                        ),
                        _ => panic!(
                            "Unsupported path provided! At path component: {:?}",
                            component
                        ),
                    }
                } else {
                    tree
                }
            }
            go(
                self,
                PathBuf::new(),
                path.components().peekable(),
                delete_action,
            )
        }
    }

    #[test]
    fn get_status() {
        let tree1 = FileTree::root_node()
            .register_managed_entry(&PathBuf::from("/").join("foo").join("bar"))
            .register_managed_entry(&PathBuf::from("/").join("foo2"))
            .register_managed_entry(&PathBuf::from("/").join("foo2").join("baz"))
            .register_managed_entry(&PathBuf::from("/").join("foo2").join("baz").join("bar"))
            .register_managed_entry(&PathBuf::from("/").join("foo2").join("baz2"))
            .register_managed_entry(&PathBuf::from("/").join("foo2").join("baz2").join("bar"))
            .register_managed_entry(&PathBuf::from("/").join("foo3").join("baz2").join("bar"))
            .register_managed_entry(&PathBuf::from("/").join("foo4"))
            .register_managed_entry(&PathBuf::from("/").join("foo4").join("baz"))
            .register_managed_entry(&PathBuf::from("/").join("foo4").join("baz").join("bar"))
            .register_managed_entry(&PathBuf::from("/").join("foo5"))
            .register_managed_entry(&PathBuf::from("/").join("foo5").join("baz"))
            .register_managed_entry(&PathBuf::from("/").join("foo5").join("baz2"))
            .register_managed_entry(&PathBuf::from("/").join("foo5").join("baz").join("bar"));

        assert!(tree1.is_managed(&PathBuf::from("/").join("foo5").join("baz").join("bar")));
        assert!(!tree1.is_managed(&PathBuf::from("/").join("foo")));
        assert!(!tree1.is_managed(&PathBuf::from("/").join("foo").join("nonexistent")));
    }

    #[test]
    fn register() {
        let tree = FileTree::root_node()
            .register_managed_entry(&PathBuf::from("/").join("foo").join("bar"))
            .register_managed_entry(&PathBuf::from("/").join("foo2").join("baz").join("bar"))
            .register_managed_entry(&PathBuf::from("/").join("foo2").join("baz2").join("bar"));
        dbg!(&tree);
        assert_eq!(
            tree.nested.keys().sorted().collect::<Vec<_>>(),
            ["foo", "foo2"]
        );
        assert_eq!(
            tree.nested
                .get("foo2")
                .unwrap()
                .nested
                .get("baz")
                .unwrap()
                .nested
                .get("bar")
                .unwrap()
                .path,
            PathBuf::from("/").join("foo2").join("baz").join("bar")
        );
    }

    #[test]
    fn deactivate() {
        let tree1 = FileTree::root_node()
            .register_managed_entry(&PathBuf::from("/").join("foo").join("bar"))
            .register_managed_entry(&PathBuf::from("/").join("foo2"))
            .register_managed_entry(&PathBuf::from("/").join("foo2").join("baz"))
            .register_managed_entry(&PathBuf::from("/").join("foo2").join("baz").join("bar"))
            .register_managed_entry(&PathBuf::from("/").join("foo2").join("baz2"))
            .register_managed_entry(&PathBuf::from("/").join("foo2").join("baz2").join("bar"))
            .register_managed_entry(&PathBuf::from("/").join("foo3").join("baz2").join("bar"))
            .register_managed_entry(&PathBuf::from("/").join("foo4"))
            .register_managed_entry(&PathBuf::from("/").join("foo4").join("baz"))
            .register_managed_entry(&PathBuf::from("/").join("foo4").join("baz").join("bar"))
            .register_managed_entry(&PathBuf::from("/").join("foo5"))
            .register_managed_entry(&PathBuf::from("/").join("foo5").join("baz"))
            .register_managed_entry(&PathBuf::from("/").join("foo5").join("baz2"))
            .register_managed_entry(&PathBuf::from("/").join("foo5").join("baz").join("bar"));
        let tree2 = tree1
            .clone()
            .deactivate_managed_entry(&PathBuf::from("/").join("foo4"), &|path, _status| {
                println!("Deactivating: {}", path.display());
                false
            })
            .deactivate_managed_entry(&PathBuf::from("/").join("foo2"), &|path, _status| {
                println!("Deactivating: {}", path.display());
                true
            })
            .deactivate_managed_entry(&PathBuf::from("/").join("foo3"), &|path, _status| {
                println!("Deactivating: {}", path.display());
                true
            })
            .deactivate_managed_entry(
                &PathBuf::from("/").join("foo5").join("baz"),
                &|path, _status| {
                    println!("Deactivating: {}", path.display());
                    true
                },
            );
        dbg!(&tree1);
        assert_eq!(
            tree2.nested.keys().sorted().collect::<Vec<_>>(),
            ["foo", "foo4", "foo5"]
        );
        assert!(tree2
            .nested
            .get("foo5")
            .unwrap()
            .nested
            .get("baz2")
            .unwrap()
            .nested
            .keys()
            .sorted()
            .collect::<Vec<_>>()
            .is_empty());
        assert_eq!(
            tree1.nested.keys().sorted().collect::<Vec<_>>(),
            ["foo", "foo2", "foo3", "foo4", "foo5"]
        );
    }

    #[test]
    fn managed_with_backup_is_managed() {
        let tree = FileTree::root_node()
            .register_backed_up_entry(&PathBuf::from("/").join("foo").join("bar"));

        assert!(tree.is_managed(&PathBuf::from("/").join("foo").join("bar")));
        assert!(!tree.is_managed(&PathBuf::from("/").join("foo")));
    }

    #[test]
    fn register_backed_up_entry_sets_status() {
        let tree = FileTree::root_node()
            .register_backed_up_entry(&PathBuf::from("/").join("etc").join("nix.conf"));

        assert_eq!(
            *tree.get_status(&PathBuf::from("/").join("etc").join("nix.conf")),
            FileStatus::ManagedWithBackup,
        );
        assert_eq!(
            *tree.get_status(&PathBuf::from("/").join("etc")),
            FileStatus::Unmanaged,
        );
    }

    #[test]
    fn merge_preserves_managed_with_backup() {
        assert_eq!(
            FileStatus::ManagedWithBackup.merge(&FileStatus::Unmanaged),
            FileStatus::ManagedWithBackup,
        );
        assert_eq!(
            FileStatus::ManagedWithBackup.merge(&FileStatus::Managed),
            FileStatus::ManagedWithBackup,
        );
        assert_eq!(
            FileStatus::Managed.merge(&FileStatus::ManagedWithBackup),
            FileStatus::ManagedWithBackup,
        );
        assert_eq!(
            FileStatus::Unmanaged.merge(&FileStatus::ManagedWithBackup),
            FileStatus::ManagedWithBackup,
        );
        assert_eq!(
            FileStatus::ManagedWithBackup.merge(&FileStatus::ManagedWithBackup),
            FileStatus::ManagedWithBackup,
        );
    }

    #[test]
    fn deactivate_passes_backup_status_to_action() {
        let tree = FileTree::root_node()
            .register_backed_up_entry(&PathBuf::from("/").join("etc").join("nix.conf"))
            .register_managed_entry(&PathBuf::from("/").join("etc").join("other"));

        let statuses = std::cell::RefCell::new(Vec::<(PathBuf, FileStatus)>::new());
        tree.deactivate(&|path: &Path, status: &FileStatus| {
            statuses
                .borrow_mut()
                .push((path.to_owned(), status.clone()));
            true
        });

        let statuses = statuses.into_inner();
        let backup_entries: Vec<_> = statuses
            .iter()
            .filter(|(_, s)| *s == FileStatus::ManagedWithBackup)
            .collect();
        assert_eq!(backup_entries.len(), 1);
        assert_eq!(
            backup_entries[0].0,
            PathBuf::from("/").join("etc").join("nix.conf")
        );

        let managed_entries: Vec<_> = statuses
            .iter()
            .filter(|(_, s)| *s == FileStatus::Managed)
            .collect();
        assert_eq!(managed_entries.len(), 1);
        assert_eq!(
            managed_entries[0].0,
            PathBuf::from("/").join("etc").join("other")
        );
    }

    #[test]
    fn mixed_managed_and_backed_up() {
        let tree = FileTree::root_node()
            .register_managed_entry(&PathBuf::from("/").join("foo").join("bar"))
            .register_backed_up_entry(&PathBuf::from("/").join("foo").join("baz"));

        assert!(tree.is_managed(&PathBuf::from("/").join("foo").join("bar")));
        assert!(tree.is_managed(&PathBuf::from("/").join("foo").join("baz")));
        assert_eq!(
            *tree.get_status(&PathBuf::from("/").join("foo").join("bar")),
            FileStatus::Managed,
        );
        assert_eq!(
            *tree.get_status(&PathBuf::from("/").join("foo").join("baz")),
            FileStatus::ManagedWithBackup,
        );
    }

    #[test]
    fn update_state() {
        let tree1 = FileTree::root_node()
            .register_managed_entry(&PathBuf::from("/").join("foo").join("bar"))
            .register_managed_entry(&PathBuf::from("/").join("foo2"))
            .register_managed_entry(&PathBuf::from("/").join("foo2").join("baz"))
            .register_managed_entry(&PathBuf::from("/").join("foo2").join("baz").join("bar"))
            .register_managed_entry(&PathBuf::from("/").join("foo2").join("baz2"))
            .register_managed_entry(&PathBuf::from("/").join("foo2").join("baz2").join("bar"))
            .register_managed_entry(&PathBuf::from("/").join("foo3").join("baz2").join("bar"));
        let tree2 = FileTree::root_node()
            .register_managed_entry(&PathBuf::from("/").join("foo").join("bar"))
            .register_managed_entry(&PathBuf::from("/").join("foo3").join("bar"))
            .register_managed_entry(&PathBuf::from("/").join("foo4"))
            .register_managed_entry(&PathBuf::from("/").join("foo4").join("bar"))
            .register_managed_entry(&PathBuf::from("/").join("foo5"))
            .register_managed_entry(&PathBuf::from("/").join("foo5").join("bar"));
        let new_tree = tree1.update_state(tree2, &|path, _status| {
            println!("Deactivating path: {}", path.display());
            *path != PathBuf::from("/").join("foo5").join("bar")
        });
        assert_eq!(
            new_tree.unwrap().nested.keys().sorted().collect::<Vec<_>>(),
            ["foo", "foo2", "foo3", "foo5"]
        );
    }
}
