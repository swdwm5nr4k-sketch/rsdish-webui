//! Tests for the plan/execute split of the group actions.

use std::fs;
use std::path::Path;

use tempfile::tempdir;

use crate::logi::gp::{Group, GroupInfo};
use crate::logi::mem::{Member, MemberInfo};
use crate::logi::vmem::build_virtual_member_from_group;
use crate::phy::cab::CabinetInfo;
use crate::phy::cab_conf::{
    CabinetConfig, CoverLevel, DstOption, LinkOption, MemberConfig, SaveLevel, SrcOption,
};
use crate::phy::stg::StorageInfo;

const G1: &str = "0199ebad-44ad-78a2-baad-c56a052e33ac";

/// Build a member the way the scanner would, but from plain directories.
#[allow(clippy::too_many_arguments)]
fn make_member(
    cabinet: &Path,
    storage: &Path,
    priority: i32,
    src: bool,
    dst: bool,
    link: bool,
    dst_cover: CoverLevel,
    dst_save: SaveLevel,
    link_save: SaveLevel,
) -> Member {
    let mem_conf = MemberConfig {
        group_uuid: G1.to_string(),
        priority,
        src_option: SrcOption { enable: src },
        dst_option: DstOption {
            enable: dst,
            cover_level: dst_cover as i32,
            save_level: dst_save as i32,
        },
        link_option: LinkOption {
            enable: link,
            save_level: link_save as i32,
        },
    };

    Member {
        mem_info: MemberInfo {
            cab_info: CabinetInfo {
                abs_path: cabinet.to_path_buf(),
                conf_abs_path: cabinet.join(env!("CABINET_CONFIG_NAME")),
                cab_conf: CabinetConfig {
                    memberships: vec![mem_conf.clone()],
                },
            },
            mem_conf,
            stg_info: StorageInfo {
                abs_path: storage.to_path_buf(),
            },
        },
    }
}

fn make_group(mems: Vec<Member>) -> Group {
    Group {
        gp_info: GroupInfo {
            gp_uuid: G1.to_string(),
        },
        mems,
    }
}

fn src_member(dir: &Path, storage: &Path, priority: i32) -> Member {
    make_member(
        dir,
        storage,
        priority,
        true,
        false,
        false,
        CoverLevel::DontCover,
        SaveLevel::DontSave,
        SaveLevel::DontSave,
    )
}

#[test]
fn sync_plan_copies_missing_files_and_skips_unchanged_ones() {
    let tmp = tempdir().unwrap();
    let storage = tmp.path();
    let src_dir = storage.join("src");
    let dst_dir = storage.join("dst");

    fs::create_dir_all(src_dir.join("sub")).unwrap();
    fs::create_dir_all(&dst_dir).unwrap();
    fs::write(src_dir.join("a.txt"), b"hello").unwrap();
    fs::write(src_dir.join("sub").join("b.txt"), b"world").unwrap();

    let group = make_group(vec![
        src_member(&src_dir, storage, 1),
        make_member(
            &dst_dir,
            storage,
            0,
            false,
            true,
            false,
            CoverLevel::HigherCover,
            SaveLevel::SaveAll,
            SaveLevel::DontSave,
        ),
    ]);

    let vmem = build_virtual_member_from_group(&group);
    assert_eq!(vmem.virtual_tree.len(), 2);
    assert_eq!(vmem.highest_priority, 1);

    let dst = &group.mems[1];
    assert_eq!(dst.plan_sync_from_vmem(&vmem, false).len(), 2);

    let report = dst.sync_from_vmem(&vmem, false);
    assert_eq!(report.planned, 2);
    assert_eq!(report.done, 2);
    assert!(report.failures.is_empty());
    assert_eq!(fs::read_to_string(dst_dir.join("a.txt")).unwrap(), "hello");
    assert_eq!(
        fs::read_to_string(dst_dir.join("sub").join("b.txt")).unwrap(),
        "world"
    );

    // nothing changed in the meantime: an incremental run has nothing to do
    // while a strict run copies everything again
    assert!(dst.plan_sync_from_vmem(&vmem, false).is_empty());
    assert_eq!(dst.plan_sync_from_vmem(&vmem, true).len(), 2);
}

