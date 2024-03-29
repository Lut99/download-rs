//  DOWNLOAD.rs
//    by Lut99
//
//  Created:
//    11 Mar 2024, 15:53:15
//  Last edited:
//    12 Mar 2024, 10:30:51
//  Auto updated?
//    Yes
//
//  Description:
//!   Defines functions that download files from the internet.
//

use std::fmt::{Display, Formatter, Result as FResult};
use std::io::{Read as _, Write as _};
use std::path::{Path, PathBuf};
use std::str::FromStr as _;
use std::{error, fs};

pub use console::Style;
use indicatif::{ProgressBar, ProgressStyle};
use reqwest::blocking::{Client, Request, Response};
use reqwest::StatusCode;
use sha2::{Digest as _, Sha256};
use url::Url;
#[cfg(feature = "async-tokio")]
use ::{
    reqwest::{Client as AsyncClient, Request as AsyncRequest, Response as AsyncResponse},
    tokio::fs as tfs,
    tokio::io::AsyncWriteExt as _,
    tokio_stream::StreamExt as _,
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
    /// Failed to build a new request to the given URL.
    RequestCreate { url: String, err: reqwest::Error },
    /// Failed to execute a request to the given URL.
    RequestExecute { url: String, err: reqwest::Error },
    /// Failed to download a chunk of the response.
    ResponseDownload { url: String, err: std::io::Error },
    /// Failed to download a chunk of the response in async mode.
    #[cfg(feature = "async-tokio")]
    ResponseDownloadAsync { url: String, err: reqwest::Error },
    /// The given response was not an OK-response.
    ResponseNotOk { url: String, code: StatusCode, response: Option<String> },
    /// The downloaded target did not match the given checksum.
    SecurityChecksum { path: PathBuf, got: String, expected: String },
    /// HTTPS security was enabled, but the target address isn't HTTPS (or couldn't be parsed).
    SecurityNoHttps { url: String },
    /// Failed to parse the source URL as a... well... URL.
    SourceParse { raw: String, err: url::ParseError },
    /// Failed to create the target for writing.
    TargetCreate { path: PathBuf, err: std::io::Error },
    /// The target's directory is not found.
    TargetParentNotFound { path: PathBuf },
    /// Failed to write to the given target.
    TargetWrite { path: PathBuf, err: std::io::Error },
}
impl Display for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> FResult {
        use Error::*;
        match self {
            RequestCreate { url, .. } => write!(f, "Failed to create GET-request to '{url}'"),
            RequestExecute { url, .. } => write!(f, "Failed to execute GET-request to '{url}'"),
            ResponseDownload { url, .. } => write!(f, "Failed to download response body from '{url}'"),
            #[cfg(feature = "async-tokio")]
            ResponseDownloadAsync { url, .. } => write!(f, "Failed to download response body from '{url}'"),
            ResponseNotOk { url, code, response } => write!(
                f,
                "GET-request to '{}' failed with {} ({}){}",
                url,
                code.as_u16(),
                code.canonical_reason().unwrap_or("???"),
                if let Some(res) = response {
                    format!("\n\nResponse:\n{}\n{}\n{}\n", (0..80).map(|_| '-').collect::<String>(), res, (0..80).map(|_| '-').collect::<String>())
                } else {
                    String::new()
                }
            ),
            SecurityChecksum { path, got, expected } => {
                write!(f, "Checksum of downloaded file '{}' does not match (got '{}', expected '{}')", path.display(), got, expected)
            },
            SecurityNoHttps { url } => write!(f, "HTTPS check enabled, but given url '{url}' does not have an HTTPS request"),
            SourceParse { raw, .. } => write!(f, "Failed to parse source '{raw}' as a URL"),
            TargetCreate { path, .. } => write!(f, "Failed to create target file '{}'", path.display()),
            TargetParentNotFound { path } => write!(f, "Target's parent directory '{}' not found", path.display()),
            TargetWrite { path, .. } => write!(f, "Failed to write to target file '{}'", path.display()),
        }
    }
}
impl error::Error for Error {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        use Error::*;
        match self {
            RequestCreate { err, .. } => Some(err),
            RequestExecute { err, .. } => Some(err),
            ResponseDownload { err, .. } => Some(err),
            #[cfg(feature = "async-tokio")]
            ResponseDownloadAsync { err, .. } => Some(err),
            ResponseNotOk { .. } => None,
            SecurityChecksum { .. } => None,
            SecurityNoHttps { .. } => None,
            SourceParse { err, .. } => Some(err),
            TargetCreate { err, .. } => Some(err),
            TargetParentNotFound { .. } => None,
            TargetWrite { err, .. } => Some(err),
        }
    }
}





