use thiserror::Error;

use std::{
    env, fs, io,
    path::{Path, PathBuf},
};

use crate::phy::cab_conf::{CabinetConfig, CabinetConfigError};

#[derive(Debug, Error)]
pub enum CabinetError {
    #[error(transparent)]
    Io(#[from] io::Error),

    #[error(transparent)]
    TomlDe(#[from] toml::de::Error),

    #[error(transparent)]
    TomlSer(#[from] toml::ser::Error),
    
    #[error(transparent)]
    Config(#[from] CabinetConfigError),
}

#[derive(Debug)]
pub struct Cabinet {
    pub cab_info: CabinetInfo,
    // relations
}

#[derive(Debug, Clone)]
pub struct CabinetInfo {
    pub abs_path: PathBuf,
    pub conf_abs_path: PathBuf,
    pub cab_conf: CabinetConfig,
}

pub fn build_cabinet_from_path(cab_abs_path: &Path) -> Result<Cabinet, CabinetError> {
    let cab_conf_abs_path = cab_abs_path.join(env!("CABINET_CONFIG_NAME"));
    let cab_conf_str = fs::read_to_string(cab_conf_abs_path.as_path())?;
    let cab_conf: CabinetConfig = toml::from_str(&cab_conf_str)?;

    cab_conf.verify()?;

    Ok(Cabinet {
        cab_info: CabinetInfo {
            abs_path: cab_abs_path.to_path_buf(),
            conf_abs_path: cab_conf_abs_path,
            cab_conf: cab_conf,
        },
        // stg: Weak::new(),
    })
}

#[cfg(test)]
mod tests {
    use crate::phy::cab_conf::{default_cabinet_config, default_membership};

    use super::*;
    use std::fs;
    use tempfile::tempdir;

    const CABINET_CONFIG_NAME: &str = "rsdish.cabinet.toml";

    #[test]
    fn test_build_cabinet_from_path() -> Result<(), CabinetError> {
        let tmp_dir = tempdir().unwrap();
        let tmp_path = tmp_dir.path();

        // a cabinet config is only meaningful with a membership in it
        let mut test_config = default_cabinet_config();
        test_config.memberships.push(default_membership());

        let toml_str = toml::to_string(&test_config).unwrap();
        let cab_file_path = tmp_path.join(CABINET_CONFIG_NAME);
        fs::write(&cab_file_path, toml_str).unwrap();

        let cabinet = build_cabinet_from_path(tmp_path)?;

        assert_eq!(
            cabinet.cab_info.cab_conf.memberships.len(),
            test_config.memberships.len()
        );
        assert_eq!(
            cabinet.cab_info.cab_conf.memberships[0].group_uuid,
            test_config.memberships[0].group_uuid
        );
        assert_eq!(cabinet.cab_info.abs_path, tmp_path);
        assert_eq!(cabinet.cab_info.conf_abs_path, cab_file_path);

        Ok(())
    }
}

pub fn write_cabinet(cab: &Cabinet) -> Result<(), CabinetError> {
    let toml_str = toml::to_string(&cab.cab_info.cab_conf)?;
    fs::write(&cab.cab_info.conf_abs_path, toml_str)?;
    Ok(())
}