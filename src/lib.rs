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

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum ProgressHashingError {
    PermissionDenied,
    NotFound
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct CurrentFileUpdate {
    pub current_file: String,
    pub total_hashed_files: usize,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum WorkStatus {
    Started(usize),
    Progress(CurrentFileUpdate),
    Result(HashMap<PathBuf, String>),
    Error(ProgressHashingError)
}

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

fn calculate_hash_with_blake3(path: &Path) -> io::Result<String> {
    let mut hasher = blake3::Hasher::new();
    let mut file = std::fs::File::open(path)?;
    std::io::copy(&mut file, &mut hasher)?;
    Ok(hasher.finalize().to_hex().to_string())
}

pub async fn progressed_hashing(file_path: &PathBuf) -> impl Stream<Item = WorkStatus> {
    let (tx, rx) = mpsc::unbounded_channel();

    let file_paths = match collect_files_in_dir(file_path) {
        Ok(paths) => {
            tx.send(WorkStatus::Started(paths.len())).unwrap();
            paths
        }
        Err(_err) => {
            eprintln!("Error collecting files: {:?}", _err);
            tx.send(WorkStatus::Error(ProgressHashingError::PermissionDenied)).unwrap();
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
                    panic!("Error hashing file: {:?}", _err);
                }
            }

        }).collect();

        tx.send(WorkStatus::Result(file_hashes)).unwrap();
    });

    UnboundedReceiverStream::new(rx)
}