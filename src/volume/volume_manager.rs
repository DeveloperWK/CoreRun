use crate::{
    error::{ContainerError, ContainerResult},
    volume::VolumeMount,
};
use std::fs;
pub struct VolumeManager;
impl VolumeManager {
    pub fn setup_volume(mount: &VolumeMount) -> ContainerResult<()> {
        if !mount.source.exists() {
            fs::create_dir_all(&mount.source)?;
        }
        if !mount.source.is_dir() {
            return Err(ContainerError::Volume {
                message: format!("Volume source must be a directory: {:?}", mount.source),
            });
        }
        Ok(())
    }
    pub fn cleanup_anonymous_volume(mount: &VolumeMount) -> ContainerResult<()> {
        if mount.source.to_string_lossy().contains("CoreRun/vol_") {
            if mount.source.exists() {
                fs::remove_dir_all(&mount.source)?;
            }
        }
        Ok(())
    }
}
