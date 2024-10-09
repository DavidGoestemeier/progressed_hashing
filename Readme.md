## Progressed Hashing

A very small lib which provides a stream based hashing mechanism for a specified directory and all its content.
Returns a stream of WorkStatus which can be used to get updates on the progress of the hashing process.
When the hashing is done, it returns a Result with a HashMap containing the file paths and their respective hashes.

ONLY AND FINAL DISCLAIMER:
Bad code ahead.
I started learning rust 6 days ago.

### Usage
Dunno if dis works, should though
```rust
use ProgressedHasher::progressed_hashing;

#[tokio::main]
async fn main() {
    let file_path = PathBuf::from("./your_directory"); // Replace with your directory

    let mut stream = progressed_hashing(&file_path);

    while let Some(work_status) = stream.next().await {
        match work_status {
            WorkStatus::Started(total_files) => {
                println!("Started hashing {} files", total_files);
            }
            WorkStatus::Progress(update) => {
                println!("Hashing file: {} ({}/{})", update.current_file, update.total_hashed_files, total_files);
            }
            WorkStatus::Result(file_hashes) => {
                println!("Finished hashing! Results:");
                for (path, hash) in file_hashes {
                    println!("{}: {}", path.display(), hash);
                }
            }
            WorkStatus::Error(err) => {
                eprintln!("Error: {:?}", err);
            }
        }
    }
}
```
