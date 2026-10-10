//! HTTP handlers. Every handler is a thin wrapper around the library: all the
//! real work happens in the same functions the CLI calls.

use std::fs;
use std::path::PathBuf;
use std::sync::Arc;

use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::Deserialize;
use serde_json::json;
use tracing::warn;

use crate::logi::gp::{Group, build_group_map_from_storages};
use crate::logi::vmem::build_virtual_member_from_group;
use crate::phy::cab::{Cabinet, build_cabinet_from_path, write_cabinet};
use crate::phy::cab_conf::{
    CabinetConfig, MemberConfig, default_cabinet_config, default_membership,
};
use crate::phy::stg::scan_storages;
use crate::user::user_conf::UserConfig;
use crate::web::dto;
use crate::web::logs as log_store;
use crate::web::state::AppState;

pub type ApiResult<T> = Result<T, ApiError>;

/// A JSON error, always shaped as { "error": "..." }.
#[derive(Debug)]
pub struct ApiError {
    status: StatusCode,
    message: String,
}

impl ApiError {
    pub fn bad_request(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            message: message.into(),
        }
    }

    pub fn not_found(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::NOT_FOUND,
            message: message.into(),
        }
    }

    pub fn internal(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: message.into(),
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (self.status, Json(json!({ "error": self.message }))).into_response()
    }
}

/// Run blocking filesystem work off the async runtime.
async fn blocking<T, F>(work: F) -> ApiResult<T>
where
    F: FnOnce() -> ApiResult<T> + Send + 'static,
    T: Send + 'static,
{
    match tokio::task::spawn_blocking(work).await {
        Ok(result) => result,
        Err(e) => Err(ApiError::internal(format!("background task failed: {}", e))),
    }
}

fn find_group(uuid: &str) -> ApiResult<Group> {
    let (stgs, _issues) = scan_storages();
    let mut gp_map = build_group_map_from_storages(&stgs);

    gp_map
        .remove(uuid)
        .ok_or_else(|| ApiError::not_found(format!("unknown group uuid: {}", uuid)))
}

fn open_cabinet(path: &str) -> ApiResult<Cabinet> {
    let dir = PathBuf::from(path.trim());

    if !dir.is_dir() {
        return Err(ApiError::bad_request(format!(
            "not a directory: {}",
            dir.display()
        )));
    }

    build_cabinet_from_path(&dir).map_err(|e| {
        ApiError::bad_request(format!(
            "failed to read cabinet config in {}: {}",
            dir.display(),
            e
        ))
    })
}

fn save_cabinet(cab: &Cabinet) -> ApiResult<()> {
    write_cabinet(cab).map_err(|e| {
        ApiError::internal(format!(
            "failed to write {}: {}",
            cab.cab_info.conf_abs_path.display(),
            e
        ))
    })
}

fn cabinet_response(cab: &Cabinet, message: String) -> Json<serde_json::Value> {
    let storage_root = cab
        .cab_info
        .abs_path
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| cab.cab_info.abs_path.clone());

    Json(json!({
        "message": message,
        "cabinet": dto::cabinet_dto(storage_root.as_path(), cab),
    }))
}

pub async fn health(State(state): State<Arc<AppState>>) -> Json<serde_json::Value> {
    Json(json!({
        "ok": true,
        "name": "rsdish-webui",
        "version": env!("CARGO_PKG_VERSION"),
        "uptime_secs": state.started.elapsed().as_secs(),
    }))
}

pub async fn overview() -> ApiResult<Json<dto::OverviewDto>> {
    blocking(|| Ok(Json(dto::overview_dto()))).await
}

#[derive(Debug, Deserialize)]
pub struct BrowseQuery {
    #[serde(default)]
    pub path: Option<String>,
}

pub async fn browse(Query(query): Query<BrowseQuery>) -> ApiResult<Json<dto::BrowseDto>> {
    blocking(move || {
        dto::browse_dto(query.path.as_deref())
            .map(Json)
            .map_err(|e| ApiError::bad_request(format!("failed to list directories: {}", e)))
    })
    .await
}

pub async fn get_config() -> ApiResult<Json<dto::ConfigDto>> {
    blocking(|| {
        let conf = crate::user::user_conf::user_conf();
        Ok(Json(dto::config_dto(conf.custom_storages)))
    })
    .await
}

#[derive(Debug, Deserialize)]
pub struct ConfigInput {
    #[serde(default)]
    pub custom_storages: Vec<String>,
}

