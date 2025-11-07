use crate::error::{ContainerError, ContainerResult};
use nix::libc;
use nix::mount::{MsFlags, mount};
use nix::pty::openpty;
use nix::sys::signal::{SigHandler, Signal, kill, signal};
use nix::sys::wait::{WaitPidFlag, WaitStatus, waitpid};
use nix::unistd::{ForkResult, Pid, execve, fork, setsid, tcsetpgrp};
use std::ffi::CString;
use std::fs;
use std::io::{Read, Write};
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
use std::path::Path;
use std::sync::atomic::{AtomicI32, Ordering};

static CHILD_PID: AtomicI32 = AtomicI32::new(0);

extern "C" fn handle_signal(sig: i32) {
    let child = CHILD_PID.load(Ordering::SeqCst);
    if child > 0 {
        if let Ok(signal) = Signal::try_from(sig) {
            let _ = kill(Pid::from_raw(child), signal);
        }
    }
}

#[derive(Debug)]
pub struct ProcessManager;

impl ProcessManager {
    pub fn execute_container_command(command: &str, args: &[String]) -> ContainerResult<()> {
        log::info!("Executing container command: {command} with args: {args:?}");

        // Mount devpts BEFORE attempting to use PTY
        Self::ensure_devpts_mounted()?;

        let command_path = if command.starts_with("/") {
            command.to_string()
        } else {
            ["/bin", "/usr/bin", "/sbin", "/usr/sbin"]
                .iter()
                .map(|prefix| format!("{}/{}", prefix, command))
                .find(|p| Path::new(p).exists())
                .unwrap_or_else(|| format!("/bin/{}", command))
        };

        if !Path::new(&command_path).exists() {
            return Err(ContainerError::process_execution(format!(
                "Command not found in container: {}",
                command_path
            )));
        }

        let argv = Self::build_argv(&command_path, args)?;
        let envp = Self::build_environment()?;

        // Try to create pseudo-terminal, fall back to direct execution if not available
        let use_pty = openpty(None, None).is_ok();

        if use_pty {
            Self::execute_with_pty(command, &argv, &envp)
        } else {
            log::warn!("PTY not available, running without PTY support");
            Self::execute_without_pty(command, &argv, &envp)
        }
    }

    pub fn ensure_devpts_mounted() -> ContainerResult<()> {
        let dev_pts = Path::new("/dev/pts");
        if !dev_pts.exists() {
            log::info!("Creating /dev/pts directory");
            std::fs::create_dir_all(dev_pts).map_err(|e| ContainerError::Cgroup {
                message: format!("Failed to create /dev/pts: {}", e),
            })?;
        }

        // Check if already mounted by reading /proc/mounts
        if let Ok(mounts) = std::fs::read_to_string("/proc/mounts") {
            if mounts
                .lines()
                .any(|line| line.contains("/dev/pts") && line.contains("devpts"))
            {
                log::info!("devpts already mounted at /dev/pts");
                return Ok(());
            }
        }

        // Mount devpts
        let result = mount(
            Some("devpts"),
            "/dev/pts",
            Some("devpts"),
            MsFlags::empty(),
            Some("newinstance,ptmxmode=0666,mode=0620,gid=5"),
        );

        match result {
            Ok(_) => log::info!("Mounted devpts at /dev/pts"),
            Err(e) if e == nix::errno::Errno::EBUSY => {
                log::info!("devpts already mounted at /dev/pts (EBUSY)");
            }
            Err(e) => {
                log::warn!("Failed to mount devpts: {}, continuing anyway", e);
                // Don't fail here, as we might be in a restricted environment
            }
        }

        // Create /dev/ptmx symlink if it doesn't exist
        let dev_ptmx = Path::new("/dev/ptmx");
        let pts_ptmx = Path::new("/dev/pts/ptmx");

        if !dev_ptmx.exists() && pts_ptmx.exists() {
            log::info!("Creating /dev/ptmx symlink to /dev/pts/ptmx");
            if let Err(e) = std::os::unix::fs::symlink("/dev/pts/ptmx", dev_ptmx) {
                log::warn!("Failed to create /dev/ptmx symlink: {}", e);
            }
        }

        Ok(())
    }

