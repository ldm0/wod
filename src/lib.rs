use std::{
    fs::{self, File},
    hash::{BuildHasher, BuildHasherDefault, Hasher},
    io::{self, BufReader, Cursor},
    path::Path,
};

struct HashWriter<T: Hasher>(T);

impl<T: Hasher> io::Write for HashWriter<T> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.0.write(buf);
        Ok(buf.len())
    }

    fn write_all(&mut self, buf: &[u8]) -> io::Result<()> {
        self.write(buf).map(|_| ())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

pub fn write_on_file_diff<H: Hasher + Default>(
    from: impl AsRef<Path>,
    to: impl AsRef<Path>,
) -> io::Result<()> {
    let from = from.as_ref();
    let to = to.as_ref();
    let build_hasher = BuildHasherDefault::<H>::default();
    let from_hash = {
        let mut from_hash = HashWriter(build_hasher.build_hasher());
        io::copy(&mut BufReader::new(File::open(from)?), &mut from_hash)?;
        from_hash.0.finish()
    };
    let to_hash = (|| -> Result<_, io::Error> {
        let mut to_hash = HashWriter(build_hasher.build_hasher());
        io::copy(&mut BufReader::new(File::open(to)?), &mut to_hash)?;
        Ok(to_hash.0.finish())
    })();
    if to_hash.ok() != Some(from_hash) {
        fs::copy(from, to)?;
    }
    Ok(())
}

pub fn write_on_bytes_diff<H: Hasher + Default>(
    from: &[u8],
    to: impl AsRef<Path>,
) -> io::Result<()> {
    let to = to.as_ref();
    let build_hasher = BuildHasherDefault::<H>::default();
    let from_hash = {
        let mut from_hash = HashWriter(build_hasher.build_hasher());
        io::copy(&mut Cursor::new(from), &mut from_hash)?;
        from_hash.0.finish()
    };
    let to_hash = (|| -> Result<_, io::Error> {
        let mut to_hash = HashWriter(build_hasher.build_hasher());
        io::copy(&mut BufReader::new(File::open(to)?), &mut to_hash)?;
        Ok(to_hash.0.finish())
    })();
    if to_hash.ok() != Some(from_hash) {
        io::copy(&mut Cursor::new(from), &mut File::create(to)?)?;
    }
    Ok(())
}

pub fn write_on_dir_diff<H: Hasher + Default>(
    from: impl AsRef<Path>,
    to: impl AsRef<Path>,
) -> io::Result<()> {
    let from = from.as_ref();
    let to = to.as_ref();
    if !to.exists() {
        fs::create_dir_all(to)?;
    }

    for entry in fs::read_dir(from)? {
        let entry = entry?;
        let from_path = entry.path();
        let to_path = to.join(entry.file_name());

        if from_path.is_dir() {
            write_on_dir_diff::<H>(&from_path, &to_path)?;
        } else {
            if to_path.exists() {
                write_on_file_diff::<H>(&from_path, &to_path)?;
            } else {
                fs::copy(&from_path, &to_path)?;
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustc_hash::FxHasher;
    use std::fs;
    use std::io::Write;
    use tempfile::tempdir;
    use tempfile::NamedTempFile;

    #[test]
    fn test_dir_diff() -> io::Result<()> {
        let from_dir = tempdir()?;
        let to_dir = tempdir()?;

        // Create a file in the source directory
        let from_file_path = from_dir.path().join("a.txt");
        let mut from_file = File::create(&from_file_path)?;
        write!(from_file, "hello")?;

        // Create a file with the same content in the destination directory
        let to_file_path = to_dir.path().join("a.txt");
        let mut to_file = File::create(&to_file_path)?;
        write!(to_file, "hello")?;

        // Create a file with different content in the destination directory
        let to_file_path_b = to_dir.path().join("b.txt");
        let mut to_file_b = File::create(&to_file_path_b)?;
        write!(to_file_b, "world")?;

        // Create a corresponding file in the source directory
        let from_file_path_b = from_dir.path().join("b.txt");
        let mut from_file_b = File::create(&from_file_path_b)?;
        write!(from_file_b, "rust")?;

        // Create a subdirectory and a file in it
        let from_subdir = from_dir.path().join("sub");
        fs::create_dir(&from_subdir)?;
        let from_file_path_c = from_subdir.join("c.txt");
        let mut from_file_c = File::create(&from_file_path_c)?;
        write!(from_file_c, "subdir file")?;

        let original_meta = fs::metadata(&to_file_path)?;

        write_on_dir_diff::<FxHasher>(from_dir.path(), to_dir.path())?;

        // a.txt should not be modified
        let new_meta = fs::metadata(&to_file_path)?;
        assert_eq!(original_meta.modified()?, new_meta.modified()?);

        // b.txt should be modified
        let b_content = fs::read_to_string(&to_file_path_b)?;
        assert_eq!(b_content, "rust");

        // c.txt should be created
        let c_content = fs::read_to_string(to_dir.path().join("sub/c.txt"))?;
        assert_eq!(c_content, "subdir file");

        Ok(())
    }

    #[test]
    fn test_dir_diff_to_nonexistent() -> io::Result<()> {
        let from_dir = tempdir()?;
        let to_dir = tempdir()?;
        let to_path = to_dir.path().join("nonexistent");

        let from_file_path = from_dir.path().join("a.txt");
        let mut from_file = File::create(&from_file_path)?;
        write!(from_file, "hello")?;

        write_on_dir_diff::<FxHasher>(from_dir.path(), &to_path)?;

        assert!(to_path.exists());
        assert!(to_path.is_dir());
        let to_file_path = to_path.join("a.txt");
        assert!(to_file_path.exists());
        let content = fs::read_to_string(to_file_path)?;
        assert_eq!(content, "hello");

        Ok(())
    }

    #[test]
    fn test_dir_diff_from_empty() -> io::Result<()> {
        let from_dir = tempdir()?;
        let to_dir = tempdir()?;

        let to_file_path = to_dir.path().join("a.txt");
        let mut to_file = File::create(&to_file_path)?;
        write!(to_file, "hello")?;

        let original_meta = fs::metadata(&to_file_path)?;

        write_on_dir_diff::<FxHasher>(from_dir.path(), to_dir.path())?;

        let new_meta = fs::metadata(&to_file_path)?;
        assert_eq!(original_meta.modified()?, new_meta.modified()?);

        Ok(())
    }

    #[test]
    fn test_dir_diff_to_has_extra_files() -> io::Result<()> {
        let from_dir = tempdir()?;
        let to_dir = tempdir()?;

        let from_file_path = from_dir.path().join("a.txt");
        let mut from_file = File::create(&from_file_path)?;
        write!(from_file, "hello")?;

        let to_extra_file_path = to_dir.path().join("extra.txt");
        let mut to_extra_file = File::create(&to_extra_file_path)?;
        write!(to_extra_file, "extra")?;

        write_on_dir_diff::<FxHasher>(from_dir.path(), to_dir.path())?;

        assert!(to_extra_file_path.exists());
        let content = fs::read_to_string(&to_extra_file_path)?;
        assert_eq!(content, "extra");

        Ok(())
    }

    #[test]
    fn test_dir_diff_deeply_nested() -> io::Result<()> {
        let from_dir = tempdir()?;
        let to_dir = tempdir()?;

        let from_sub_dir = from_dir.path().join("a/b/c");
        fs::create_dir_all(&from_sub_dir)?;
        let from_file_path = from_sub_dir.join("d.txt");
        let mut from_file = File::create(&from_file_path)?;
        write!(from_file, "deep")?;

        write_on_dir_diff::<FxHasher>(from_dir.path(), to_dir.path())?;

        let to_file_path = to_dir.path().join("a/b/c/d.txt");
        assert!(to_file_path.exists());
        let content = fs::read_to_string(to_file_path)?;
        assert_eq!(content, "deep");

        Ok(())
    }

    #[test]
    fn test_file_diff_dest_nonexistent() -> io::Result<()> {
        let mut from_file = NamedTempFile::new()?;
        write!(from_file, "hello")?;

        let to_path = NamedTempFile::new()?.into_temp_path();
        fs::remove_file(&to_path)?;

        write_on_file_diff::<FxHasher>(from_file.path(), &to_path)?;

        assert!(to_path.exists());
        let content = fs::read_to_string(&to_path)?;
        assert_eq!(content, "hello");

        Ok(())
    }

    #[test]
    fn test_file_diff_files_are_same() -> io::Result<()> {
        let mut from_file = NamedTempFile::new()?;
        write!(from_file, "hello")?;

        let mut to_file = NamedTempFile::new()?;
        write!(to_file, "hello")?;
        let to_path = to_file.path();
        let original_meta = fs::metadata(to_path)?;

        write_on_file_diff::<FxHasher>(from_file.path(), to_path)?;

        let new_meta = fs::metadata(to_path)?;
        assert_eq!(original_meta.modified()?, new_meta.modified()?);

        Ok(())
    }

    #[test]
    fn test_file_diff_files_are_different() -> io::Result<()> {
        let mut from_file = NamedTempFile::new()?;
        write!(from_file, "hello")?;

        let mut to_file = NamedTempFile::new()?;
        write!(to_file, "world")?;
        let to_path = to_file.path();

        write_on_file_diff::<FxHasher>(from_file.path(), to_path)?;

        let to_content = fs::read_to_string(to_path)?;
        assert_eq!(to_content, "hello");

        Ok(())
    }

    #[test]
    fn test_file_diff_source_nonexistent() -> io::Result<()> {
        let from_path = Path::new("nonexistent_source_file");
        let to_path = NamedTempFile::new()?.into_temp_path();

        let result = write_on_file_diff::<FxHasher>(from_path, &to_path);

        assert!(result.is_err());
        assert_eq!(result.unwrap_err().kind(), io::ErrorKind::NotFound);

        Ok(())
    }

    #[test]
    fn test_bytes_diff_dest_nonexistent() -> io::Result<()> {
        let from_bytes = b"hello";
        let to_path = NamedTempFile::new()?.into_temp_path();
        fs::remove_file(&to_path)?;

        write_on_bytes_diff::<FxHasher>(from_bytes, &to_path)?;

        assert!(to_path.exists());
        let content = fs::read_to_string(&to_path)?;
        assert_eq!(content, "hello");

        Ok(())
    }

    #[test]
    fn test_bytes_diff_bytes_are_same() -> io::Result<()> {
        let from_bytes = b"hello";
        let mut to_file = NamedTempFile::new()?;
        write!(to_file, "hello")?;
        let to_path = to_file.path();
        let original_meta = fs::metadata(to_path)?;

        write_on_bytes_diff::<FxHasher>(from_bytes, to_path)?;

        let new_meta = fs::metadata(to_path)?;
        assert_eq!(original_meta.modified()?, new_meta.modified()?);

        Ok(())
    }

    #[test]
    fn test_bytes_diff_bytes_are_different() -> io::Result<()> {
        let from_bytes = b"hello";
        let mut to_file = NamedTempFile::new()?;
        write!(to_file, "world")?;
        let to_path = to_file.path();

        write_on_bytes_diff::<FxHasher>(from_bytes, to_path)?;

        let to_content = fs::read_to_string(to_path)?;
        assert_eq!(to_content, "hello");

        Ok(())
    }

    #[test]
    fn test_file_diff_empty_source() -> io::Result<()> {
        let from_file = NamedTempFile::new()?;

        let mut to_file = NamedTempFile::new()?;
        write!(to_file, "world")?;
        let to_path = to_file.path();

        write_on_file_diff::<FxHasher>(from_file.path(), to_path)?;

        let to_content = fs::read_to_string(to_path)?;
        assert_eq!(to_content, "");

        Ok(())
    }

    #[test]
    fn test_file_diff_empty_dest() -> io::Result<()> {
        let mut from_file = NamedTempFile::new()?;
        write!(from_file, "hello")?;

        let to_file = NamedTempFile::new()?;
        let to_path = to_file.path();

        write_on_file_diff::<FxHasher>(from_file.path(), to_path)?;

        let to_content = fs::read_to_string(to_path)?;
        assert_eq!(to_content, "hello");

        Ok(())
    }

    #[test]
    fn test_file_diff_both_empty() -> io::Result<()> {
        let from_file = NamedTempFile::new()?;
        let to_file = NamedTempFile::new()?;
        let to_path = to_file.path();
        let original_meta = fs::metadata(to_path)?;

        write_on_file_diff::<FxHasher>(from_file.path(), to_path)?;

        let new_meta = fs::metadata(to_path)?;
        assert_eq!(original_meta.modified()?, new_meta.modified()?);

        Ok(())
    }

    #[test]
    fn test_bytes_diff_empty_source() -> io::Result<()> {
        let from_bytes = b"";
        let mut to_file = NamedTempFile::new()?;
        write!(to_file, "world")?;
        let to_path = to_file.path();

        write_on_bytes_diff::<FxHasher>(from_bytes, to_path)?;

        let to_content = fs::read_to_string(to_path)?;
        assert_eq!(to_content, "");

        Ok(())
    }

    #[test]
    fn test_bytes_diff_empty_dest() -> io::Result<()> {
        let from_bytes = b"hello";
        let to_file = NamedTempFile::new()?;
        let to_path = to_file.path();

        write_on_bytes_diff::<FxHasher>(from_bytes, to_path)?;

        let to_content = fs::read_to_string(to_path)?;
        assert_eq!(to_content, "hello");

        Ok(())
    }

    #[test]
    fn test_bytes_diff_both_empty() -> io::Result<()> {
        let from_bytes = b"";
        let to_file = NamedTempFile::new()?;
        let to_path = to_file.path();
        let original_meta = fs::metadata(to_path)?;

        write_on_bytes_diff::<FxHasher>(from_bytes, to_path)?;

        let new_meta = fs::metadata(to_path)?;
        assert_eq!(original_meta.modified()?, new_meta.modified()?);

        Ok(())
    }
}