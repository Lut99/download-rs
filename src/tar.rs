//  TAR.rs
//    by Lut99
//
//  Created:
//    11 Mar 2024, 15:53:35
//  Last edited:
//    11 Mar 2024, 16:51:58
//  Auto updated?
//    Yes
//
//  Description:
//!   Defines functions for archiving/unarchiving tarballs.
//

use std::ffi::{OsStr, OsString};
use std::fmt::{Display, Formatter, Result as FResult};
use std::path::{Path, PathBuf};
use std::{error, fs, io};

use flate2::read::GzDecoder;
use flate2::write::GzEncoder;
use flate2::Compression;
use log::debug;
use tar::{Archive, Builder, Entries, Entry};
#[cfg(feature = "async-tokio")]
use ::{
    async_compression::tokio::bufread::GzipDecoder as AsyncGzipDecoder,
    async_compression::tokio::write::GzipEncoder as AsyncGzipEncoder,
    tokio::io::AsyncWriteExt as _,
    tokio::{fs as tfs, io as tio},
    tokio_stream::StreamExt as _,
    tokio_tar::{Archive as AsyncArchive, Builder as AsyncBuilder, Entries as AsyncEntries, Entry as AsyncEntry},
};


/***** MACROS *****/
/// Mirrors [`log`]'s [`debug!`]-macro, but only when the `log`-feature it given.
#[cfg(feature = "log")]
macro_rules! debug {
    ($($t:tt)*) => {
        ::log::debug!($($t)*)
    };
}
#[cfg(not(feature = "log"))]
macro_rules! debug {
    ($($t:tt)*) => {};
}





/***** ERRORS *****/
/// Defines the errors tha may occur when dealing with the filesystem operations.
#[derive(Debug)]
pub enum Error {
    // Archive errors
    /// Failed to read an entry in the to-be-archived directory.
    SourceDirEntryRead { path: PathBuf, entry: usize, err: std::io::Error },
    /// Failed to read the to-be-archived directory.
    SourceDirRead { path: PathBuf, err: std::io::Error },
    /// The given source entry is neither a file nor a directory.
    SourceNotAFileOrDir { path: PathBuf },
    /// The given source file did not exist.
    SourceNotFound { path: PathBuf },
    /// Failed to append a file to the output tar file.
    TargetTarAppend { source: PathBuf, tarball: PathBuf, err: std::io::Error },
    /// Failed to create the output tar file.
    TargetTarCreate { tarball: PathBuf, err: std::io::Error },
    /// Failed to finish up the tarball.
    TargetTarFinish { tarball: PathBuf, err: std::io::Error },
    /// Failed to flush the encoder writing to the tar file.
    TargetTarFlush { tarball: PathBuf, err: std::io::Error },