    fn execute_with_pty(command: &str, argv: &[CString], envp: &[CString]) -> ContainerResult<()> {
        log::debug!("Executing with PTY");
        let pty = openpty(None, None)
            .map_err(|e| ContainerError::process_execution(format!("openpty failed: {e}")))?;

        unsafe {
            signal(Signal::SIGINT, SigHandler::Handler(handle_signal)).ok();
            signal(Signal::SIGTERM, SigHandler::Handler(handle_signal)).ok();
            signal(Signal::SIGQUIT, SigHandler::Handler(handle_signal)).ok();
        }

        match unsafe { fork()? } {
            ForkResult::Child => {
                // Child process
                drop(pty.master); // Close master in child

                // Create new session and make this the session leader
                if let Err(e) = setsid() {
                    log::error!("setsid failed: {}", e);
                }

                let slave_fd = pty.slave.as_raw_fd();

                // Set the slave as the controlling terminal
                unsafe {
                    // Make stdin, stdout, stderr point to the slave
                    libc::dup2(slave_fd, 0);
                    libc::dup2(slave_fd, 1);
                    libc::dup2(slave_fd, 2);

                    // Set controlling terminal
                    if libc::ioctl(0, libc::TIOCSCTTY, 0) < 0 {
                        log::warn!("TIOCSCTTY failed");
                    }
                }

                // Set process group
                if let Err(e) = tcsetpgrp(
                    unsafe { std::os::fd::BorrowedFd::borrow_raw(0) },
                    nix::unistd::getpid(),
                ) {
                    log::warn!("tcsetpgrp failed: {}", e);
                }
                // Close the slave after dup2
                drop(pty.slave);

                // Reset signal handlers to default
                unsafe {
                    signal(Signal::SIGINT, SigHandler::SigDfl).ok();
                    signal(Signal::SIGTERM, SigHandler::SigDfl).ok();
                    signal(Signal::SIGQUIT, SigHandler::SigDfl).ok();
                    signal(Signal::SIGTSTP, SigHandler::SigDfl).ok();
                    signal(Signal::SIGTTIN, SigHandler::SigDfl).ok();
                    signal(Signal::SIGTTOU, SigHandler::SigDfl).ok();
                }

                execve(&argv[0], argv, envp).map_err(|e| {
                    ContainerError::process_execution(format!("execve failed for {command}: {e}"))
                })?;
                unreachable!()
            }
            ForkResult::Parent { child } => {
                // Parent process
                CHILD_PID.store(child.as_raw(), Ordering::SeqCst);
                drop(pty.slave); // Close slave in parent

                log::info!("Container process PID: {child}");

                // Set master to non-blocking
                let master_fd = pty.master.as_raw_fd();
                unsafe {
                    let flags = libc::fcntl(master_fd, libc::F_GETFL, 0);
                    libc::fcntl(master_fd, libc::F_SETFL, flags | libc::O_NONBLOCK);
                }

                // Set raw mode on stdin
                Self::set_raw_mode(true);

                // Spawn thread to copy stdin to master
                let master_fd_in = unsafe { libc::dup(master_fd) };
                if master_fd_in < 0 {
                    Self::set_raw_mode(false);
                    return Err(ContainerError::ProcessExecution {
                        message: "Failed to duplicate master fd".to_string(),
                    });
                }
                let master_fd_owned = unsafe { OwnedFd::from_raw_fd(master_fd_in) };
                std::thread::spawn(move || {
                    let mut stdin = std::io::stdin();
                    let mut master_in: fs::File = master_fd_owned.into();
                    let mut buffer = [0u8; 1024];

                    loop {
                        match stdin.read(&mut buffer) {
                            Ok(0) => break, // EOF
                            Ok(n) => {
                                if master_in.write_all(&buffer[..n]).is_err() {
                                    break;
                                }
                            }
                            Err(_) => break,
                        }
                    }
                });

                // Main thread: copy master to stdout
                let mut master_out = fs::File::from(pty.master);
                let mut stdout = std::io::stdout();
                let mut buffer = [0u8; 4096];

                loop {
                    // Check if child is still alive
                    match waitpid(child, Some(WaitPidFlag::WNOHANG)) {
                        Ok(WaitStatus::Exited(_, status)) => {
                            // Read any remaining output
                            while let Ok(n) = master_out.read(&mut buffer) {
                                if n == 0 {
                                    break;
                                }
                                let _ = stdout.write_all(&buffer[..n]);
                            }
                            let _ = stdout.flush();

                            log::info!("Container exited with status: {status}");
                            CHILD_PID.store(0, Ordering::SeqCst);
                            Self::set_raw_mode(false);

                            if status != 0 {
                                return Err(ContainerError::process_execution(format!(
                                    "Container process exited with non-zero status: {status}"
                                )));
                            }
                            return Ok(());
                        }
                        Ok(WaitStatus::Signaled(_, sig, _)) => {
                            log::warn!("Container killed by signal: {sig}");
                            CHILD_PID.store(0, Ordering::SeqCst);
                            Self::set_raw_mode(false);
                            return Err(ContainerError::process_execution(format!(
                                "Container process killed by signal: {sig}"
                            )));
                        }
                        Ok(WaitStatus::StillAlive) => {
                            // Child still running, continue reading output
                        }
                        Ok(_) => continue,
                        Err(nix::errno::Errno::ECHILD) => {
                            // Child already exited
                            break;
                        }
                        Err(_) => continue,
                    }

                    // Read from master and write to stdout
                    match master_out.read(&mut buffer) {
                        Ok(0) => {
                            // EOF - wait for child to exit
                            std::thread::sleep(std::time::Duration::from_millis(10));
                        }
                        Ok(n) => {
                            let _ = stdout.write_all(&buffer[..n]);
                            let _ = stdout.flush();
                        }
                        Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                            // No data available, sleep briefly
                            std::thread::sleep(std::time::Duration::from_millis(10));
                        }
                        Err(_) => {
                            // Error reading, child probably exited
                            std::thread::sleep(std::time::Duration::from_millis(10));
                        }
                    }
                }

                // Final wait for child
                match waitpid(child, Some(WaitPidFlag::empty())) {
                    Ok(WaitStatus::Exited(_, status)) => {
                        log::info!("Container exited with status: {status}");
                        CHILD_PID.store(0, Ordering::SeqCst);
                        Self::set_raw_mode(false);
                        if status != 0 {
                            return Err(ContainerError::process_execution(format!(
                                "Container process exited with non-zero status: {status}"
                            )));
                        }
                    }
                    Ok(WaitStatus::Signaled(_, sig, _)) => {
                        log::warn!("Container killed by signal: {sig}");
                        CHILD_PID.store(0, Ordering::SeqCst);
                        Self::set_raw_mode(false);
                        return Err(ContainerError::process_execution(format!(
                            "Container process killed by signal: {sig}"
                        )));
                    }
                    _ => {}
                }

                Self::set_raw_mode(false);
                Ok(())
            }
        }
    }

    fn set_raw_mode(enable: bool) {
        use nix::sys::termios::{
            ControlFlags, InputFlags, LocalFlags, OutputFlags, SetArg, tcgetattr, tcsetattr,
        };

        let stdin = std::io::stdin();

        if enable {
            if let Ok(mut termios) = tcgetattr(&stdin) {
                // Set raw mode
                termios.local_flags &= !(LocalFlags::ICANON
                    | LocalFlags::ECHO
                    | LocalFlags::ISIG
                    | LocalFlags::IEXTEN);
                termios.input_flags &= !(InputFlags::IXON
                    | InputFlags::ICRNL
                    | InputFlags::BRKINT
                    | InputFlags::INPCK
                    | InputFlags::ISTRIP);
                termios.output_flags &= !OutputFlags::OPOST;
                termios.control_flags |= ControlFlags::CS8;

                let _ = tcsetattr(&stdin, SetArg::TCSANOW, &termios);
            }
        } else {
            // Restore to original settings or at least cooked mode
            if let Ok(mut termios) = tcgetattr(&stdin) {
                termios.local_flags |= LocalFlags::ICANON | LocalFlags::ECHO | LocalFlags::ISIG;
                termios.input_flags |= InputFlags::ICRNL;
                termios.output_flags |= OutputFlags::OPOST;

                let _ = tcsetattr(&stdin, SetArg::TCSANOW, &termios);
            }
        }
    }
    fn execute_without_pty(
        command: &str,
        argv: &[CString],
        envp: &[CString],
    ) -> ContainerResult<()> {
        log::debug!("Executing without PTY");
        unsafe {
            signal(Signal::SIGINT, SigHandler::Handler(handle_signal)).ok();
            signal(Signal::SIGTERM, SigHandler::Handler(handle_signal)).ok();
            signal(Signal::SIGQUIT, SigHandler::Handler(handle_signal)).ok();
        }

        match unsafe { fork()? } {
            ForkResult::Child => {
                let _ = setsid();

                unsafe {
                    signal(Signal::SIGINT, SigHandler::SigDfl).ok();
                    signal(Signal::SIGTERM, SigHandler::SigDfl).ok();
                    signal(Signal::SIGQUIT, SigHandler::SigDfl).ok();
                }

                execve(&argv[0], argv, envp).map_err(|e| {
                    ContainerError::process_execution(format!("execve failed for {command}: {e}"))
                })?;
                unreachable!()
            }
            ForkResult::Parent { child } => {
                CHILD_PID.store(child.as_raw(), Ordering::SeqCst);
                log::info!("Container process PID: {child}");

                Self::wait_for_child(child)?;
                CHILD_PID.store(0, Ordering::SeqCst);
                Ok(())
            }
        }
    }

    fn wait_for_child(child: Pid) -> ContainerResult<()> {
        loop {
            match waitpid(child, Some(WaitPidFlag::empty())) {
                Ok(WaitStatus::Exited(_, status)) => {
                    log::info!("Container exited with status: {status}");
                    if status != 0 {
                        return Err(ContainerError::process_execution(format!(
                            "Container process exited with non-zero status: {status}"
                        )));
                    }
                    break;
                }
                Ok(WaitStatus::Signaled(_, sig, _)) => {
                    log::warn!("Container killed by signal: {sig}");
                    return Err(ContainerError::process_execution(format!(
                        "Container process killed by signal: {sig}"
                    )));
                }
                Ok(_) => continue,
                Err(nix::errno::Errno::EINTR) => continue,
                Err(e) => {
                    return Err(ContainerError::process_execution(format!(
                        "waitpid failed: {e}"
                    )));
                }
            }
        }
        Ok(())
    }

    pub fn build_argv(command_path: &str, args: &[String]) -> ContainerResult<Vec<CString>> {
        let mut argv = vec![CString::new(command_path).unwrap()];
        for arg in args {
            argv.push(CString::new(arg.as_str()).unwrap());
        }
        Ok(argv)
    }

    pub fn build_environment() -> ContainerResult<Vec<CString>> {
        let envs = vec![
            "PATH=/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin",
            "TERM=xterm-256color",
            "HOME=/root",
            "HOSTNAME=rust-container",
            "container=rust-container-runtime",
        ];
        Ok(envs.iter().map(|s| CString::new(*s).unwrap()).collect())
    }
}
