# CoreRun - A Container Runtime in Rust

A lightweight, educational container runtime implementation written in Rust that demonstrates core containerization concepts including namespaces, cgroups, filesystem isolation, networking, and volume mounting.

## Features

### Core Container Technologies

- **Linux Namespaces**: Process (PID), network, mount, UTS (hostname), IPC, and user namespace isolation.
- **Control Groups (cgroups v2)**: Resource limiting for memory, CPU, and process count.
- **Filesystem Isolation**: Container root filesystem setup with `pivot_root`.
- **Volume Management**: Bind mount support for sharing host directories with containers.

### Networking

- **Bridge Networking**: Creates an isolated network for containers with a dedicated bridge device.
- **Host Networking**: Shares the host's network stack with the container.
- **Container Networking**: Shares the network stack of another container.
- **Port Mapping**: Exposes container ports to the host network.

### Resource Management

- Memory limiting (MB).
- CPU usage limiting (percentage-based).
- Process/thread count limiting.
- Automatic cleanup on container exit.

### Additional Features

- Custom hostname support.
- Comprehensive error handling.
- Detailed logging control.
- Command-line interface with `clap`.

## Prerequisites

- Linux system with cgroups v2 support.
- Root privileges (required for namespace and mount operations).
- Rust 2024 edition.

## Installation

1.  Clone the repository:
    ```bash
    git clone <repository-url>
    cd CoreRun
    ```
2.  Build the project:
    ```bash
    cargo build --release
    ```

## Usage

### Basic Usage

Run a simple command in an isolated container:

```bash
sudo ./target/release/corerun --rootfs /path/to/rootfs /bin/sh
```

### Advanced Usage with Resource and Network Configuration

```bash
sudo ./target/release/corerun \
    --rootfs /path/to/rootfs \
    --memory 512 \
    --cpu 50 \
    --pids 100 \
    --hostname my-container \
    --network bridge \
    -P 8080:80 \
    --volume /host/data:/container/data:rw \
    --log true \
    /bin/bash -c "echo 'Hello from container'"
```

### Command Line Options

| Option | Short | Description | Example |
| --- | --- | --- | --- |
| `--rootfs` | - | Path to container root filesystem (required) | `--rootfs /tmp/alpine-rootfs` |
| `--memory` | `-m` | Memory limit in MB | `--memory 512` |
| `--cpu` | `-c` | CPU limit as percentage of one core | `--cpu 50` |
| `--pids` | `-p` | Maximum number of processes/threads | `--pids 100` |
| `--hostname` | - | Container hostname | `--hostname my-container` |
| `--volume` | `-v` | Bind mount volumes (repeatable) | `--volume /host:/container:rw` |
| `--network`| `-n` | Network mode: `bridge`, `host`, `none`  | `--network bridge` |
| `--port` | `-P` | Publish a container's port to the host | `-P 8080:80/tcp or udp` |
| `--log` | `-l` | Enable or disable logging output | `--log true` |

**Note:** The `--net` / `-N` flag for custom bridge names is defined in the CLI but not yet implemented in the container configuration.

### Volume Format

Volumes use the format: `/host/path:/container/path[:permissions]`

-   Permissions: `rw` (read-write, default) or `ro` (read-only).
-   Example: `--volume /home/user/data:/app/data:ro`

## Networking

CoreRun supports four networking modes:

-   **`bridge` (default)**: Creates a virtual Ethernet (veth) pair for the container and connects it to a network bridge on the host. This provides an isolated network for the container.
-   **`host`**: The container shares the host's network stack. Any services running in the container will be accessible on the host's IP address.
-   **`none`**: The container has only a loopback interface and is completely isolated from the network.
-   **`multi`**: Connect multiple containers to the same virtual network [default: bridge].

### Port Mapping

When using `bridge` mode, you can expose a container's port to the host using the `--port` or `-P` flag.

-   Format: `host_port:container_port/protocol`
-   Example: `-P 8080:80/tcp` maps port 80 in the container to port 8080 on the host.

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
├── error/               # Error handling
├── namespace/           # Linux namespace management
├── filesystem/          # Container filesystem setup
├── process/             # Process execution and management
├── cgroup/              # Control groups (resource limiting)
├── network/             # Network management (bridge, veth, etc.)
└── volume/              # Volume and bind mount management
```

## How It Works

1.  **Argument Parsing**: The command-line arguments are parsed to create a `ContainerConfig`.
2.  **Privilege Check**: Verifies that the process is running with root privileges.
3.  **Fork for Network Setup**: The main process forks into a parent and a child.
    -   The **parent process** waits for the child to signal that it has entered its new namespaces. It then configures the container's network from the host side (e.g., setting up the veth pair and adding it to the bridge).
    -   The **child process** proceeds with creating and entering the new namespaces (PID, mount, UTS, etc.).
4.  **Resource Limiting**: Sets up cgroups to enforce memory, CPU, and PID limits.
5.  **Namespace Entry**: The child process becomes PID 1 in the new PID namespace.
6.  **Hostname Setup**: Sets the container's hostname.
7.  **Filesystem Setup**: Prepares the container's root filesystem using `pivot_root`, making it the new root.
8.  **Volume Setup**: Configures bind mounts for shared directories.
9.  **Process Execution**: The child process executes the user-specified command inside the fully isolated container.
10. **Cleanup**: When the container command exits, the parent process cleans up network resources, and the kernel tears down the namespaces and cgroups.

## Dependencies

-   `clap`: Command-line argument parsing
-   `env_logger` & `log`: Logging infrastructure
-   `nix`: Safe Rust bindings for Unix system calls
-   `uuid`: UUID generation for unique identifiers
-   `thiserror`: Custom error type definitions

## Limitations

This is an educational implementation and has several limitations compared to production container runtimes:

-   No image management or layered filesystems.
-   No OCI (Open Container Initiative) compliance.
-   No container orchestration features.
-   Limited security features.
-   No checkpoint/restore functionality.

## Educational Value

This project demonstrates:

-   Linux namespace APIs and their usage.
-   Control groups (cgroups) v2 implementation.
-   Filesystem manipulation with `pivot_root`.
-   Process management in isolated environments, including the fork/exec model for setup.
-   Container networking with bridges and veth pairs.
-   Resource limiting and monitoring.
-   Error handling in system programming.
-   Modern Rust practices for system-level programming.

## Contributing

Contributions are welcome! Areas for improvement:

-   Enhanced security features (e.g., seccomp, AppArmor).
-   User namespace support for rootless containers.
-   OCI compliance.
-   Performance optimizations.
-   Additional resource controls.
-   Implement the `--net` flag to allow custom bridge names.

## Safety and Security

⚠️ **Warning**: This is an educational tool. Do not use in production environments without a thorough security review and testing.

## License

This project is released under the MIT License.

## References

-   [Linux Namespaces Documentation](https://man7.org/linux/man-pages/man7/namespaces.7.html)
-   [Control Groups v2](https://www.kernel.org/doc/html/latest/admin-guide/cgroup-v2.html)
-   [OCI Runtime Specification](https://github.com/opencontainers/runtime-spec)
-   [Container Fundamentals](https://container.training/)