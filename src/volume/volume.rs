use std::{fmt::format, path::PathBuf};

use crate::error::{ContainerError, ContainerResult};
#[derive(Debug, Clone)]
pub struct VolumeMount {
    pub source: PathBuf,
    pub dest: PathBuf,
    pub mode: MountMode,
}

#[derive(Debug, Clone)]
pub enum MountMode {
    ReadWrite,
    ReadOnly,
}

impl VolumeMount {
    pub fn parse(volume_str: &str) -> ContainerResult<Self> {
        let parts: Vec<&str> = volume_str.split(":").collect();
        match parts.len() {
            1 => {
                let dest = PathBuf::from(parts[0]);
                if !dest.is_absolute() {
                    return Err(ContainerError::Volume {
                        message: format!("Container path must be absolute: {}", parts[0]),
                    });
                }
                Ok(VolumeMount {
                    source: Self::create_anonymous_volume()?,
                    dest,
                    mode: MountMode::ReadWrite,
                })
            }
            2 => {
                let source = PathBuf::from(parts[0]);
                let dest = PathBuf::from(parts[1]);
                if !dest.is_absolute() {
                    return Err(ContainerError::Volume {
                        message: format!("Container path must be absolute: {}", parts[1]),
                    });
                }
                Ok(VolumeMount {
                    source,
                    dest,
                    mode: MountMode::ReadWrite,
                })
            }
            3 => {
                let source = PathBuf::from(parts[0]);
                let dest = PathBuf::from(parts[1]);
                let mode = match parts[2] {
                    "ro" => MountMode::ReadOnly,
                    "rw" => MountMode::ReadWrite,
                    other => {
                        return Err(crate::error::ContainerError::InvalidConfiguration {
                            message: format!("Invalid mount mode: {}", other),
                        });
                    }
                };
                if !dest.is_absolute() {
                    return Err(ContainerError::Volume {
                        message: format!("Container path must be absolute: {}", parts[2]),
                    });
                }
                Ok(VolumeMount { source, dest, mode })
            }
            _ => Err(ContainerError::InvalidConfiguration {
                message: "Invalid mount format".to_string(),
            }), // Change to volume error
        }
    }
    fn create_anonymous_volume() -> ContainerResult<PathBuf> {
        let temp_dir = std::env::temp_dir()
            .join("CoreRun")
            .join(format!("vol_{}", uuid::Uuid::new_v4()));
        let _ = std::fs::create_dir_all(&temp_dir)?;
        Ok(temp_dir)
    }
}