    // Unarchive errors
    /// Failed to read the available entries in the given source tarball.
    SourceTarEntries { tarball: PathBuf, err: std::io::Error },
    /// Failed to read the one of the availablke entries in the given source tarball.
    SourceTarEntry { tarball: PathBuf, entry: usize, err: std::io::Error },
    /// Did not extract an entry because its path would have escaped the target directory.
    SourceTarEntryEscaped { tarball: PathBuf, entry: PathBuf },
    /// Failed to read the relative path of an entry in the given source tarball.
    SourceTarEntryPath { tarball: PathBuf, entry: usize, err: std::io::Error },
    /// Failed to unpack an entry from the given source tarball to the given location.
    SourceTarEntryUnpack { tarball: PathBuf, entry: PathBuf, target: PathBuf, err: std::io::Error },
    /// Failed to open the source tarball.
    SourceTarOpen { tarball: PathBuf, err: std::io::Error },
    /// Failed to create the target directory.
    TargetDirCreate { path: PathBuf, err: std::io::Error },
    /// The target path already exists.
    TargetExists { path: PathBuf },
}
impl Display for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> FResult {
        use Error::*;
        match self {
            SourceDirEntryRead { path, entry, .. } => write!(f, "Failed to read entry {} in source directory '{}'", entry, path.display()),
            SourceDirRead { path, .. } => write!(f, "Failed to read source directory '{}'", path.display()),
            SourceNotAFileOrDir { path } => write!(f, "Source '{}' is not a file or a directory", path.display()),
            SourceNotFound { path } => write!(f, "Source '{}' not found", path.display()),
            TargetTarAppend { source, tarball, .. } => write!(f, "Failed to append file '{}' to tarball '{}'", source.display(), tarball.display()),
            TargetTarCreate { tarball, .. } => write!(f, "Failed to create tarball '{}'", tarball.display()),
            TargetTarFinish { tarball, .. } => write!(f, "Failed to finish up tarball '{}'", tarball.display()),
            TargetTarFlush { tarball, .. } => write!(f, "Failed to finish tarball '{}'", tarball.display()),

            SourceTarEntries { tarball, .. } => write!(f, "Failed to read entries in tarball '{}'", tarball.display()),
            SourceTarEntry { tarball, entry, .. } => write!(f, "Failed to read entry {} in tarball '{}'", entry, tarball.display()),
            SourceTarEntryEscaped { tarball, entry } => {
                write!(f, "Entry '{}' in tarball '{}' would have escaped target directory", entry.display(), tarball.display())
            },
            SourceTarEntryPath { tarball, entry, .. } => write!(f, "Failed to get path of entry {} in tarball '{}'", entry, tarball.display()),
            SourceTarEntryUnpack { tarball, entry, target, .. } => {
                write!(f, "Failed to unpack entry '{}' in tarball '{}' to '{}'", entry.display(), tarball.display(), target.display())
            },
            SourceTarOpen { tarball, .. } => write!(f, "Failed to open source tarball '{}'", tarball.display()),
            TargetDirCreate { path, .. } => write!(f, "Failed to create target directory '{}'", path.display()),
            TargetExists { path } => write!(f, "Target path '{}' already exists", path.display()),
        }
    }
}
impl error::Error for Error {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        use Error::*;
        match self {
            SourceDirEntryRead { err, .. } => Some(err),
            SourceDirRead { err, .. } => Some(err),
            SourceNotAFileOrDir { .. } => None,
            SourceNotFound { .. } => None,
            TargetTarAppend { err, .. } => Some(err),
            TargetTarCreate { err, .. } => Some(err),
            TargetTarFinish { err, .. } => Some(err),
            TargetTarFlush { err, .. } => Some(err),

            SourceTarEntries { err, .. } => Some(err),
            SourceTarEntry { err, .. } => Some(err),
            SourceTarEntryEscaped { .. } => None,
            SourceTarEntryPath { err, .. } => Some(err),
            SourceTarEntryUnpack { err, .. } => Some(err),
            SourceTarOpen { err, .. } => Some(err),
            TargetDirCreate { err, .. } => Some(err),
            TargetExists { .. } => None,
        }
    }
}





