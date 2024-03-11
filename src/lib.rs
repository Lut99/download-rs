//  LIB.rs
//    by Lut99
//
//  Created:
//    11 Mar 2024, 15:52:32
//  Last edited:
//    11 Mar 2024, 15:54:58
//  Auto updated?
//    Yes
//
//  Description:
//!   Provides some simple-to-use wrappers for downloading and managing
//!   files from the internet.
//

// Declare the modules
#[cfg(feature = "download")]
pub mod download;
#[cfg(feature = "tar")]
pub mod tar;
