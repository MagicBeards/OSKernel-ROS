//! Host-side tooling for the ROS666 filesystem.
//!
//! `ros-fs` itself is `#![no_std]` and reaches storage only through the
//! `BlockDevice` trait, so the same code that the kernel runs can also read and
//! write an image file on a normal machine. This crate is the host side of that:
//! [`block_file::BlockFile`] adapts a `std::fs::File` to `BlockDevice`, and the
//! two binaries built from it are `ros-fs-fuse` (packs an image) and `inspect`
//! (reads one back).

pub mod block_file;
