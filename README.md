# wod

`wod` stands for "write on diff". It's a small Rust library that provides utilities to write files or directories only when their content has changed.

This can be useful to avoid unnecessary writes and updates, for example, in build systems or data synchronization tools.

## Features

- `write_on_bytes_diff`: Writes a slice of bytes to a file path if the content is different.
- `write_on_file_diff`: Copies a file to a destination path if the content is different.
- `write_on_dir_diff`: Recursively copies a directory to a destination path, only writing files that have different content.

## Usage

Add this to your `Cargo.toml`:

```toml
[dependencies]
wod = "0.1.0"
```

### Example: `write_on_bytes_diff`

```rust
use wod::write_on_bytes_diff;
use rustc_hash::FxHasher;
use std::fs;
use std::io;

fn main() -> io::Result<()> {
    let path = "temp.txt";
    let data1 = b"hello world";
    let data2 = b"hello rust";

    // First write
    write_on_bytes_diff::<FxHasher>(path, data1)?;
    assert_eq!(fs::read_to_string(path)?, "hello world");

    // Write again with same content, file is not modified
    let metadata1 = fs::metadata(path)?;
    write_on_bytes_diff::<FxHasher>(path, data1)?;
    let metadata2 = fs::metadata(path)?;
    assert_eq!(metadata1.modified()?, metadata2.modified()?);

    // Write with different content, file is modified
    write_on_bytes_diff::<FxHasher>(path, data2)?;
    assert_eq!(fs::read_to_string(path)?, "hello rust");

    fs::remove_file(path)?;
    Ok(())
}
```

### Example: `write_on_dir_diff`

```rust
use wod::write_on_dir_diff;
use rustc_hash::FxHasher;
use std::fs::{self, File};
use std::io::{self, Write};
use tempfile::tempdir;

fn main() -> io::Result<()> {
    // Create a source directory with a file
    let from_dir = tempdir().unwrap();
    let from_file_path = from_dir.path().join("a.txt");
    let mut from_file = File::create(&from_file_path).unwrap();
    write!(from_file, "hello").unwrap();

    // Create a destination directory
    let to_dir = tempdir().unwrap();

    // Copy directory
    write_on_dir_diff::<FxHasher>(from_dir.path(), to_dir.path()).unwrap();

    // Check if the file is copied
    let to_file_path = to_dir.path().join("a.txt");
    assert!(to_file_path.exists());
    assert_eq!(fs::read_to_string(to_file_path).unwrap(), "hello");

    Ok(())
}
```

## Generic Hasher

You can use any `std::hash::Hasher` implementation. `rustc_hash::FxHasher` is a fast non-cryptographic hasher and a good default choice.

```rust
use wod::write_on_bytes_diff;
use rustc_hash::FxHasher; // or any other hasher

// ...
write_on_bytes_diff::<FxHasher>(path, data).unwrap();
```