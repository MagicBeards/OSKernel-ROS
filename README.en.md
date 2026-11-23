# ROS666 — a Unix-like kernel in Rust for RISC-V

A small Unix-like operating system kernel written from scratch in Rust, targeting
64-bit RISC-V. It boots under OpenSBI (via RustSBI) on QEMU, runs a set of
user-space programs, and implements preemptive multitasking, virtual memory, and
a disk filesystem.

This began as a competition entry for the 2024 National College Student Computer
System Capability Competition (OS design track, East China region) and is kept
here as a readable, buildable reference implementation. The Chinese-language
project record, team details, and full design handbook live in
[README.md](README.md) and [docs/](docs/).

## What it implements

| Area | Contents |
| --- | --- |
| Boot | `bootloader/` (RustSBI), linker script, entry assembly, kernel stack setup |
| Traps | S-mode trap entry/return via a trampoline, `scause` dispatch, user and kernel entry points |
| Memory | Sv39 paging, per-process address spaces, physical frame allocator, kernel heap |
| Processes | Task control blocks, context switch, scheduling with a timer interrupt, `fork` / `exec` / `wait` |
| Filesystem | `ros-fs`: superblock, inode and bitmap layout, block cache, directory entries, regular files |
| Devices | virtio block device driver, console I/O, PLIC-style interrupt handling |
| User space | `user-lib` (syscall wrappers, `println!`), `user/` (shell, and test programs) |

Roughly 8,000 lines of Rust across 79 files.

## The filesystem runs outside the kernel too

`ros-fs` is a `#![no_std]` crate with no kernel dependencies — it talks to a
block device through a two-method trait. The same code that runs inside the
kernel therefore also builds for a normal host, and `ros-fs-fuse` is the
host-side tool built on that: it formats an image, writes program binaries into
it, then reopens the image and reads every file back to check it byte for byte.

```bash
cargo run -p ros-fs-fuse -- \
  --app hello --target /tmp/apps --output /tmp/fs.img
```

The image it produces is what the kernel mounts as `/`. That makes the on-disk
format reproducible outside the kernel: the packer is a `mkfs` for this
filesystem, and its own verification pass is what keeps the format honest.

Despite the crate name, there is no FUSE implementation — it packs, it does not
mount. Mounting the image on a Linux host would need a FUSE layer that does not
exist here yet.

## Building

The workspace uses the unstable `per-package-target` feature (see
`user-build/`), so **a nightly toolchain is required**. `rust-toolchain.toml`
pins the exact nightly the repository is verified against, so `rustup` selects it
automatically.

```bash
rustup show                      # installs the pinned nightly + riscv64 target
cargo build --target riscv64gc-unknown-none-elf
```

`make` wraps the rest:

```bash
make check-dev-requirements      # checks qemu-system-riscv64, rustup, jq
make build-kernel.dev            # kernel only
make build-users.dev             # user-space programs
make launch-qemu-system.dev      # build and boot in QEMU
```

On Windows the toolchain works, but the QEMU launch targets assume a Unix shell
and `jq`.

## Repository layout

```
src/                kernel: traps, memory, tasks, syscalls, drivers
ros-fs/             the filesystem, no_std, host-buildable
ros-fs-fuse/        FUSE adapter, so the image mounts on Linux
slab_allocator/     kernel heap allocator
user/               user-space programs (shell, tests)
user-lib/           user-space syscall wrappers
user-build/         build glue for user programs
common/             shared constants between kernel and user space
bootloader/         RustSBI payload
docs/               design handbook (PDF) and per-subsystem notes
```

## Status

The kernel builds and boots on QEMU from a clean checkout on the pinned
toolchain; CI builds the kernel for `riscv64gc-unknown-none-elf` and builds
`ros-fs` for the host on every push. It is a teaching-grade kernel rather than a
production OS: no networking, no SMP, a single filesystem type, and the device
layer covers only what QEMU and the K210 board expose.

## Attribution

The design follows the [rCore tutorial](https://rcore-os.cn/rCore-Tutorial-Book-v3/)
closely, and several components are adapted from the projects listed in
[README.md](README.md#参考): [rCore-Tutorial-v3](https://github.com/rcore-os/rCore-Tutorial-v3)
(the filesystem layout in `ros-fs` is derived from its `easy-fs`),
[slab_allocator](https://github.com/weclaw1/slab_allocator),
and [virtio-drivers](https://github.com/rcore-os/virtio-drivers). Licenses are
carried in the respective upstream repositories; see [LICENSE](LICENSE) for this
repository's own.
