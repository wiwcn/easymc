//! Desktop Java Edition launcher: find/install Java, build args, spawn process.

mod java;
mod launch;
mod minecraft_dir;
mod process;

pub use java::*;
pub use launch::*;
pub use minecraft_dir::*;
pub use process::*;
