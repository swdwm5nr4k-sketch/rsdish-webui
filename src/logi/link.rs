use std::{fs, io, path::Path};

use std::time::{SystemTime, UNIX_EPOCH};

use tracing::{error, info};

use crate::{
    logi::{
        gp::Group,
        mem::Member,
        report::{ActionReport, MemberReport},
        vmem::{VirtualLeaf, VirtualMember},
    },
    phy::cab_conf::SaveLevel,
};

impl Group {
    pub fn link_from_vmem(&self, vmem: &VirtualMember) -> ActionReport {
        let link_dsts = self
            .mems
            .iter()
            .filter(|mem| mem.mem_info.mem_conf.link_option.enable);

        let mut report = ActionReport::default();
        link_dsts.for_each(|link_dst| {
            info!("linking dst {:?}", link_dst.mem_info.cab_info.abs_path);
            report.members.push(link_dst.link_from_vmem(vmem));
        });
        report
    }
}

impl Member {
    /// Files this member would receive as symlinks from the virtual member.
    /// Read-only: the web UI uses it to preview an action before running it.
    pub fn plan_link_from_vmem<'a>(&self, vmem: &'a VirtualMember) -> Vec<&'a VirtualLeaf> {
        let link_dst_root = self.mem_info.cab_info.abs_path.as_path();
        let priority = self.mem_info.mem_conf.priority;
        let mut plan: Vec<&VirtualLeaf> = Vec::new();

        for leaf in vmem.virtual_tree.values() {
            let target_abs_path = link_dst_root.join(leaf.file_rel_path.as_path());

            match fs::metadata(&target_abs_path) {
                Ok(_) => (),
                Err(e) => {
                    if e.kind() != std::io::ErrorKind::NotFound {
                        error!("Failed to read meta {}: {}", target_abs_path.display(), e);
                        continue;
                    }
                    match SaveLevel::from(self.mem_info.mem_conf.link_option.save_level) {
                        SaveLevel::DontSave => (),
                        SaveLevel::SaveHigher => {
                            if priority < leaf.priority {
                                plan.push(leaf);
                            }
                        }
                        SaveLevel::SaveHigherEqual => {
                            if priority <= leaf.priority {
                                plan.push(leaf);
                            }
                        }
                        SaveLevel::SaveAll => plan.push(leaf),
                        _ => (),
                    }
                }
            };
        }

        plan
    }

    pub fn link_from_vmem(&self, vmem: &VirtualMember) -> MemberReport {
        let plan = self.plan_link_from_vmem(vmem);

        let mut report = MemberReport {
            cabinet_abs_path: self.mem_info.cab_info.abs_path.clone(),
            planned: plan.len(),
            ..Default::default()
        };

        for leaf in plan {
            match self.link_from_leaf(leaf) {
                Ok(_) => report.done += 1,
                Err(e) => {
                    error!(
                        "Failed to link from original {} to link {}: {}",
                        leaf.file_abs_path.display(),
                        self.mem_info
                            .cab_info
                            .abs_path
                            .join(&leaf.file_rel_path)
                            .display(),
                        e
                    );
                    report.failures.push(format!(
                        "{}: {}",
                        leaf.file_abs_path.display(),
                        e
                    ));
                }
            }
        }

        report
    }

    fn link_from_leaf(&self, leaf: &VirtualLeaf) -> io::Result<()> {
        let link_dst_root = &self.mem_info.cab_info.abs_path;
        let target_abs_path = link_dst_root.join(&leaf.file_rel_path);

        let target_folder = match target_abs_path.parent() {
            Some(p) => p,
            None => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("Failed to get parent of {:?}", target_abs_path),
                ));
            }
        };

        fs::create_dir_all(target_folder)?;

        create_symlink(leaf.file_abs_path.as_path(), target_abs_path.as_path())?;

        info!(
            "Linked file {:?} to {:?}",
            leaf.file_abs_path, target_abs_path
        );

        Ok(())
    }
}

pub fn create_symlink(original: &Path, link: &Path) -> io::Result<()> {
    // 1. Generate a pseudo-random suffix using timestamp + process ID
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis();
    let pid = std::process::id();

    // Construct a temporary link name like "my_link_1234_1698324000000"
    let tmp_link = link.with_file_name(format!(
        "{}_{}_{}",
        link.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("link"),
        pid,
        timestamp
    ));

    // 2. Create the temporary symbolic link
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(original, &tmp_link)?;
    }

    #[cfg(windows)]
    {
        if original.is_dir() {
            std::os::windows::fs::symlink_dir(original, &tmp_link)?;
        } else {
            std::os::windows::fs::symlink_file(original, &tmp_link)?;
        }
    }

    // 3. Remove the existing link if it already exists
    if link.exists() {
        let metadata = fs::symlink_metadata(link)?;
        if metadata.file_type().is_symlink() {
            fs::remove_file(link)?;
        } else if metadata.is_dir() {
            fs::remove_dir_all(link)?;
        }
    }

    // 4. Atomically rename the temporary symlink to the final path
    fs::rename(&tmp_link, link)?;

    Ok(())
}