#[test]
fn clean_plan_targets_files_from_lower_priority_sources() {
    let tmp = tempdir().unwrap();
    let storage = tmp.path();
    let top_dir = storage.join("top_src");
    let low_dir = storage.join("low_src");
    let dst_dir = storage.join("dst");

    for dir in [&top_dir, &low_dir, &dst_dir] {
        fs::create_dir_all(dir).unwrap();
    }
    fs::write(top_dir.join("top.txt"), b"top").unwrap();
    fs::write(low_dir.join("low.txt"), b"low").unwrap();

    let group = make_group(vec![
        src_member(&top_dir, storage, 3),
        src_member(&low_dir, storage, 1),
        make_member(
            &dst_dir,
            storage,
            0,
            false,
            true,
            false,
            CoverLevel::HigherCover,
            SaveLevel::SaveAll,
            SaveLevel::DontSave,
        ),
    ]);

    let vmem = build_virtual_member_from_group(&group);
    assert_eq!(vmem.highest_priority, 3);

    let dst = &group.mems[2];
    let sync_report = dst.sync_from_vmem(&vmem, false);
    assert_eq!(sync_report.done, 2);

    // top.txt comes from the highest priority source and stays,
    // low.txt comes from a lower one and is cleaned
    let plan = dst.plan_clean_from_vmem(&vmem);
    assert_eq!(plan.len(), 1);
    assert!(plan[0].ends_with("low.txt"));

    let clean_report = dst.clean_from_vmem(&vmem);
    assert_eq!(clean_report.planned, 1);
    assert_eq!(clean_report.done, 1);
    assert!(!dst_dir.join("low.txt").exists());
    assert!(dst_dir.join("top.txt").exists());
}

#[cfg(unix)]
#[test]
fn link_plan_creates_symlinks_in_the_target_cabinet() {
    let tmp = tempdir().unwrap();
    let storage = tmp.path();
    let src_dir = storage.join("src");
    let gate_dir = storage.join("gate");

    fs::create_dir_all(&src_dir).unwrap();
    fs::create_dir_all(&gate_dir).unwrap();
    fs::write(src_dir.join("a.txt"), b"hello").unwrap();

    let group = make_group(vec![
        src_member(&src_dir, storage, 1),
        make_member(
            &gate_dir,
            storage,
            0,
            false,
            false,
            true,
            CoverLevel::DontCover,
            SaveLevel::DontSave,
            SaveLevel::SaveAll,
        ),
    ]);

    let vmem = build_virtual_member_from_group(&group);
    let gate = &group.mems[1];

    assert_eq!(gate.plan_link_from_vmem(&vmem).len(), 1);

    let report = gate.link_from_vmem(&vmem);
    assert_eq!(report.planned, 1);
    assert_eq!(report.done, 1);

    let link_meta = fs::symlink_metadata(gate_dir.join("a.txt")).unwrap();
    assert!(link_meta.file_type().is_symlink());
    assert_eq!(fs::read_to_string(gate_dir.join("a.txt")).unwrap(), "hello");
}

#[cfg(unix)]
#[test]
fn exec_returns_one_outcome_per_member() {
    let tmp = tempdir().unwrap();
    let storage = tmp.path();
    let dir = storage.join("cab");

    fs::create_dir_all(&dir).unwrap();

    let group = make_group(vec![make_member(
        &dir,
        storage,
        0,
        false,
        false,
        false,
        CoverLevel::DontCover,
        SaveLevel::DontSave,
        SaveLevel::DontSave,
    )]);

    let outcomes = group.exec(&"echo from-cabinet".to_string());
    assert_eq!(outcomes.len(), 1);
    assert!(outcomes[0].success);
    assert!(outcomes[0].stdout.contains("from-cabinet"));
    assert!(outcomes[0].error.is_none());
}
