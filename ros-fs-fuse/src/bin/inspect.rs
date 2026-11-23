//! Read a `ros-fs` image without mounting it.
//!
//! `ros-fs` has no kernel dependencies, so the kernel's own filesystem code can
//! be pointed at an image file. That makes an image built by `ros-fs-fuse`
//! inspectable on any host: no FUSE, no root, no loop device.
//!
//! ```text
//! inspect ls  fs.img          # list the root directory
//! inspect cat fs.img hello    # write one file to stdout
//! ```

use std::fs::OpenOptions;
use std::io::Write;
use std::sync::{Arc, Mutex};

use clap::{Parser, Subcommand};
use ros_fs::fs::{FileSystem, FileSystemRootInode};
use ros_fs_fuse::block_file::BlockFile;

#[derive(Parser)]
#[command(version, about = "Inspect a ros-fs filesystem image without mounting it")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// List the entries in the image's root directory.
    Ls {
        /// Path to the filesystem image.
        image: String,
    },
    /// Write one file from the image to stdout.
    Cat {
        /// Path to the filesystem image.
        image: String,
        /// Name of the entry in the root directory.
        name: String,
    },
}

fn open(image: &str) -> Arc<Mutex<FileSystem>> {
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .open(image)
        .unwrap_or_else(|e| panic!("cannot open image {image}: {e}"));
    FileSystem::open(Arc::new(BlockFile(Mutex::new(file))))
}

fn main() {
    match Cli::parse().command {
        Command::Ls { image } => {
            let fs = open(&image);
            let root = fs.root_inode();
            let mut names = root.ls();
            names.sort();
            for name in names {
                println!("{name}");
            }
        }
        Command::Cat { image, name } => {
            let fs = open(&image);
            let root = fs.root_inode();
            let inode = root
                .find(&name)
                .unwrap_or_else(|| panic!("no entry named {name} in {image}"));
            let size = inode.get_size() as usize;
            let mut data = vec![0u8; size];
            inode.read_at(0, &mut data);
            std::io::stdout()
                .write_all(&data)
                .expect("failed to write to stdout");
        }
    }
}