/***** LIBRARY *****/
/// Archives the given file or directory as a `.tar.gz` file.
///
/// If you enabled the `async-tokio` feature, also check the [`archive_async()`]-function for async contexts.
///
/// # Arguments
/// - `source`: The source file or directory to archive.
/// - `tarball`: The target tarball file to archive to.
/// - `skip_root_dir`: If the `source` points to a directory, then this determines whether to trim it (true) or not (false) in the resulting tarfile
///   (i.e., the files in the root dir will be in the tar's root instead of the directory). Ignore otherwise.
///
/// # Errors
/// This function errors if we somehow encountered an error.
///
/// # Examples
/// ```rust
/// use download::tar::archive;
///
/// // Write a test directory
/// let tmp = std::env::temp_dir();
/// let dir = tmp.join("example");
/// # std::fs::remove_dir_all(&dir).unwrap();
/// std::fs::create_dir(&dir).unwrap();
/// std::fs::write(dir.join("file1.txt"), "Hello there!\n").unwrap();
/// std::fs::write(dir.join("file2.txt"), "General Kenobi...\n").unwrap();
/// std::fs::write(dir.join("file3.txt"), "...you are a bold one\n").unwrap();
///
/// // We can archive them!
/// let tar = tmp.join("example.tar.gz");
/// # std::fs::remove_file(&tar).unwrap();
/// archive(&dir, &tar, false).unwrap();
///
/// assert!(tar.is_file());
/// ```
pub fn archive(source: impl AsRef<Path>, tarball: impl AsRef<Path>, skip_root_dir: bool) -> Result<(), Error> {
    let source: &Path = source.as_ref();
    let tarball: &Path = tarball.as_ref();
    debug!("Archiving '{}' to '{}'...", source.display(), tarball.display());

    // Open the target file
    let handle: fs::File = match fs::File::create(tarball) {
        Ok(handle) => handle,
        Err(err) => {
            return Err(Error::TargetTarCreate { tarball: tarball.into(), err });
        },
    };

    // Create the encoder & tarfile around this file
    let enc: GzEncoder<_> = GzEncoder::new(handle, Compression::best());
    let mut tar: Builder<GzEncoder<_>> = Builder::new(enc);

    // Now add the source recursively
    let mut is_root_dir: bool = true;
    let mut todo: Vec<(PathBuf, OsString)> = vec![(source.into(), source.file_name().map(|f| f.into()).unwrap_or_else(|| OsString::from(".")))];
    while let Some((path, name)) = todo.pop() {
        // Switch on the file type
        if path.is_file() {
            debug!("Adding file '{}' as '{}/{}'...", path.display(), tarball.display(), name.to_string_lossy());

            // Compress as a file
            if let Err(err) = tar.append_path_with_name(&path, name) {
                return Err(Error::TargetTarAppend { source: path, tarball: tarball.into(), err });
            }
        } else if path.is_dir() {
            // Recurse to add the files
            let entries: fs::ReadDir = match fs::read_dir(&path) {
                Ok(entries) => entries,
                Err(err) => {
                    return Err(Error::SourceDirRead { path, err });
                },
            };
            for (i, entry) in entries.enumerate() {
                // Fetch the next entry
                let entry: fs::DirEntry = match entry {
                    Ok(entry) => entry,
                    Err(err) => {
                        return Err(Error::SourceDirEntryRead { path, entry: i, err });
                    },
                };

                // Compute the tar-side path of this file
                let name: &OsStr = if skip_root_dir && is_root_dir { OsStr::new("") } else { &name };

                // Add its path
                todo.push((entry.path(), PathBuf::from(name).join(entry.file_name()).into()));
            }
            is_root_dir = false;
        } else if !path.exists() {
            return Err(Error::SourceNotFound { path });
        } else {
            return Err(Error::SourceNotAFileOrDir { path });
        }
    }

    // Finish writing the archive
    debug!("Finishing tarball...");
    match tar.finish() {
        Ok(_) => Ok(()),
        Err(err) => Err(Error::TargetTarFinish { tarball: tarball.into(), err }),
    }
}