pub async fn put_config(Json(input): Json<ConfigInput>) -> ApiResult<Json<dto::ConfigDto>> {
    blocking(move || {
        let mut paths: Vec<String> = Vec::new();

        for raw in input.custom_storages {
            let trimmed = raw.trim();
            if trimmed.is_empty() || paths.iter().any(|p| p == trimmed) {
                continue;
            }
            if !PathBuf::from(trimmed).is_dir() {
                warn!("custom storage {} is not an existing directory", trimmed);
            }
            paths.push(trimmed.to_string());
        }

        let conf = UserConfig {
            custom_storages: paths.clone(),
        };

        confy::store(env!("APP_NAME"), env!("APP_CONFIG_NAME"), &conf)
            .map_err(|e| ApiError::internal(format!("failed to write config: {}", e)))?;

        Ok(Json(dto::config_dto(paths)))
    })
    .await
}

#[derive(Debug, Deserialize)]
pub struct PlanQuery {
    #[serde(default)]
    pub strict: bool,
}

pub async fn group_plan(
    Path(uuid): Path<String>,
    Query(query): Query<PlanQuery>,
) -> ApiResult<Json<dto::PlanDto>> {
    blocking(move || {
        let gp = find_group(&uuid)?;
        let vmem = build_virtual_member_from_group(&gp);
        Ok(Json(dto::plan_dto(&gp, &vmem, query.strict)))
    })
    .await
}

#[derive(Debug, Deserialize)]
pub struct SyncInput {
    #[serde(default)]
    pub strict: bool,
}

pub async fn group_sync(
    State(state): State<Arc<AppState>>,
    Path(uuid): Path<String>,
    Json(input): Json<SyncInput>,
) -> ApiResult<Json<dto::ActionDto>> {
    let _guard = state.action_lock.lock().await;

    blocking(move || {
        let gp = find_group(&uuid)?;
        let vmem = build_virtual_member_from_group(&gp);
        let report = gp.sync_from_vmem(&vmem, input.strict);
        Ok(Json(dto::action_dto("sync", &uuid, &report)))
    })
    .await
}

pub async fn group_link(
    State(state): State<Arc<AppState>>,
    Path(uuid): Path<String>,
) -> ApiResult<Json<dto::ActionDto>> {
    let _guard = state.action_lock.lock().await;

    blocking(move || {
        let gp = find_group(&uuid)?;
        let vmem = build_virtual_member_from_group(&gp);
        let report = gp.link_from_vmem(&vmem);
        Ok(Json(dto::action_dto("link", &uuid, &report)))
    })
    .await
}

pub async fn group_clean(
    State(state): State<Arc<AppState>>,
    Path(uuid): Path<String>,
) -> ApiResult<Json<dto::ActionDto>> {
    let _guard = state.action_lock.lock().await;

    blocking(move || {
        let gp = find_group(&uuid)?;
        let vmem = build_virtual_member_from_group(&gp);
        let report = gp.clean_from_vmem(&vmem);
        Ok(Json(dto::action_dto("clean", &uuid, &report)))
    })
    .await
}

#[derive(Debug, Deserialize)]
pub struct ExecInput {
    #[serde(default)]
    pub script: String,
}

pub async fn group_exec(
    State(state): State<Arc<AppState>>,
    Path(uuid): Path<String>,
    Json(input): Json<ExecInput>,
) -> ApiResult<Json<dto::ExecDto>> {
    if input.script.trim().is_empty() {
        return Err(ApiError::bad_request("script must not be empty"));
    }

    let _guard = state.action_lock.lock().await;

    blocking(move || {
        let gp = find_group(&uuid)?;
        let outcomes = gp.exec(&input.script);
        Ok(Json(dto::exec_dto(&uuid, &outcomes)))
    })
    .await
}

#[derive(Debug, Deserialize)]
pub struct CabinetPathInput {
    pub path: String,
}

#[derive(Debug, Deserialize)]
pub struct CabinetQuery {
    pub path: String,
}

/// Read a cabinet without touching it; the editor in the web UI uses it.
pub async fn cabinet_read(Query(query): Query<CabinetQuery>) -> ApiResult<Json<serde_json::Value>> {
    blocking(move || {
        let dir = PathBuf::from(query.path.trim());
        let conf_path = dir.join(env!("CABINET_CONFIG_NAME"));
        let path = dir.to_string_lossy().to_string();

        if !conf_path.is_file() {
            return Ok(Json(json!({ "path": path, "has_config": false })));
        }

        match build_cabinet_from_path(&dir) {
            Ok(cab) => {
                let storage_root = cab
                    .cab_info
                    .abs_path
                    .parent()
                    .map(|p| p.to_path_buf())
                    .unwrap_or_else(|| cab.cab_info.abs_path.clone());

                Ok(Json(json!({
                    "path": path,
                    "has_config": true,
                    "cabinet": dto::cabinet_dto(storage_root.as_path(), &cab),
                })))
            }
            Err(e) => Ok(Json(json!({
                "path": path,
                "has_config": true,
                "error": e.to_string(),
            }))),
        }
    })
    .await
}

