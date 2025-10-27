# CoreRun - A Container Runtime in Rust

A lightweight, educational container runtime implementation written in Rust that demonstrates core containerization concepts including namespaces, cgroups, filesystem isolation, and volume mounting.

## Features

### Core Container Technologies

- **Linux Namespaces**: Process (PID), network, mount, UTS (hostname), IPC, and user namespace isolation
- **Control Groups (cgroups v2)**: Resource limiting for memory, CPU, and process count
- **Filesystem Isolation**: Container root filesystem setup with pivot_root
- **Volume Management**: Bind mount support for sharing host directories with containers

### Resource Management

- Memory limiting (MB)
- CPU usage limiting (percentage-based)
- Process/thread count limiting
- Automatic cleanup on container exit

### Additional Features

- Custom hostname support
- Comprehensive error handling
- Detailed logging
- Command-line interface with clap

## Prerequisites

- Linux system with cgroups v2 support
- Root privileges (required for namespace and mount operations)
- Rust 2024 edition

## Installation

1. Clone the repository:

```bash
git clone <repository-url>
cd container_rs
```

2. Build the project:

```bash
cargo build --release
```

## Usage

### Basic Usage

Run a simple command in an isolated container:

```bash
sudo ./target/release/corerun --rootfs /path/to/rootfs /bin/sh
```

### Advanced Usage with Resource Limits

```bash
sudo ./target/release/corerun \
    --rootfs /path/to/rootfs \
    --memory 512 \
    --cpu 50 \
    --pids 100 \
    --hostname my-container \
    --volume /host/data:/container/data:rw \
    --volume /host/config:/container/config:ro \
    /bin/bash -c "echo 'Hello from container'"
```

### Command Line Options

| Option       | Short | Description                                  | Example                        |
| ------------ | ----- | -------------------------------------------- | ------------------------------ |
| `--rootfs`   | -     | Path to container root filesystem (required) | `--rootfs /tmp/alpine-rootfs`  |
| `--memory`   | `-m`  | Memory limit in MB                           | `--memory 512`                 |
| `--cpu`      | `-c`  | CPU limit as percentage of one core          | `--cpu 50`                     |
| `--pids`     | `-p`  | Maximum number of processes/threads          | `--pids 100`                   |
| `--hostname` | -     | Container hostname                           | `--hostname my-container`      |
| `--volume`   | `-v`  | Bind mount volumes (repeatable)              | `--volume /host:/container:rw` |

### Volume Format

Volumes use the format: `/host/path:/container/path[:permissions]`

- Permissions: `rw` (read-write, default) or `ro` (read-only)
- Example: `--volume /home/user/data:/app/data:ro`

## Setting Up a Root Filesystem

You'll need a root filesystem to run containers. Here are a few options:

### Option 1: Download Alpine Linux Root Filesystem

```bash
wget https://dl-cdn.alpinelinux.org/alpine/latest-stable/releases/x86_64/alpine-minirootfs-3.18.4-x86_64.tar.gz
mkdir rootfs
cd rootfs
sudo tar -xzf ../alpine-minirootfs-3.18.4-x86_64.tar.gz
```

### Option 2: Create with Docker

```bash
docker create --name temp alpine:latest
docker export temp | tar -xC rootfs/
docker rm temp
```

### Option 3: Use debootstrap (Debian/Ubuntu)

```bash
sudo debootstrap stable rootfs http://deb.debian.org/debian/
```

## Project Structure

```
src/
├── main.rs              # Application entry point
├── cli/                 # Command-line interface
│   ├── mod.rs
│   └── cli.rs
├── error/               # Error handling
│   ├── mod.rs
│   └── error.rs
├── namespace/           # Linux namespace management
│   ├── mod.rs
│   └── namespace.rs
├── filesystem/          # Container filesystem setup
│   ├── mod.rs
│   └── filesystem.rs
├── process/             # Process execution and management
│   ├── mod.rs
│   └── process.rs
├── cgroup/              # Control groups (resource limiting)
│   ├── mod.rs
│   └── cgroup.rs
└── volume/              # Volume and bind mount management
    ├── mod.rs
    ├── volume.rs
    ├── volume_manager.rs
    └── impl_volume.rs
```

## How It Works

1. **Privilege Check**: Verifies root privileges are available
2. **Namespace Creation**: Creates isolated namespaces for the container
3. **Resource Limiting**: Sets up cgroups if resource limits are specified
4. **Volume Setup**: Configures bind mounts for shared directories
5. **Namespace Entry**: Enters the new PID namespace (becomes PID 1)
6. **Hostname Setup**: Sets the container hostname
7. **Filesystem Setup**: Prepares the container root filesystem using pivot_root
8. **Process Execution**: Runs the specified command inside the container
9. **Cleanup**: Unmounts volumes and cleans up resources on exit

## Dependencies

- `clap`: Command-line argument parsing
- `env_logger` & `log`: Logging infrastructure
- `nix`: Safe Rust bindings for Unix system calls
- `uuid`: UUID generation for unique identifiers
- `thiserror`: Custom error type definitions

## Limitations

This is an educational implementation and has several limitations compared to production container runtimes:

- No image management or layered filesystems
- No network configuration beyond isolation
- No OCI (Open Container Initiative) compliance
- No container orchestration features
- Limited security features
- No checkpoint/restore functionality

## Educational Value

This project demonstrates:

- Linux namespace APIs and their usage
- Control groups (cgroups) v2 implementation
- Filesystem manipulation with pivot_root
- Process management in isolated environments
- Resource limiting and monitoring
- Error handling in system programming
- Modern Rust practices for system-level programming

## Contributing

Contributions are welcome! Areas for improvement:

- Enhanced security features
- Network namespace configuration
- User namespace support
- OCI compliance
- Performance optimizations
- Additional resource controls

## Safety and Security

⚠️ **Warning**: This is an educational tool. Do not use in production environments without thorough security review and testing.

## License

This project is released under MIT.

## References

- [Linux Namespaces Documentation](https://man7.org/linux/man-pages/man7/namespaces.7.html)
- [Control Groups v2](https://www.kernel.org/doc/html/latest/admin-guide/cgroup-v2.html)
- [OCI Runtime Specification](https://github.com/opencontainers/runtime-spec)
- [Container Fundamentals](https://container.training/)
