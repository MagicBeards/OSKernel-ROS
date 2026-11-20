//! Integration tests for `ros-fs` against an in-memory block device.
//!
//! The crate is `#![no_std]` and reaches storage only through
//! [`BlockDevice`](ros_fs::block_dev::BlockDevice), so a `Vec<u8>` is a
//! sufficient device and none of this needs a kernel or a real disk.
//!
//! Two details follow from `no_std`:
//!
//! - `ros-fs` guards its filesystem with `spin::Mutex`, not `std::sync::Mutex`,
//!   so the handles below use the same type. `spin` is a dev-dependency.
//! - `BLOCK_CACHE_MANAGER` is a process-wide singleton and
//!   `BlockCacheManager::get_cache` matches on the block id alone, without
//!   comparing the device. Two filesystems open at once in one process collide
//!   on cache slot 0 and read each other's data. Every test therefore takes
//!   `SERIAL` and resets the cache first.

use std::sync::{Arc, Mutex as StdMutex, MutexGuard};

use ros_fs::block_cache::{
    BLOCK_CACHE_MANAGER, BLOCK_SIZE, BlockCacheManager, block_cache_sync_all,
};
use ros_fs::block_dev::BlockDevice;
use ros_fs::fs::{FileSystem, FileSystemRootInode};
use ros_fs::layout::disk_inode::InodeType;
use spin::Mutex;

/// A one-block inode bitmap covers 4096 inodes, and DiskInode is 128 bytes, so
/// the inode area alone needs 1024 blocks. Anything below ~1026 blocks underflows
/// the remaining-blocks arithmetic in FileSystem::new.
const TEST_BLOCKS: usize = 4096;

/// Serialises the tests, because the block cache is global.
static SERIAL: StdMutex<()> = StdMutex::new(());

fn serial() -> MutexGuard<'static, ()> {
    // A panicking test poisons the lock; the next test still has to run.
    let guard = SERIAL.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    *BLOCK_CACHE_MANAGER.lock() = BlockCacheManager::new();
    guard
}

/// A block device backed by a flat byte vector.
struct MemBlockDevice {
    bytes: StdMutex<Vec<u8>>,
}

impl MemBlockDevice {
    fn new(total_blocks: usize) -> Self {
        Self {
            bytes: StdMutex::new(vec![0u8; total_blocks * BLOCK_SIZE]),
        }
    }
}

impl BlockDevice for MemBlockDevice {
    fn read_block(&self, block_id: usize, buf: &mut [u8]) {
        let bytes = self.bytes.lock().unwrap();
        let start = block_id * BLOCK_SIZE;
        buf[..BLOCK_SIZE].copy_from_slice(&bytes[start..start + BLOCK_SIZE]);
    }

    fn write_block(&self, block_id: usize, buf: &[u8]) {
        let mut bytes = self.bytes.lock().unwrap();
        let start = block_id * BLOCK_SIZE;
        bytes[start..start + BLOCK_SIZE].copy_from_slice(&buf[..BLOCK_SIZE]);
    }
}

/// Formats a filesystem on a fresh device and hands back both ends.
fn format(
    total_blocks: usize,
    inode_bitmap_blocks: usize,
) -> (Arc<MemBlockDevice>, Arc<Mutex<FileSystem>>) {
    let dev = Arc::new(MemBlockDevice::new(total_blocks));
    let fs = FileSystem::new(dev.clone(), total_blocks as u32, inode_bitmap_blocks as u32);
    (dev, fs)
}

#[test]
fn create_write_and_read_back() {
    let _guard = serial();
    let (_dev, fs) = format(TEST_BLOCKS, 1);
    let root = fs.root_inode();

    let file = root.create("hello.txt", InodeType::File);
    let payload: &[u8] = b"hello, ros-fs";
    assert_eq!(payload.len(), 13);
    assert_eq!(file.write_at(0, payload), payload.len());

    let mut buf = [0u8; 13];
    assert_eq!(file.read_at(0, &mut buf), payload.len());
    assert_eq!(&buf, payload);
    assert_eq!(file.get_size(), payload.len() as u32);
}

#[test]
fn entries_survive_a_reopen() {
    let _guard = serial();
    let (dev, fs) = format(TEST_BLOCKS, 1);

    let root = fs.root_inode();
    let file = root.create("persist.txt", InodeType::File);
    file.write_at(0, b"written before reopen");

    // Flush the cache, then drop every handle so the second open reads the device.
    block_cache_sync_all();
    drop(root);
    drop(file);
    drop(fs);

    let reopened = FileSystem::open(dev.clone());
    let root = reopened.root_inode();
    let file = root.find("persist.txt").expect("entry lost across reopen");

    let mut buf = [0u8; 21];
    assert_eq!(file.read_at(0, &mut buf), 21);
    assert_eq!(&buf, b"written before reopen");
}

#[test]
fn ls_reports_created_entries() {
    let _guard = serial();
    let (_dev, fs) = format(TEST_BLOCKS, 1);
    let root = fs.root_inode();

    assert!(root.ls().is_empty(), "a fresh root should be empty");

    root.create("a.txt", InodeType::File);
    root.create("b.txt", InodeType::File);
    root.create("subdir", InodeType::Dir);

    let mut names = root.ls();
    names.sort();
    assert_eq!(names, vec!["a.txt", "b.txt", "subdir"]);
}

#[test]
fn writes_spanning_several_blocks_round_trip() {
    let _guard = serial();
    let (_dev, fs) = format(TEST_BLOCKS, 1);
    let root = fs.root_inode();

    // Long enough to cross the direct block pointers and exercise the indirect block.
    let payload: Vec<u8> = (0..BLOCK_SIZE * 4).map(|i| (i % 251) as u8).collect();
    let file = root.create("big.bin", InodeType::File);
    assert_eq!(file.write_at(0, &payload), payload.len());
    assert_eq!(file.get_size(), payload.len() as u32);

    let mut buf = vec![0u8; payload.len()];
    assert_eq!(file.read_at(0, &mut buf), payload.len());
    assert_eq!(buf, payload);
}

#[test]
fn inode_and_data_block_allocation_is_reusable() {
    let _guard = serial();
    let (_dev, fs) = format(TEST_BLOCKS, 1);

    let first = fs.lock().alloc_inode();
    let second = fs.lock().alloc_inode();
    assert_ne!(first, second, "two allocations must not hand out one inode");

    fs.lock().free_inode(first);
    let reused = fs.lock().alloc_inode();
    assert_eq!(reused, first, "a freed inode should be handed out again");

    let block = fs.lock().alloc_data_block();
    fs.lock().free_data_block(block);
    assert_eq!(fs.lock().alloc_data_block(), block);
}