/// Archives the given file or directory as a `.tar.gz` file.
///
/// This variation is built using [`tokio`] versions of the normal operations, and is as such only available on the `async-tokio` feature.
///
/// # Arguments
/// - `source`: The source file or directory to archive.
/// - `tarball`: The target tarball file to archive to.
/// - `skip_root_dir`: If the `source` points to a directory, then this determines whether to trim it (true) or not (false) in the resulting tarfile
///   (i.e., the files in the root dir will be in the tar's root instead of the directory). Ignore otherwise.
///
/// # Errors
/// This function errors if we somehow encountered an error.
///
/// # Examples
/// ```rust
/// # tokio_test::block_on(async {
/// use download::tar::archive_async;
///
/// // Write a test directory
/// let tmp = std::env::temp_dir();
/// let dir = tmp.join("example");
/// # tokio::fs::remove_dir_all(&dir).await.unwrap();
/// tokio::fs::create_dir(&dir).await.unwrap();
/// tokio::fs::write(dir.join("file1.txt"), "Hello there!\n").await.unwrap();
/// tokio::fs::write(dir.join("file2.txt"), "General Kenobi...\n").await.unwrap();
/// tokio::fs::write(dir.join("file3.txt"), "...you are a bold one\n").await.unwrap();
///
/// // We can archive them!
/// let tar = tmp.join("example.tar.gz");
/// # tokio::fs::remove_file(&tar).await.unwrap();
/// archive_async(&dir, &tar, false).await.unwrap();
///
/// assert!(tar.is_file());
/// # });
/// ```
#[cfg(feature = "async-tokio")]
pub async fn archive_async(source: impl AsRef<Path>, tarball: impl AsRef<Path>, skip_root_dir: bool) -> Result<(), Error> {
    let source: &Path = source.as_ref();
    let tarball: &Path = tarball.as_ref();
    debug!("Archiving '{}' to '{}'...", source.display(), tarball.display());

    // Open the target file
    let handle: tfs::File = match tfs::File::create(tarball).await {
        Ok(handle) => handle,
        Err(err) => {
            return Err(Error::TargetTarCreate { tarball: tarball.into(), err });
        },
    };

    // Create the encoder & tarfile around this file
    let enc: AsyncGzipEncoder<_> = AsyncGzipEncoder::new(handle);
    let mut tar: AsyncBuilder<AsyncGzipEncoder<_>> = AsyncBuilder::new(enc);

    // Now add the source recursively
    let mut is_root_dir: bool = true;
    let mut todo: Vec<(PathBuf, OsString)> = vec![(source.into(), source.file_name().map(|f| f.into()).unwrap_or_else(|| OsString::from(".")))];
    while let Some((path, name)) = todo.pop() {
        // Switch on the file type
        if path.is_file() {
            debug!("Adding file '{}' as '{}/{}'...", path.display(), tarball.display(), name.to_string_lossy());

            // Compress as a file
            if let Err(err) = tar.append_path_with_name(&path, name).await {
                return Err(Error::TargetTarAppend { source: path, tarball: tarball.into(), err });
            }
        } else if path.is_dir() {
            // Recurse to add the files
            let mut entries: tfs::ReadDir = match tfs::read_dir(&path).await {
                Ok(entries) => entries,
                Err(err) => {
                    return Err(Error::SourceDirRead { path, err });
                },
            };
            let mut i: usize = 0;
            loop {
                // Fetch the next entry
                let entry: tfs::DirEntry = match entries.next_entry().await {
                    Ok(Some(entry)) => entry,
                    Ok(None) => {
                        break;
                    },
                    Err(err) => {
                        return Err(Error::SourceDirEntryRead { path, entry: i, err });
                    },
                };
                i += 1;

                // Compute the tar-side path of this file
                let name: &OsStr = if skip_root_dir && is_root_dir { OsStr::new("") } else { &name };

                // Add its path
                todo.push((entry.path(), PathBuf::from(name).join(entry.file_name()).into()));
            }
            is_root_dir = false;
        } else if !path.exists() {
            return Err(Error::SourceNotFound { path });
        } else {
            return Err(Error::SourceNotAFileOrDir { path });
        }
    }

    // Finish writing the archive
    debug!("Finishing tarball...");
    match tar.into_inner().await {
        Ok(mut enc) => {
            // Flush the encoder before we quit
            if let Err(err) = enc.shutdown().await {
                return Err(Error::TargetTarFlush { tarball: tarball.into(), err });
            };
            Ok(())
        },
        Err(err) => Err(Error::TargetTarFinish { tarball: tarball.into(), err }),
    }
}



