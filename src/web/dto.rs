//! JSON shapes served by the web API. Everything the browser sees is built
//! here, so the internal library types can keep evolving freely.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::logi::exec::ExecOutcome;
use crate::logi::gp::Group;
use crate::logi::report::ActionReport;
use crate::logi::vmem::VirtualMember;
use crate::phy::cab::Cabinet;
use crate::phy::cab_conf::{CabinetConfig, MemberConfig};
use crate::phy::dk::disks;
use crate::phy::stg::{Storage, scan_storages_with_issues};
use crate::user::user_conf::{user_conf, user_conf_path};
use crate::web::logs::LogLine;

/// How many file paths a plan may list per action.
pub const SAMPLE_LIMIT: usize = 200;
/// How many directories the browser may list at once.
pub const BROWSE_LIMIT: usize = 500;

fn path_str(path: &Path) -> String {
    path.to_string_lossy().to_string()
}

fn rel_str(root: &Path, path: &Path) -> String {
    match path.strip_prefix(root) {
        Ok(rel) => path_str(rel),
        Err(_) => path_str(path),
    }
}

#[derive(Debug, Serialize)]
pub struct FileListDto {
    pub total: usize,
    pub truncated: bool,
    pub files: Vec<String>,
}

fn file_list<I>(items: I) -> FileListDto
where
    I: IntoIterator<Item = String>,
{
    let mut files: Vec<String> = Vec::new();
    let mut total = 0usize;

    for item in items {
        total += 1;
        if files.len() < SAMPLE_LIMIT {
            files.push(item);
        }
    }

    FileListDto {
        total,
        truncated: total > files.len(),
        files,
    }
}

#[derive(Debug, Serialize)]
pub struct CabinetDto {
    pub abs_path: String,
    pub rel_path: String,
    pub conf_path: String,
    pub memberships: Vec<MemberConfig>,
}

#[derive(Debug, Serialize)]
pub struct StorageDto {
    pub abs_path: String,
    pub from_disk_list: bool,
    pub cabinet_count: usize,
    pub cabinets: Vec<CabinetDto>,
}

#[derive(Debug, Serialize)]
pub struct IssueDto {
    pub path: String,
    pub message: String,
}

#[derive(Debug, Serialize)]
pub struct GroupMemberDto {
    pub cabinet_abs_path: String,
    pub cabinet_rel_path: String,
    pub storage_abs_path: String,
    pub priority: i32,
    pub src_enabled: bool,
    pub dst_enabled: bool,
    pub link_enabled: bool,
    pub role: String,
}

#[derive(Debug, Serialize)]
pub struct GroupDto {
    pub uuid: String,
    pub max_priority: i32,
    pub members: Vec<GroupMemberDto>,
}