pub async fn cabinet_init(
    Json(input): Json<CabinetPathInput>,
) -> ApiResult<Json<serde_json::Value>> {
    blocking(move || {
        let dir = PathBuf::from(input.path.trim());

        if !dir.is_dir() {
            return Err(ApiError::bad_request(format!(
                "not a directory: {}",
                dir.display()
            )));
        }

        let conf_path = dir.join(env!("CABINET_CONFIG_NAME"));
        if conf_path.exists() {
            return Err(ApiError::bad_request(format!(
                "config already exists at {}",
                conf_path.display()
            )));
        }

        let conf = default_cabinet_config();
        let toml_str = toml::to_string(&conf)
            .map_err(|e| ApiError::internal(format!("failed to serialise config: {}", e)))?;

        fs::write(&conf_path, toml_str).map_err(|e| {
            ApiError::internal(format!("failed to write {}: {}", conf_path.display(), e))
        })?;

        let cab = open_cabinet(&dir.to_string_lossy())?;
        Ok(cabinet_response(
            &cab,
            format!("created {}", conf_path.display()),
        ))
    })
    .await
}

#[derive(Debug, Deserialize)]
pub struct CabinetJoinInput {
    pub path: String,
    #[serde(default)]
    pub group_uuids: Vec<String>,
}

pub async fn cabinet_join(
    Json(input): Json<CabinetJoinInput>,
) -> ApiResult<Json<serde_json::Value>> {
    blocking(move || {
        let mut cab = open_cabinet(&input.path)?;

        let mut added: Vec<String> = Vec::new();
        let uuids: Vec<String> = if input.group_uuids.is_empty() {
            // same default as the CLI: join a brand new group
            vec![default_membership().group_uuid]
        } else {
            input.group_uuids
        };

        for raw in uuids {
            let uuid = raw.trim();
            if uuid.is_empty() {
                return Err(ApiError::bad_request("group uuid must not be empty"));
            }

            let mut membership = default_membership();
            membership.group_uuid = uuid.to_string();
            added.push(membership.group_uuid.clone());
            cab.cab_info.cab_conf.memberships.push(membership);
        }

        cab.cab_info
            .cab_conf
            .verify()
            .map_err(|e| ApiError::bad_request(e.to_string()))?;

        save_cabinet(&cab)?;

        Ok(cabinet_response(
            &cab,
            format!("joined group(s): {}", added.join(", ")),
        ))
    })
    .await
}

#[derive(Debug, Deserialize)]
pub struct CabinetPresetInput {
    pub path: String,
    pub preset: String,
}

pub async fn cabinet_preset(
    Json(input): Json<CabinetPresetInput>,
) -> ApiResult<Json<serde_json::Value>> {
    blocking(move || {
        let mut cab = open_cabinet(&input.path)?;

        let preset = input.preset.trim().to_lowercase();
        match preset.as_str() {
            "main" => cab.cab_info.cab_conf.to_main(),
            "mirror" => cab.cab_info.cab_conf.to_mirror(),
            "gate" => cab.cab_info.cab_conf.to_gate(),
            other => {
                return Err(ApiError::bad_request(format!(
                    "unknown preset {}: expected main, mirror or gate",
                    other
                )));
            }
        }

        save_cabinet(&cab)?;

        Ok(cabinet_response(&cab, format!("applied {} preset", preset)))
    })
    .await
}

#[derive(Debug, Deserialize)]
pub struct CabinetConfigInput {
    pub path: String,
    pub memberships: Vec<MemberConfig>,
}

pub async fn cabinet_put_config(
    Json(input): Json<CabinetConfigInput>,
) -> ApiResult<Json<serde_json::Value>> {
    blocking(move || {
        let mut cab = open_cabinet(&input.path)?;

        let conf = CabinetConfig {
            memberships: input.memberships,
        };
        conf.verify()
            .map_err(|e| ApiError::bad_request(e.to_string()))?;

        cab.cab_info.cab_conf = conf;
        save_cabinet(&cab)?;

        Ok(cabinet_response(&cab, "memberships saved".to_string()))
    })
    .await
}

#[derive(Debug, Deserialize)]
pub struct LogsQuery {
    #[serde(default)]
    pub since: u64,
    #[serde(default)]
    pub limit: Option<usize>,
}

pub async fn logs(Query(query): Query<LogsQuery>) -> Json<dto::LogsDto> {
    let buffer = log_store::buffer();
    let limit = query.limit.unwrap_or(log_store::DEFAULT_LIMIT);
    let lines = buffer.snapshot(query.since, limit);

    let next = lines.last().map(|line| line.seq).unwrap_or(query.since);

    Json(dto::LogsDto { lines, next })
}