/// Unarchives the given `.tar.gz` file to the given location.
///
/// If you enabled the `async-tokio` feature, also check the [`unarchive_async()`]-function for async contexts.
///
/// # Arguments
/// - `tarball`: The source tarball file to extract from.
/// - `target`: The target directory to write to. Note that we will throw all sorts of nasty errors if it already exists somehow.
///
/// # Errors
/// This function errors if we failed to read or write anything or if some directories do or do not exist.
///
/// # Examples
/// ```rust
/// use download::tar::unarchive;
///
/// // Create an archive (see 'archive()' example)
/// # let tmp = std::env::temp_dir();
/// # let dir = tmp.join("example");
/// # std::fs::remove_dir_all(&dir).unwrap();
/// # std::fs::create_dir(&dir).unwrap();
/// # std::fs::write(dir.join("file1.txt"), "Hello there!\n").unwrap();
/// # std::fs::write(dir.join("file2.txt"), "General Kenobi...\n").unwrap();
/// # std::fs::write(dir.join("file3.txt"), "...you are a bold one\n").unwrap();
/// # let tar = tmp.join("example.tar.gz");
/// # std::fs::remove_file(&tar).unwrap();
/// # download::tar::archive(&dir, &tar, false).unwrap();
///
/// // Unarchive it to another directory!
/// let out = tmp.join("example2");
/// # std::fs::remove_dir_all(&out).unwrap();
/// unarchive(&tar, &out).unwrap();
///
/// // Now check the directory contains what we expect :)
/// let mut entries = std::fs::read_dir(&out).unwrap();
/// assert_eq!(entries.next().unwrap().unwrap().file_name().to_string_lossy(), "example");
///
/// let mut entries: Vec<_> = std::fs::read_dir(out.join("example"))
///     .unwrap()
///     .map(|e| e.unwrap().file_name().to_string_lossy().to_string())
///     .collect();
/// assert!(entries.contains(&"file1.txt".to_string()));
/// assert!(entries.contains(&"file2.txt".to_string()));
/// assert!(entries.contains(&"file3.txt".to_string()));
/// ```
pub fn unarchive(tarball: impl AsRef<Path>, target: impl AsRef<Path>) -> Result<(), Error> {
    let tarball: &Path = tarball.as_ref();
    let target: &Path = target.as_ref();
    debug!("Extracting '{}' to '{}'...", tarball.display(), target.display());

    // Whine if the target already exists
    if target.exists() {
        return Err(Error::TargetExists { path: target.into() });
    }
    if let Err(err) = fs::create_dir(target) {
        return Err(Error::TargetDirCreate { path: target.into(), err });
    }

    // Open the source tarfile
    let handle: fs::File = match fs::File::open(tarball) {
        Ok(handle) => handle,
        Err(err) => {
            return Err(Error::SourceTarOpen { tarball: tarball.into(), err });
        },
    };

    // Create the decoder & tarfile around this file
    let dec: GzDecoder<_> = GzDecoder::new(io::BufReader::new(handle));
    let mut tar: Archive<GzDecoder<_>> = Archive::new(dec);
    let entries: Entries<GzDecoder<_>> = match tar.entries() {
        Ok(entries) => entries,
        Err(err) => {
            return Err(Error::SourceTarEntries { tarball: tarball.into(), err });
        },
    };

    // Iterate over all of the entries
    for (i, entry) in entries.enumerate() {
        // Unwrap the entry
        let mut entry: Entry<GzDecoder<_>> = match entry {
            Ok(entry) => entry,
            Err(err) => {
                return Err(Error::SourceTarEntry { tarball: tarball.into(), entry: i, err });
            },
        };

        // Attempt to extract the entry
        let entry_path: PathBuf = match entry.path() {
            Ok(entry_path) => entry_path.into(),
            Err(err) => {
                return Err(Error::SourceTarEntryPath { tarball: tarball.into(), entry: i, err });
            },
        };

        // Unpack the thing
        let target_path: PathBuf = target.join(&entry_path);
        debug!("Extracting '{}/{}' to '{}'...", tarball.display(), entry_path.display(), target_path.display());
        match entry.unpack_in(&target) {
            Ok(true) => {},
            Ok(false) => {
                return Err(Error::SourceTarEntryEscaped { tarball: tarball.into(), entry: entry_path });
            },
            Err(err) => {
                return Err(Error::SourceTarEntryUnpack { tarball: tarball.into(), entry: entry_path, target: target_path, err });
            },
        }

        // Done, go to next entry
    }

    // Done
    Ok(())
}

