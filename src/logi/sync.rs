use tracing::{error, info};

use std::{
    fs::{self, Metadata},
    io,
    time::SystemTime,
};

use crate::{
    logi::{
        gp::Group,
        mem::Member,
        report::{ActionReport, MemberReport},
        vmem::{VirtualLeaf, VirtualMember},
    },
    phy::cab_conf::{CoverLevel, SaveLevel},
};

pub fn need_sync(src: &VirtualLeaf, dst_meta: &Metadata, strict: bool) -> bool {
    if strict {
        return true;
    }

    let src_meta = match fs::metadata(&src.file_abs_path) {
        Ok(m) => m,
        Err(_) => return false,
    };

    let src_size = src_meta.len();
    let dst_size = dst_meta.len();

    let src_mtime = src_meta.modified().unwrap_or(SystemTime::UNIX_EPOCH);
    let dst_mtime = dst_meta.modified().unwrap_or(SystemTime::UNIX_EPOCH);

    if src_size != dst_size || src_mtime > dst_mtime {
        true
    } else {
        false
    }
}

impl Group {
    pub fn sync_from_vmem(&self, vmem: &VirtualMember, strict: bool) -> ActionReport {
        let dsts = self
            .mems
            .iter()
            .filter(|mem| mem.mem_info.mem_conf.dst_option.enable);

        let mut report = ActionReport::default();
        dsts.for_each(|dst| {
            info!("run sync for dst {:?}", dst.mem_info.cab_info.abs_path);
            report.members.push(dst.sync_from_vmem(vmem, strict));
        });
        report
    }
}

impl Member {
    /// Files this member would receive from the virtual member.
    /// Read-only: the web UI uses it to preview an action before running it.
    pub fn plan_sync_from_vmem<'a>(
        &self,
        vmem: &'a VirtualMember,
        strict: bool,
    ) -> Vec<&'a VirtualLeaf> {
        let dst_root = self.mem_info.cab_info.abs_path.as_path();
        let priority = self.mem_info.mem_conf.priority;
        let mut plan: Vec<&VirtualLeaf> = Vec::new();

        for leaf in vmem.virtual_tree.values() {
            let target_abs_path = dst_root.join(leaf.file_rel_path.as_path());

            match fs::metadata(&target_abs_path) {
                Ok(t) => match CoverLevel::from(self.mem_info.mem_conf.dst_option.cover_level) {
                    CoverLevel::DontCover => (),
                    CoverLevel::HigherCover => {
                        if priority < leaf.priority && need_sync(leaf, &t, strict) {
                            plan.push(leaf);
                        }
                    }
                    _ => (),
                },
                Err(e) => {
                    if e.kind() != std::io::ErrorKind::NotFound {
                        error!("Failed to read meta {}: {}", target_abs_path.display(), e);
                        continue;
                    }
                    match SaveLevel::from(self.mem_info.mem_conf.dst_option.save_level) {
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

    pub fn sync_from_vmem(&self, vmem: &VirtualMember, strict: bool) -> MemberReport {
        let plan = self.plan_sync_from_vmem(vmem, strict);

        let mut report = MemberReport {
            cabinet_abs_path: self.mem_info.cab_info.abs_path.clone(),
            planned: plan.len(),
            ..Default::default()
        };

        for leaf in plan {
            match self.sync_from_leaf(leaf) {
                Ok(_) => report.done += 1,
                Err(e) => {
                    error!(
                        "Failed to copy file {:?} to {:?}: {}",
                        leaf.file_abs_path, self.mem_info.cab_info.abs_path, e
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

    fn sync_from_leaf(&self, leaf: &VirtualLeaf) -> io::Result<()> {
        let dst_root = &self.mem_info.cab_info.abs_path;
        let target_abs_path = dst_root.join(&leaf.file_rel_path);

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
        fs::copy(&leaf.file_abs_path, &target_abs_path)?;

        info!(
            "Synced file {:?} to {:?}",
            leaf.file_abs_path, target_abs_path
        );

        Ok(())
    }
}
