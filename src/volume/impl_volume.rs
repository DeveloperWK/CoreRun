use nix::mount::{self, MntFlags, MsFlags, mount, umount2};

use crate::{
    cli::ContainerConfig,
    error::{ContainerError, ContainerResult},
    volume::{self, MountMode, VolumeManager, VolumeMount, impl_volume},
};
use std::{fs, path::Path};
pub struct ImplVolume {
    volumes: Vec<VolumeMount>,
}
impl ImplVolume {
    pub fn setup_volumes(volumes: Vec<String>, rootfs: &Path) -> ContainerResult<Self> {
        let volume_mounts = volumes
            .into_iter()
            .map(|v| VolumeMount::parse(&v))
            .collect::<ContainerResult<Vec<_>>>()?;
        for mount in &volume_mounts {
            VolumeManager::setup_volume(mount)?;
        }
        let impl_volume = ImplVolume {
            volumes: volume_mounts,
        };
        impl_volume.setup_fs(rootfs);
        Ok(impl_volume)
    }
    fn setup_fs(&self, rootfs: &Path) -> ContainerResult<()> {
        for volume in &self.volumes {
            self.mount_volume(volume, rootfs)?;
        }
        Ok(())
    }
    fn mount_volume(&self, volume: &VolumeMount, rootfs: &Path) -> ContainerResult<()> {
        let container_dest = &rootfs.join(volume.dest.strip_prefix("/").unwrap_or(&volume.dest));
        if !container_dest.exists() {
            fs::create_dir_all(&container_dest)?;
        }

        mount(
            Some(&volume.source),
            container_dest,
            None::<&str>,
            MsFlags::MS_BIND | MsFlags::MS_REC,
            None::<&str>,
        )?;
        if matches!(volume.mode, MountMode::ReadOnly) {
            mount(
                Some(container_dest),
                container_dest,
                None::<&str>,
                MsFlags::MS_BIND | MsFlags::MS_REMOUNT | MsFlags::MS_RDONLY,
                None::<&str>,
            )?;
        }
        Ok(())
    }
    pub fn cleanup_volume(&self, rootfs: &Path) -> ContainerResult<()> {
        for volume in self.volumes.iter().rev() {
            if let Err(e) = self.unmount_volume(volume, &rootfs) {
                log::warn!("Failed to unmount {:?}: {}", volume.dest, e);
            }
            std::thread::sleep(std::time::Duration::from_millis(100));
            VolumeManager::cleanup_anonymous_volume(volume)?;
        }
        Ok(())
    }
    fn unmount_volume(&self, volume: &VolumeMount, rootfs: &Path) -> ContainerResult<()> {
        let container_dest = &rootfs.join(volume.dest.strip_prefix("/").unwrap_or(&volume.dest));
        if container_dest.exists() {
            umount2(container_dest, MntFlags::MNT_DETACH)?;
        }
        Ok(())
    }
}