/// Unarchives the given `.tar.gz` file to the given location.
///
/// This variation is built using [`tokio`] versions of the normal operations, and is as such only available on the `async-tokio` feature.
///
/// # Arguments
/// - `tarball`: The source tarball file to extract from.
/// - `target`: The target directory to write to. Note that we will throw all sorts of nasty errors if it already exists somehow.
///
/// # Errors
/// This function errors if we failed to read or write anything or if some directories do or do not exist.
///
/// # Examples
/// ```rust
/// # tokio_test::block_on(async {
/// use download::tar::unarchive_async;
///
/// // Create an archive (see 'archive()' example)
/// # let tmp = std::env::temp_dir();
/// # let dir = tmp.join("example");
/// # tokio::fs::remove_dir_all(&dir).await.unwrap();
/// # tokio::fs::create_dir(&dir).await.unwrap();
/// # tokio::fs::write(dir.join("file1.txt"), "Hello there!\n").await.unwrap();
/// # tokio::fs::write(dir.join("file2.txt"), "General Kenobi...\n").await.unwrap();
/// # tokio::fs::write(dir.join("file3.txt"), "...you are a bold one\n").await.unwrap();
/// # let tar = tmp.join("example.tar.gz");
/// # tokio::fs::remove_file(&tar).await.unwrap();
/// # download::tar::archive_async(&dir, &tar, false).await.unwrap();
///
/// // Unarchive it to another directory!
/// let out = tmp.join("example2");
/// # tokio::fs::remove_dir_all(&out).await.unwrap();
/// unarchive_async(&tar, &out).await.unwrap();
///
/// // Now check the directory contains what we expect :)
/// let mut entries = std::fs::read_dir(&out).unwrap();
/// assert_eq!(entries.next().unwrap().unwrap().file_name().to_string_lossy(), "example");
///
/// let mut entries: Vec<_> = std::fs::read_dir(out.join("example"))
///     .unwrap()
///     .map(|e| e.unwrap().file_name().to_string_lossy().to_string())
///     .collect();
/// assert!(entries.contains(&"file1.txt".to_string()));
/// assert!(entries.contains(&"file2.txt".to_string()));
/// assert!(entries.contains(&"file3.txt".to_string()));
/// # });
/// ```
#[cfg(feature = "async-tokio")]
pub async fn unarchive_async(tarball: impl AsRef<Path>, target: impl AsRef<Path>) -> Result<(), Error> {
    let tarball: &Path = tarball.as_ref();
    let target: &Path = target.as_ref();
    debug!("Extracting '{}' to '{}'...", tarball.display(), target.display());

    // Whine if the target already exists
    if target.exists() {
        return Err(Error::TargetExists { path: target.into() });
    }
    if let Err(err) = tfs::create_dir(target).await {
        return Err(Error::TargetDirCreate { path: target.into(), err });
    }

    // Open the source tarfile
    let handle: tfs::File = match tfs::File::open(tarball).await {
        Ok(handle) => handle,
        Err(err) => {
            return Err(Error::SourceTarOpen { tarball: tarball.into(), err });
        },
    };

    // Create the decoder & tarfile around this file
    let dec: AsyncGzipDecoder<_> = AsyncGzipDecoder::new(tio::BufReader::new(handle));
    let mut tar: AsyncArchive<AsyncGzipDecoder<_>> = AsyncArchive::new(dec);
    let mut entries: AsyncEntries<AsyncGzipDecoder<_>> = match tar.entries() {
        Ok(entries) => entries,
        Err(err) => {
            return Err(Error::SourceTarEntries { tarball: tarball.into(), err });
        },
    };

    // Iterate over all of the entries
    let mut i: usize = 0;
    while let Some(entry) = entries.next().await {
        // Unwrap the entry
        let mut entry: AsyncEntry<AsyncArchive<_>> = match entry {
            Ok(entry) => entry,
            Err(err) => {
                return Err(Error::SourceTarEntry { tarball: tarball.into(), entry: i, err });
            },
        };
        i += 1;

        // Attempt to extract the entry
        let entry_path: PathBuf = match entry.path() {
            Ok(entry_path) => entry_path.into(),
            Err(err) => {
                return Err(Error::SourceTarEntryPath { tarball: tarball.into(), entry: i, err });
            },
        };

        // Unpack the thing
        let target_path: PathBuf = target.join(&entry_path);
        debug!("Extracting '{}/{}' to '{}'...", tarball.display(), entry_path.display(), target_path.display());
        match entry.unpack_in(&target).await {
            Ok(true) => {},
            Ok(false) => {
                return Err(Error::SourceTarEntryEscaped { tarball: tarball.into(), entry: entry_path });
            },
            Err(err) => {
                return Err(Error::SourceTarEntryUnpack { tarball: tarball.into(), entry: entry_path, target: target_path, err });
            },
        }

        // Done, go to next entry
    }

    // Done
    Ok(())
}
