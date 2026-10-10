use thiserror::Error;
use tracing::error;

use std::{
    fs,
    path::{Path, PathBuf},
};

use crate::phy::cab::{Cabinet, build_cabinet_from_path};

#[derive(Debug, Error)]
pub enum StorageError {}

#[derive(Debug, Clone)]
pub struct StorageInfo {
    pub abs_path: PathBuf,
}

#[derive(Debug)]
pub struct Storage {
    pub stg_info: StorageInfo,
    // relations
    pub cabs: Vec<Cabinet>,
}

/// A non-fatal problem met while scanning storages and cabinets.
/// Scanning keeps going so a single broken cabinet never hides the rest.
#[derive(Debug, Clone)]
pub struct ScanIssue {
    pub path: PathBuf,
    pub message: String,
}

/// Where storages are looked for: every mounted disk, plus the paths
/// declared by the user config (`custom_storages`).
pub fn storage_search_paths() -> Vec<PathBuf> {
    let mut paths = crate::phy::dk::disks();

    for custom in crate::user::user_conf::user_conf().custom_storages {
        let path = PathBuf::from(custom);
        if !paths.contains(&path) {
            paths.push(path);
        }
    }

    paths
}

/// Scan the given storage roots, collecting both the storages found and
/// the issues met on the way.
pub fn scan_storages_with_issues(stg_abs_paths: &[PathBuf]) -> (Vec<Storage>, Vec<ScanIssue>) {
    let mut stgs: Vec<Storage> = Vec::new();
    let mut issues: Vec<ScanIssue> = Vec::new();

    // ex: /System/Volumes/Samsung SSD, D:\
    for stg_abs_path in stg_abs_paths {
        let stg_ents = match fs::read_dir(stg_abs_path) {
            Ok(t) => t,
            Err(e) => {
                error!(
                    "Failed to read storage path {}: {}",
                    stg_abs_path.display(),
                    e
                );
                issues.push(ScanIssue {
                    path: stg_abs_path.to_path_buf(),
                    message: format!("Failed to read storage path: {}", e),
                });
                continue;
            }
        };

        let mut cabs: Vec<Cabinet> = Vec::new();
        // ex: /System/Volumes/Samsung SSD/anime123/, D:\game\
        for stg_ent in stg_ents {
            let f = match stg_ent {
                Ok(t) => t,
                Err(e) => {
                    error!("Failed to read entry in {}: {}", stg_abs_path.display(), e);
                    issues.push(ScanIssue {
                        path: stg_abs_path.to_path_buf(),
                        message: format!("Failed to read entry: {}", e),
                    });
                    continue;
                }
            };
            let f_path = f.path();

            let f_type = match f.file_type() {
                Ok(t) => t,
                Err(e) => {
                    error!("Failed to read entry in {}: {}", f_path.display(), e);
                    issues.push(ScanIssue {
                        path: f_path.clone(),
                        message: format!("Failed to read entry: {}", e),
                    });
                    continue;
                }
            };

            if f_type.is_dir() {
                // A plain directory is not a cabinet. Only report something when
                // the directory claims to be one but its config cannot be used,
                // otherwise every unrelated folder would show up as an issue.
                if !f_path.join(env!("CABINET_CONFIG_NAME")).exists() {
                    continue;
                }

                let cab = match build_cabinet_from_path(f_path.as_path()) {
                    Ok(t) => t,
                    Err(e) => {
                        error!("Failed to build cabinet from {}: {}", f_path.display(), e);
                        issues.push(ScanIssue {
                            path: f_path.clone(),
                            message: format!("Failed to build cabinet: {}", e),
                        });
                        continue;
                    }
                };

                cabs.push(cab);
            }
        }

        if !cabs.is_empty() {
            let stg: Storage = Storage {
                stg_info: StorageInfo {
                    abs_path: stg_abs_path.to_path_buf(),
                },
                cabs,
            };
            stgs.push(stg);
        }
    }

    (stgs, issues)
}

/// Scan every candidate storage root.
pub fn scan_storages() -> (Vec<Storage>, Vec<ScanIssue>) {
    scan_storages_with_issues(&storage_search_paths())
}

pub fn build_storages_from_paths(stg_abs_paths: &Vec<&Path>) -> Vec<Storage> {
    let paths: Vec<PathBuf> = stg_abs_paths.iter().map(|p| p.to_path_buf()).collect();
    scan_storages_with_issues(&paths).0
}