#[derive(Debug, Serialize)]
pub struct ConfigDto {
    pub config_path: String,
    pub custom_storages: Vec<String>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct OverviewDto {
    pub version: String,
    pub config: ConfigDto,
    pub storages: Vec<StorageDto>,
    pub groups: Vec<GroupDto>,
    pub issues: Vec<IssueDto>,
    pub scan_paths: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct MemberPlanDto {
    pub cabinet_abs_path: String,
    pub storage_abs_path: String,
    pub priority: i32,
    pub role: String,
    pub src_enabled: bool,
    pub dst_enabled: bool,
    pub link_enabled: bool,
    pub copies: FileListDto,
    pub links: FileListDto,
    pub cleans: FileListDto,
}

#[derive(Debug, Serialize)]
pub struct PlanDto {
    pub group_uuid: String,
    pub strict: bool,
    pub highest_priority: i32,
    pub files: FileListDto,
    pub members: Vec<MemberPlanDto>,
}

#[derive(Debug, Serialize)]
pub struct MemberActionDto {
    pub cabinet_abs_path: String,
    pub planned: usize,
    pub done: usize,
    pub failures: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct ActionDto {
    pub action: String,
    pub group_uuid: String,
    pub planned: usize,
    pub done: usize,
    pub failed: usize,
    pub members: Vec<MemberActionDto>,
}

#[derive(Debug, Serialize)]
pub struct ExecMemberDto {
    pub cabinet_abs_path: String,
    pub success: bool,
    pub code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
    pub error: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ExecDto {
    pub group_uuid: String,
    pub success: bool,
    pub members: Vec<ExecMemberDto>,
}

#[derive(Debug, Serialize)]
pub struct LogsDto {
    pub lines: Vec<LogLine>,
    pub next: u64,
}

#[derive(Debug, Serialize)]
pub struct BrowseEntryDto {
    pub name: String,
    pub path: String,
    pub group_uuids: Vec<String>,
    pub has_cabinet_config: bool,
}

#[derive(Debug, Serialize)]
pub struct BrowseDto {
    pub path: String,
    pub parent: Option<String>,
    pub entries: Vec<BrowseEntryDto>,
    pub truncated: bool,
}

/// Bundle a cabinet with the display helpers the UI needs.
pub fn cabinet_dto(storage_root: &Path, cab: &Cabinet) -> CabinetDto {
    CabinetDto {
        abs_path: path_str(&cab.cab_info.abs_path),
        rel_path: rel_str(storage_root, &cab.cab_info.abs_path),
        conf_path: path_str(&cab.cab_info.conf_abs_path),
        memberships: cab.cab_info.cab_conf.memberships.clone(),
    }
}

pub fn storage_dto(stg: &Storage, from_disk_list: bool) -> StorageDto {
    let root = stg.stg_info.abs_path.as_path();

    StorageDto {
        abs_path: path_str(root),
        from_disk_list,
        cabinet_count: stg.cabs.len(),
        cabinets: stg.cabs.iter().map(|cab| cabinet_dto(root, cab)).collect(),
    }
}

/// Which preset a membership looks like, only used for display.
fn member_role(mem: &MemberConfig) -> &'static str {
    match (
        mem.src_option.enable,
        mem.dst_option.enable,
        mem.link_option.enable,
    ) {
        (true, true, false) => "main",
        (true, false, false) => "src",
        (false, true, false) => "dst",
        (false, false, true) => "gate",
        (false, false, false) => "idle",
        _ => "mixed",
    }
}

pub fn group_member_dto(mem: &crate::logi::mem::Member) -> GroupMemberDto {
    let info = &mem.mem_info;
    let storage_root = info.stg_info.abs_path.as_path();
    let cabinet_path = info.cab_info.abs_path.as_path();

    GroupMemberDto {
        cabinet_abs_path: path_str(cabinet_path),
        cabinet_rel_path: rel_str(storage_root, cabinet_path),
        storage_abs_path: path_str(storage_root),
        priority: info.mem_conf.priority,
        src_enabled: info.mem_conf.src_option.enable,
        dst_enabled: info.mem_conf.dst_option.enable,
        link_enabled: info.mem_conf.link_option.enable,
        role: member_role(&info.mem_conf).to_string(),
    }
}

pub fn group_dto(gp: &Group) -> GroupDto {
    let max_priority = gp
        .mems
        .iter()
        .map(|mem| mem.mem_info.mem_conf.priority)
        .max()
        .unwrap_or(0);

    GroupDto {
        uuid: gp.gp_info.gp_uuid.clone(),
        max_priority,
        members: gp.mems.iter().map(group_member_dto).collect(),
    }
}

/// Config as seen by the UI, with a warning per unusable custom storage.
pub fn config_dto(custom_storages: Vec<String>) -> ConfigDto {
    let warnings = custom_storages
        .iter()
        .filter(|raw| !PathBuf::from(raw).is_dir())
        .map(|raw| format!("{} is not an existing directory", raw))
        .collect();

    let config_path = match user_conf_path() {
        Ok(path) => path_str(&path),
        Err(e) => format!("unavailable ({})", e),
    };

    ConfigDto {
        config_path,
        custom_storages,
        warnings,
    }
}

pub fn issue_dto(path: &Path, message: &str) -> IssueDto {
    IssueDto {
        path: path_str(path),
        message: message.to_string(),
    }
}

/// Full scan of every storage root, as shown on the dashboard.
pub fn overview_dto() -> OverviewDto {
    let conf = user_conf();
    let disk_paths = disks();

    let mut scan_paths: Vec<PathBuf> = disk_paths.clone();
    let mut custom_storages: Vec<String> = Vec::new();

    for raw in &conf.custom_storages {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            continue;
        }
        custom_storages.push(trimmed.to_string());

        let path = PathBuf::from(trimmed);
        if !scan_paths.contains(&path) {
            scan_paths.push(path);
        }
    }

    let (stgs, issues) = scan_storages_with_issues(&scan_paths);

    OverviewDto {
        version: env!("CARGO_PKG_VERSION").to_string(),
        config: config_dto(custom_storages),
        storages: stgs
            .iter()
            .map(|stg| storage_dto(stg, disk_paths.contains(&stg.stg_info.abs_path)))
            .collect(),
        groups: groups_dto(&stgs),
        issues: issues
            .iter()
            .map(|issue| issue_dto(&issue.path, &issue.message))
            .collect(),
        scan_paths: scan_paths.iter().map(|p| path_str(p)).collect(),
    }
}

pub fn groups_dto(stgs: &[Storage]) -> Vec<GroupDto> {
    crate::logi::gp::build_group_map_from_storages(stgs)
        .into_values()
        .map(|gp| group_dto(&gp))
        .collect()
}

/// What a strict or incremental run would do, without touching any file.
pub fn plan_dto(gp: &Group, vmem: &VirtualMember, strict: bool) -> PlanDto {
    let members = gp
        .mems
        .iter()
        .map(|mem| {
            let info = &mem.mem_info;
            let conf = &info.mem_conf;
            let cabinet_root = info.cab_info.abs_path.as_path();

            let copies = if conf.dst_option.enable {
                file_list(
                    mem.plan_sync_from_vmem(vmem, strict)
                        .into_iter()
                        .map(|leaf| path_str(&leaf.file_rel_path)),
                )
            } else {
                file_list(Vec::new())
            };

            let links = if conf.link_option.enable {
                file_list(
                    mem.plan_link_from_vmem(vmem)
                        .into_iter()
                        .map(|leaf| path_str(&leaf.file_rel_path)),
                )
            } else {
                file_list(Vec::new())
            };

            let cleans = if conf.dst_option.enable {
                file_list(
                    mem.plan_clean_from_vmem(vmem)
                        .into_iter()
                        .map(|path| rel_str(cabinet_root, &path)),
                )
            } else {
                file_list(Vec::new())
            };

            MemberPlanDto {
                cabinet_abs_path: path_str(cabinet_root),
                storage_abs_path: path_str(info.stg_info.abs_path.as_path()),
                priority: conf.priority,
                role: member_role(conf).to_string(),
                src_enabled: conf.src_option.enable,
                dst_enabled: conf.dst_option.enable,
                link_enabled: conf.link_option.enable,
                copies,
                links,
                cleans,
            }
        })
        .collect();

    PlanDto {
        group_uuid: gp.gp_info.gp_uuid.clone(),
        strict,
        highest_priority: vmem.highest_priority,
        files: file_list(
            vmem.virtual_tree
                .keys()
                .map(|path| path_str(path))
                .collect::<Vec<String>>(),
        ),
        members,
    }
}

pub fn action_dto(action: &str, group_uuid: &str, report: &ActionReport) -> ActionDto {
    ActionDto {
        action: action.to_string(),
        group_uuid: group_uuid.to_string(),
        planned: report.planned(),
        done: report.done(),
        failed: report.failures().len(),
        members: report
            .members
            .iter()
            .map(|mem| MemberActionDto {
                cabinet_abs_path: path_str(&mem.cabinet_abs_path),
                planned: mem.planned,
                done: mem.done,
                failures: mem.failures.clone(),
            })
            .collect(),
    }
}

pub fn exec_dto(group_uuid: &str, outcomes: &[ExecOutcome]) -> ExecDto {
    ExecDto {
        group_uuid: group_uuid.to_string(),
        success: outcomes.iter().all(|out| out.success),
        members: outcomes
            .iter()
            .map(|out| ExecMemberDto {
                cabinet_abs_path: path_str(&out.cabinet_abs_path),
                success: out.success,
                code: out.code,
                stdout: out.stdout.clone(),
                stderr: out.stderr.clone(),
                error: out.error.clone(),
            })
            .collect(),
    }
}

/// Default directory the browser opens on.
pub fn default_browse_root() -> PathBuf {
    for var in ["HOME", "USERPROFILE"] {
        let Ok(value) = std::env::var(var) else {
            continue;
        };

        if !value.trim().is_empty() {
            return PathBuf::from(value);
        }
    }

    PathBuf::from(std::path::MAIN_SEPARATOR.to_string())
}

/// Directories only, so the UI can pick a cabinet or a storage root.
pub fn browse_dto(path: Option<&str>) -> io::Result<BrowseDto> {
    let dir = match path {
        Some(raw) if !raw.trim().is_empty() => PathBuf::from(raw.trim()),
        _ => default_browse_root(),
    };

    let dir = if dir.is_dir() {
        dir
    } else {
        match dir.parent() {
            Some(parent) if parent.is_dir() => parent.to_path_buf(),
            _ => default_browse_root(),
        }
    };

    let mut entries: Vec<BrowseEntryDto> = Vec::new();
    let mut truncated = false;

    for entry in fs::read_dir(&dir)? {
        let entry = match entry {
            Ok(entry) => entry,
            Err(_) => continue,
        };

        let entry_path = entry.path();
        if !entry_path.is_dir() {
            continue;
        }

        if entries.len() >= BROWSE_LIMIT {
            truncated = true;
            break;
        }

        let (group_uuids, has_cabinet_config) = read_cabinet_memberships(&entry_path);

        entries.push(BrowseEntryDto {
            name: entry.file_name().to_string_lossy().to_string(),
            path: path_str(&entry_path),
            group_uuids,
            has_cabinet_config,
        });
    }

    entries.sort_by_key(|entry| entry.name.to_lowercase());

    Ok(BrowseDto {
        path: path_str(&dir),
        parent: dir.parent().map(path_str),
        entries,
        truncated,
    })
}

fn read_cabinet_memberships(dir: &Path) -> (Vec<String>, bool) {
    let conf_path = dir.join(env!("CABINET_CONFIG_NAME"));
    if !conf_path.is_file() {
        return (Vec::new(), false);
    }

    match fs::read_to_string(&conf_path)
        .ok()
        .and_then(|raw| toml::from_str::<CabinetConfig>(&raw).ok())
    {
        Some(conf) => (
            conf.memberships
                .iter()
                .map(|mem| mem.group_uuid.clone())
                .collect(),
            true,
        ),
        None => (Vec::new(), true),
    }
}
