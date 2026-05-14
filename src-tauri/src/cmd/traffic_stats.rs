use crate::cmd::CmdResult;
use crate::traffic_stats::{TrafficAggregate, TrafficQueryParams, TrafficStatsManager, TrafficSummary};
use chrono::{DateTime, Utc};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct TrafficQueryRequest {
    pub start: Option<String>,
    pub end: Option<String>,
    pub group_by: String, // "domain" | "process"
    pub limit: Option<i64>,
}

/// 查询流量统计 (使用 spawn_blocking 避免 DB 操作阻塞 tokio 运行时)
#[tauri::command]
pub async fn query_traffic_stats(params: TrafficQueryRequest) -> CmdResult<Vec<TrafficAggregate>> {
    let start = params
        .start
        .and_then(|s| DateTime::parse_from_rfc3339(&s).ok())
        .map(|dt| dt.with_timezone(&Utc));
    let end = params
        .end
        .and_then(|s| DateTime::parse_from_rfc3339(&s).ok())
        .map(|dt| dt.with_timezone(&Utc));

    let query_params = TrafficQueryParams {
        start,
        end,
        group_by_domain: Some(params.group_by == "domain"),
        group_by_process: Some(params.group_by == "process"),
        limit: params.limit,
    };

    let group_by = params.group_by.clone();

    // 使用 spawn_blocking 执行同步数据库查询
    let inner_result = tokio::task::spawn_blocking(move || {
        let manager = TrafficStatsManager::global();
        match group_by.as_str() {
            "domain" => manager.query_by_domain(&query_params),
            "process" => manager.query_by_process(&query_params),
            _ => Err(anyhow::anyhow!("Invalid group_by value")),
        }
    })
    .await
    .map_err(|e| smartstring::alias::String::from(e.to_string()))?;

    inner_result.map_err(|e| smartstring::alias::String::from(e.to_string()))
}

/// 获取流量概览 (使用 spawn_blocking)
#[tauri::command]
pub async fn get_traffic_summary(start: Option<String>, end: Option<String>) -> CmdResult<TrafficSummary> {
    let start_dt = start
        .and_then(|s| DateTime::parse_from_rfc3339(&s).ok())
        .map(|dt| dt.with_timezone(&Utc));
    let end_dt = end
        .and_then(|s| DateTime::parse_from_rfc3339(&s).ok())
        .map(|dt| dt.with_timezone(&Utc));

    let inner_result = tokio::task::spawn_blocking(move || TrafficStatsManager::global().get_summary(start_dt, end_dt))
        .await
        .map_err(|e| smartstring::alias::String::from(e.to_string()))?;

    inner_result.map_err(|e| smartstring::alias::String::from(e.to_string()))
}
