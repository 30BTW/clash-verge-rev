use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// 单条流量记录（连接关闭时生成）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrafficRecord {
    /// 记录 ID (自增主键)
    pub id: i64,
    /// 连接开始时间
    pub start_time: DateTime<Utc>,
    /// 连接结束时间
    pub end_time: DateTime<Utc>,
    /// 目标主机 (域名:端口)
    pub host: String,
    /// 目标域名 (提取自主机，用于聚合)
    pub domain: String,
    /// 进程名
    pub process: String,
    /// 进程路径
    pub process_path: Option<String>,
    /// 上传流量 (字节)
    pub upload: u64,
    /// 下载流量 (字节)
    pub download: u64,
    /// 代理链 (如 "节点A / 节点B")
    pub chains: String,
    /// 匹配规则
    pub rule: String,
    /// 网络类型 (TCP/UDP)
    pub network: String,
    /// 源 IP
    pub source_ip: String,
}

/// 连接快照 (用于 diff 计算)
#[derive(Debug, Clone)]
pub struct ConnectionSnapshot {
    pub id: String,
    pub host: String,
    pub domain: String,
    pub process: String,
    pub process_path: Option<String>,
    pub upload: u64,
    pub download: u64,
    pub start_time: DateTime<Utc>,
    pub chains: Vec<String>,
    pub rule: String,
    pub network: String,
    pub source_ip: String,
}

/// 查询参数
#[derive(Debug, Deserialize)]
pub struct TrafficQueryParams {
    /// 开始时间
    pub start: Option<DateTime<Utc>>,
    /// 结束时间
    pub end: Option<DateTime<Utc>>,
    /// 按域名分组
    pub group_by_domain: Option<bool>,
    /// 按进程分组
    pub group_by_process: Option<bool>,
    /// 限制返回条数
    pub limit: Option<i64>,
}

/// 聚合结果
#[derive(Debug, Serialize)]
pub struct TrafficAggregate {
    /// 分组键 (域名或进程名)
    pub key: String,
    /// 总上传
    pub total_upload: u64,
    /// 总下载
    pub total_download: u64,
    /// 连接数
    pub connection_count: u64,
    /// 最后活跃时间
    pub last_active: DateTime<Utc>,
}

/// 统计概览
#[derive(Debug, Serialize)]
pub struct TrafficSummary {
    /// 总上传流量
    pub total_upload: u64,
    /// 总下载流量
    pub total_download: u64,
    /// 总连接数
    pub total_connections: u64,
    /// 唯一域名数
    pub unique_domains: u64,
    /// 唯一进程数
    pub unique_processes: u64,
}
