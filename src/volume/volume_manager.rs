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
        println!(" cleanup_anonymous_volume");
        if mount.is_anonymous && mount.source.exists() {
            match fs::remove_dir_all(&mount.source) {
                Ok(_) => {
                    log::info!("Cleaned up anonymous volume: {:?}", mount.source);
                }
                Err(e) => {
                    log::warn!("Failed to cleanup: {}. May still be mounted.", e);
                }
            }
        }

        Ok(())
    }
}
