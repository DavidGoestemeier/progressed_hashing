use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::{io};
use std::sync::{Arc};
use std::sync::atomic::{AtomicUsize, Ordering};
use futures::{Stream};
use rayon::iter::IntoParallelRefIterator;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;
use walkdir::WalkDir;
use tokio_stream::wrappers::UnboundedReceiverStream;
use rayon::iter::ParallelIterator;

/// Enum representing possible errors during the hashing process.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum ProgressHashingError {
    ErrHashingFile,
    ErrCollectingFiles,
}

/// Struct representing the current file update status.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct CurrentFileUpdate {
    pub current_file: String,
    pub total_hashed_files: usize,
}

/// Enum representing the work status of the hashing process.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum WorkStatus {
    Started(usize),
    Progress(CurrentFileUpdate),
    Result(HashMap<PathBuf, String>),
    Error(ProgressHashingError),
}

/// Collects all files in the given directory.
///
/// # Arguments
///
/// * `dir` - A reference to the path of the directory to collect files from.
///
/// # Returns
///
/// A `Result` containing a vector of `PathBuf` if successful, or a `walkdir::Error` if an error occurs.
fn collect_files_in_dir(dir: &Path) -> Result<Vec<PathBuf>, walkdir::Error> {
    let mut files = Vec::new();
    for entry in WalkDir::new(dir) {
        let entry = entry?;
        if entry.file_type().is_file() {
            files.push(entry.path().to_path_buf());
        }
    }
    Ok(files)
}

/// Calculates the BLAKE3 hash of the file at the given path.
///
/// # Arguments
///
/// * `path` - A reference to the path of the file to hash.
///
/// # Returns
///
/// An `io::Result` containing the hash as a `String` if successful, or an `io::Error` if an error occurs.
fn calculate_hash_with_blake3(path: &Path) -> io::Result<String> {
    let mut hasher = blake3::Hasher::new();
    let mut file = std::fs::File::open(path)?;
    std::io::copy(&mut file, &mut hasher)?;
    Ok(hasher.finalize().to_hex().to_string())
}

/// Asynchronously hashes files in the given directory, providing progress updates.
///
/// # Arguments
///
/// * `file_path` - A reference to the path of the directory to hash files from.
///
/// # Returns
///
/// A stream of `WorkStatus` items representing the progress and result of the hashing process.
pub async fn progressed_hashing(file_path: &Path) -> impl Stream<Item = WorkStatus> {
    let (tx, rx) = mpsc::unbounded_channel();

    let file_paths = match collect_files_in_dir(file_path) {
        Ok(paths) => {
            tx.send(WorkStatus::Started(paths.len())).unwrap();
            paths
        }
        Err(_err) => {
            eprintln!("Error collecting files: {:?}", _err);
            tx.send(WorkStatus::Error(ProgressHashingError::ErrCollectingFiles)).unwrap();
            return UnboundedReceiverStream::new(rx);
        }
    };

    let files_hashed_counter = Arc::new(AtomicUsize::new(0));

    tokio::task::spawn_blocking(move || {
        let files_hashed_counter = Arc::clone(&files_hashed_counter);

        let file_hashes: HashMap<PathBuf, String> = file_paths.par_iter().map(|path_buf: &PathBuf| {
            match calculate_hash_with_blake3(path_buf) {
                Ok(hash) => {
                    let total_hashed_files = files_hashed_counter.fetch_add(1, Ordering::SeqCst);

                    tx.send(WorkStatus::Progress(CurrentFileUpdate {
                        current_file: path_buf.display().to_string(),
                        total_hashed_files,
                    })).unwrap();

                    (path_buf.clone(), hash)
                },
                Err(_err) => {
                    eprintln!("Error hashing file: {:?}", _err);
                    tx.send(WorkStatus::Error(ProgressHashingError::ErrHashingFile)).unwrap();
                    (path_buf.clone(), "".to_string())
                }
            }

        }).collect();

        tx.send(WorkStatus::Result(file_hashes)).unwrap();
    });

    UnboundedReceiverStream::new(rx)
}