/***** AUXILLARY *****/
/// Defines things to do to assert a downloaded file is secure and what we expect.
#[derive(Clone, Debug)]
pub struct DownloadSecurity<'c> {
    /// If not `None`, then it defined the checksum that the file should have.
    pub checksum: Option<&'c [u8]>,
    /// If true, then the file can only be downloaded over HTTPS.
    pub https:    bool,
}
impl<'c> DownloadSecurity<'c> {
    /// Constructor for the DownloadSecurity that enables with all security measures enabled.
    ///
    /// This will provide you with the most security, but is also the slowest method (since it does both encryption and checksum computation).
    ///
    /// Usually, it sufficies to only use a checksum (`DownloadSecurity::checksum()`) if you know what the file looks like a-priori.
    ///
    /// # Arguments
    /// - `checksum`: The checksum that we want the file to have. If you are unsure, give a garbage checksum, then run the function once and check what the file had (after making sure the download went correctly, of course).
    ///
    /// # Returns
    /// A new DownloadSecurity instance that will make your downloaded file so secure you can use it to store a country's deficit (not legal advice).
    ///
    /// # Example
    /// ```rust
    /// use download::{download_file, DownloadSecurity};
    /// use hex_literal::hex;
    ///
    /// // Download some file
    /// let url = "https://raw.githubusercontent.com/Lut99/download-rs/main/LICENSE";
    /// let file = std::env::temp_dir().join("index.html");
    /// download_file(
    ///     &url,
    ///     &file,
    ///     DownloadSecurity::all(&hex!(
    ///         "c71d239df91726fc519c6eb72d318ec65820627232b2f796219e87dcf35d0ab4"
    ///     )),
    ///     None,
    /// )
    /// .unwrap();
    ///
    /// // It exists now!
    /// assert!(file.is_file());
    /// assert!(std::fs::read_to_string(&file).is_ok());
    /// ```
    /// ```rust
    /// use download::{download_file, DownloadSecurity, Error};
    /// use hex_literal::hex;
    ///
    /// // Using a non-HTTPS URL
    /// let url = "http://raw.githubusercontent.com/Lut99/download-rs/main/LICENSE";
    /// let file = std::env::temp_dir().join("index.html");
    /// match download_file(&url, &file, DownloadSecurity::all(&hex!("deadbeef")), None) {
    ///     Err(Error::SecurityNoHttps { .. }) => println!("Yeah that failed"),
    /// #   got => panic!("Did not crash as expected, got {got:?}"),
    /// }
    /// ```
    /// ```rust
    /// use download::{download_file, DownloadSecurity, Error};
    /// use hex_literal::hex;
    ///
    /// // Using the wrong checksum!
    /// let url = "https://raw.githubusercontent.com/Lut99/download-rs/main/LICENSE";
    /// let file = std::env::temp_dir().join("index.html");
    /// match download_file(&url, &file, DownloadSecurity::all(&hex!("deadbeef")), None) {
    ///     Err(Error::SecurityChecksum { .. }) => println!("Yeah that failed"),
    /// #   got => panic!("Did not crash as expected, got {got:?}"),
    /// }
    /// ```
    #[inline]
    pub fn all(checkum: &'c [u8]) -> Self { Self { checksum: Some(checkum), https: true } }

    /// Constructor for the DownloadSecurity that enables checksum verification only.
    ///
    /// Using this method is considered secure, since it guarantees that the downloaded file is what we expect. It is thus safe to use if you don't trust either the network or the remote praty.
    ///
    /// Note, however, that this method only works if you know a-priori what the downloaded file should look like. If not, you must use another security method (e.g., `DownloadSecurity::https()`).
    ///
    /// # Arguments
    /// - `checksum`: The checksum that we want the file to have. If you are unsure, give a garbage checksum, then run the function once and check what the file had (after making sure the download went correctly, of course).
    ///
    /// # Returns
    /// A new DownloadSecurity instance that will make sure your file has the given checksum before returning.
    ///
    /// # Example
    /// ```rust
    /// use download::{download_file, DownloadSecurity};
    /// use hex_literal::hex;
    ///
    /// // Download some file
    /// let url = "https://raw.githubusercontent.com/Lut99/download-rs/main/LICENSE";
    /// let file = std::env::temp_dir().join("index.html");
    /// download_file(
    ///     &url,
    ///     &file,
    ///     DownloadSecurity::checksum(&hex!(
    ///         "c71d239df91726fc519c6eb72d318ec65820627232b2f796219e87dcf35d0ab4"
    ///     )),
    ///     None,
    /// )
    /// .unwrap();
    ///
    /// // It exists now!
    /// assert!(file.is_file());
    /// assert!(std::fs::read_to_string(&file).is_ok());
    /// ```
    /// ```rust
    /// use download::{download_file, DownloadSecurity, Error};
    /// use hex_literal::hex;
    ///
    /// // Using the wrong checksum!
    /// let url = "https://raw.githubusercontent.com/Lut99/download-rs/main/LICENSE";
    /// let file = std::env::temp_dir().join("index.html");
    /// match download_file(&url, &file, DownloadSecurity::checksum(&hex!("deadbeef")), None) {
    ///     Err(Error::SecurityChecksum { .. }) => println!("Yeah that failed"),
    /// #   got => panic!("Did not crash as expected, got {got:?}"),
    /// }
    /// ```
    #[inline]
    pub fn checksum(checkum: &'c [u8]) -> Self { Self { checksum: Some(checkum), https: false } }

    /// Constructor for the DownloadSecurity that forces downloads to go over HTTPS.
    ///
    /// You should only use this method if you trust the remote party. However, if you do, then it guarantees that there was no man-in-the-middle changing the downloaded file.
    ///
    /// # Returns
    /// A new DownloadSecurity instance that will make sure your file if downloaded over HTTPS only.
    ///
    /// # Example
    /// ```rust
    /// use download::{download_file, DownloadSecurity};
    ///
    /// // Download some file
    /// let url = "https://raw.githubusercontent.com/Lut99/download-rs/main/LICENSE";
    /// let file = std::env::temp_dir().join("index.html");
    /// download_file(&url, &file, DownloadSecurity::https(), None).unwrap();
    ///
    /// // It exists now!
    /// assert!(file.is_file());
    /// assert!(std::fs::read_to_string(&file).is_ok());
    /// ```
    /// ```rust
    /// use download::{download_file, DownloadSecurity, Error};
    ///
    /// // Using a non-HTTPS URL
    /// let url = "http://raw.githubusercontent.com/Lut99/download-rs/main/LICENSE";
    /// let file = std::env::temp_dir().join("index.html");
    /// match download_file(&url, &file, DownloadSecurity::https(), None) {
    ///     Err(Error::SecurityNoHttps { .. }) => println!("Yeah that failed"),
    /// #   got => panic!("Did not crash as expected, got {got:?}"),
    /// }
    /// ```
    #[inline]
    pub fn https() -> Self { Self { checksum: None, https: true } }

    /// Constructor for the DownloadSecurity that disabled all security measures.
    ///
    /// For obvious reasons, this security is not recommended unless you trust both the network _and_ the remote party.
    ///
    /// # Returns
    /// A new DownloadSecurity instance that will require no additional security measures on the downloaded file.
    ///
    /// # Example
    /// ```rust
    /// use download::{download_file, DownloadSecurity};
    ///
    /// // Download some file
    /// let url = "https://raw.githubusercontent.com/Lut99/download-rs/main/LICENSE";
    /// let file = std::env::temp_dir().join("index.html");
    /// download_file(&url, &file, DownloadSecurity::none(), None).unwrap();
    ///
    /// // It exists now!
    /// assert!(file.is_file());
    /// assert!(std::fs::read_to_string(&file).is_ok());
    /// ```
    #[inline]
    pub fn none() -> Self { Self { checksum: None, https: false } }
}
impl<'c> Display for DownloadSecurity<'c> {
    fn fmt(&self, f: &mut Formatter<'_>) -> FResult {
        // Write what is enabled
        if let Some(checksum) = &self.checksum {
            write!(f, "Checksum ({})", hex::encode(checksum))?;
            if self.https {
                write!(f, ", HTTPS")?;
            }
            Ok(())
        } else if self.https {
            write!(f, "HTTPS")
        } else {
            write!(f, "None")
        }
    }
}





/***** LIBRARY *****/
/// Downloads some file from the interwebs to the given location.
///
/// If you enabled the `async-tokio` feature, also check the [`download_file_async()`]-function for async contexts.
///
/// # Arguments
/// - `source`: The URL to download the file from.
/// - `target`: The location to download the file to.
/// - `verification`: Some method to verify the file is what we think it is. See the `VerifyMethod`-enum for more information.
/// - `verbose`: If not `None`, will print to the output with accents given in the given `Style` (use a non-exciting Style to print without styles).
///
/// # Returns
/// Nothing, except that when it does you can assume a file exists at the given location.
///
/// # Errors
/// This function may error if we failed to download the file or write it (which may happen if the parent directory of `local` does not exist, among other things).
///
/// # Example
/// ```rust
/// use download::{download_file, DownloadSecurity};
///
/// // Download some file
/// let url = "http://theuselessweb.com/index.html";
/// let file = std::env::temp_dir().join("index.html");
/// download_file(&url, &file, DownloadSecurity::none(), None).unwrap();
///
/// // It exists now!
/// assert!(file.is_file());
/// assert!(std::fs::read_to_string(&file).is_ok());
/// ```
pub fn download_file(source: impl AsRef<str>, target: impl AsRef<Path>, security: DownloadSecurity<'_>, verbose: Option<Style>) -> Result<(), Error> {
    let source: &str = source.as_ref();
    let target: &Path = target.as_ref();
    debug!("Downloading '{}' to '{}' (Security: {})...", source, target.display(), security);
    if let Some(style) = &verbose {
        println!("Downloading {}...", style.apply_to(source));
    }

    // Parse as a URL
    let url: Url = match Url::from_str(source) {
        Ok(url) => url,
        Err(err) => return Err(Error::SourceParse { raw: source.into(), err }),
    };

    // Assert the download directory exists
    if let Some(parent) = target.parent() {
        if !parent.exists() {
            return Err(Error::TargetParentNotFound { path: parent.into() });
        }
    }

    // Open the target file for writing
    let mut handle: fs::File = match fs::File::create(target) {
        // Ok(handle) => {
        //     // Prepare the permissions to set by reading the file's metadata
        //     let mut permissions: Permissions = match handle.metadata() {
        //         Ok(metadata) => metadata.permissions(),
        //         Err(err)     => { return Err(Error::FileMetadataError{ what: "temporary binary", path: local.into(), err }); },
        //     };
        //     permissions.set_mode(permissions.mode() | 0o100);

        //     // Set them
        //     if let Err(err) = handle.set_permissions(permissions) { return Err(Error::FilePermissionsError{ what: "temporary binary", path: local.into(), err }); }

        //     // Return the handle
        //     handle
        // },
        Ok(handle) => handle,
        Err(err) => {
            return Err(Error::TargetCreate { path: target.into(), err });
        },
    };

    // Send a request
    let mut res: Response = if security.https {
        debug!("Sending download request to '{}' (HTTPS enabled)...", url);

        // Assert the address starts with HTTPS first
        if url.scheme() != "https" {
            return Err(Error::SecurityNoHttps { url: url.into() });
        }

        // Send the request with a user-agent header (to make GitHub happy)
        let client: Client = Client::new();
        let req: Request = match client.get(url.clone()).header("User-Agent", "reqwest").build() {
            Ok(req) => req,
            Err(err) => {
                return Err(Error::RequestCreate { url: url.into(), err });
            },
        };
        match client.execute(req) {
            Ok(req) => req,
            Err(err) => {
                return Err(Error::RequestExecute { url: url.into(), err });
            },
        }
    } else {
        debug!("Sending download request to '{}'...", url);

        // Send the request with a user-agent header (to make GitHub happy)
        let client: Client = Client::new();
        let req: Request = match client.get(url.clone()).header("User-Agent", "reqwest").build() {
            Ok(req) => req,
            Err(err) => {
                return Err(Error::RequestCreate { url: url.into(), err });
            },
        };
        match client.execute(req) {
            Ok(req) => req,
            Err(err) => {
                return Err(Error::RequestExecute { url: url.into(), err });
            },
        }
    };

    // Assert it succeeded
    if !res.status().is_success() {
        return Err(Error::ResponseNotOk { url: url.into(), code: res.status(), response: res.text().ok() });
    }

    // Create the progress bar based on whether if there is a length
    debug!("Downloading response to file '{}'...", target.display());
    let len: Option<u64> = res.headers().get("Content-Length").and_then(|len| len.to_str().ok()).and_then(|len| u64::from_str(len).ok());
    let prgs: Option<ProgressBar> = if verbose.is_some() {
        Some(if let Some(len) = len {
            ProgressBar::new(len)
                .with_style(ProgressStyle::with_template("    {bar:60} {bytes}/{total_bytes} {bytes_per_sec} ETA {eta_precise}").unwrap())
        } else {
            ProgressBar::new_spinner()
                .with_style(ProgressStyle::with_template("    {elapsed_precise} {bar:60} {bytes} {binary_bytes_per_sec}").unwrap())
        })
    } else {
        None
    };

    // Prepare getting a checksum if that is our method of choice
    let mut hasher: Option<Sha256> = if security.checksum.is_some() { Some(Sha256::new()) } else { None };

    // Download the response to the opened output file
    let mut chunk: [u8; 65535] = [0; 65535];
    loop {
        // Read the next chunk
        let chunk_len: usize = match res.read(&mut chunk) {
            Ok(len) => len,
            Err(err) => {
                return Err(Error::ResponseDownload { url: url.into(), err });
            },
        };
        if chunk_len == 0 {
            break;
        }
        let next: &[u8] = &chunk[..chunk_len];

        // Write it to the file
        if let Err(err) = handle.write(&next) {
            return Err(Error::TargetWrite { path: target.into(), err });
        }

        // If desired, update the hash
        if let Some(hasher) = &mut hasher {
            hasher.update(&*next);
        }

        // Update what we've written if needed
        if let Some(prgs) = &prgs {
            prgs.update(|state| state.set_pos(state.pos() + next.len() as u64));
        }
    }
    if let Some(prgs) = &prgs {
        prgs.finish_and_clear();
    }

    // Assert the checksums are the same if we're doing that
    if let Some(checksum) = security.checksum {
        // Finalize the hasher first
        let result = hasher.unwrap().finalize();
        debug!("Verifying checksum...");

        // Assert the checksums check out (wheezes)
        if &result[..] != checksum {
            return Err(Error::SecurityChecksum { path: target.into(), expected: hex::encode(checksum), got: hex::encode(&result[..]) });
        }

        // Print that the checksums are equal if asked
        if let Some(style) = verbose {
            // Create the dim styles
            let dim: Style = Style::new().dim();
            let accent: Style = style.dim();

            // Write it with those styles
            println!("{}{}{}", dim.apply_to(" > Checksum "), accent.apply_to(hex::encode(&result[..])), dim.apply_to(" OK"));
        }
    }

    // Done
    Ok(())
}

/// Downloads some file from the interwebs to the given location.
///
/// This variation is built using [`tokio`] versions of the normal operations, and is as such only available on the `async-tokio` feature.
///
/// # Arguments
/// - `source`: The URL to download the file from.
/// - `target`: The location to download the file to.
/// - `verification`: Some method to verify the file is what we think it is. See the `VerifyMethod`-enum for more information.
/// - `verbose`: If not `None`, will print to the output with accents given in the given `Style` (use a non-exciting Style to print without styles).
///
/// # Returns
/// Nothing, except that when it does you can assume a file exists at the given location.
///
/// # Errors
/// This function may error if we failed to download the file or write it (which may happen if the parent directory of `local` does not exist, among other things).
///
/// # Example
/// ```rust
/// # tokio_test::block_on(async {
/// use download::{download_file_async, DownloadSecurity};
///
/// // Download some file
/// let url = "https://theuselessweb.com/index.html";
/// let file = std::env::temp_dir().join("index.html");
/// download_file_async(&url, &file, DownloadSecurity::none(), None).await.unwrap();
///
/// // It exists now!
/// assert!(file.is_file());
/// assert!(tokio::fs::read_to_string(&file).await.is_ok());
/// # });
/// ```
#[cfg(feature = "async-tokio")]
pub async fn download_file_async(
    source: impl AsRef<str>,
    target: impl AsRef<Path>,
    security: DownloadSecurity<'_>,
    verbose: Option<Style>,
) -> Result<(), Error> {
    let source: &str = source.as_ref();
    let target: &Path = target.as_ref();
    debug!("Downloading '{}' to '{}' (Security: {})...", source, target.display(), security);
    if let Some(style) = &verbose {
        println!("Downloading {}...", style.apply_to(source));
    }

    // Assert the download directory exists
    if let Some(parent) = target.parent() {
        if !parent.exists() {
            return Err(Error::TargetParentNotFound { path: parent.into() });
        }
    }

    // Open the target file for writing
    let mut handle: tfs::File = match tfs::File::create(target).await {
        // Ok(handle) => {
        //     // Prepare the permissions to set by reading the file's metadata
        //     let mut permissions: Permissions = match handle.metadata() {
        //         Ok(metadata) => metadata.permissions(),
        //         Err(err)     => { return Err(Error::FileMetadataError{ what: "temporary binary", path: local.into(), err }); },
        //     };
        //     permissions.set_mode(permissions.mode() | 0o100);

        //     // Set them
        //     if let Err(err) = handle.set_permissions(permissions) { return Err(Error::FilePermissionsError{ what: "temporary binary", path: local.into(), err }); }

        //     // Return the handle
        //     handle
        // },
        Ok(handle) => handle,
        Err(err) => {
            return Err(Error::TargetCreate { path: target.into(), err });
        },
    };

    // Send a request
    let res: AsyncResponse = if security.https {
        debug!("Sending download request to '{}' (HTTPS enabled)...", source);

        // Assert the address starts with HTTPS first
        if Url::parse(source).ok().map(|u| u.scheme() != "https").unwrap_or(true) {
            return Err(Error::SecurityNoHttps { url: source.into() });
        }

        // Send the request with a user-agent header (to make GitHub happy)
        let client: AsyncClient = AsyncClient::new();
        let req: AsyncRequest = match client.get(source).header("User-Agent", "reqwest").build() {
            Ok(req) => req,
            Err(err) => {
                return Err(Error::RequestCreate { url: source.into(), err });
            },
        };
        match client.execute(req).await {
            Ok(req) => req,
            Err(err) => {
                return Err(Error::RequestExecute { url: source.into(), err });
            },
        }
    } else {
        debug!("Sending download request to '{}'...", source);

        // Send the request with a user-agent header (to make GitHub happy)
        let client: AsyncClient = AsyncClient::new();
        let req: AsyncRequest = match client.get(source).header("User-Agent", "reqwest").build() {
            Ok(req) => req,
            Err(err) => {
                return Err(Error::RequestCreate { url: source.into(), err });
            },
        };
        match client.execute(req).await {
            Ok(req) => req,
            Err(err) => {
                return Err(Error::RequestExecute { url: source.into(), err });
            },
        }
    };

    // Assert it succeeded
    if !res.status().is_success() {
        return Err(Error::ResponseNotOk { url: source.into(), code: res.status(), response: res.text().await.ok() });
    }

    // Create the progress bar based on whether if there is a length
    debug!("Downloading response to file '{}'...", target.display());
    let len: Option<u64> = res.headers().get("Content-Length").and_then(|len| len.to_str().ok()).and_then(|len| u64::from_str(len).ok());
    let prgs: Option<ProgressBar> = if verbose.is_some() {
        Some(if let Some(len) = len {
            ProgressBar::new(len)
                .with_style(ProgressStyle::with_template("    {bar:60} {bytes}/{total_bytes} {bytes_per_sec} ETA {eta_precise}").unwrap())
        } else {
            ProgressBar::new_spinner()
                .with_style(ProgressStyle::with_template("    {elapsed_precise} {bar:60} {bytes} {binary_bytes_per_sec}").unwrap())
        })
    } else {
        None
    };

    // Prepare getting a checksum if that is our method of choice
    let mut hasher: Option<Sha256> = if security.checksum.is_some() { Some(Sha256::new()) } else { None };

    // Download the response to the opened output file
    let mut stream = res.bytes_stream();
    while let Some(next) = stream.next().await {
        // Unwrap the result
        let next = match next {
            Ok(next) => next,
            Err(err) => {
                return Err(Error::ResponseDownloadAsync { url: source.into(), err });
            },
        };

        // Write it to the file
        if let Err(err) = handle.write(&next).await {
            return Err(Error::TargetWrite { path: target.into(), err });
        }

        // If desired, update the hash
        if let Some(hasher) = &mut hasher {
            hasher.update(&*next);
        }

        // Update what we've written if needed
        if let Some(prgs) = &prgs {
            prgs.update(|state| state.set_pos(state.pos() + next.len() as u64));
        }
    }
    if let Some(prgs) = &prgs {
        prgs.finish_and_clear();
    }

    // Assert the checksums are the same if we're doing that
    if let Some(checksum) = security.checksum {
        // Finalize the hasher first
        let result = hasher.unwrap().finalize();
        debug!("Verifying checksum...");

        // Assert the checksums check out (wheezes)
        if &result[..] != checksum {
            return Err(Error::SecurityChecksum { path: target.into(), expected: hex::encode(checksum), got: hex::encode(&result[..]) });
        }

        // Print that the checksums are equal if asked
        if let Some(style) = verbose {
            // Create the dim styles
            let dim: Style = Style::new().dim();
            let accent: Style = style.dim();

            // Write it with those styles
            println!("{}{}{}", dim.apply_to(" > Checksum "), accent.apply_to(hex::encode(&result[..])), dim.apply_to(" OK"));
        }
    }

    // Done
    Ok(())
}
