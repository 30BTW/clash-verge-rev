use anyhow::Result;
use chrono::Utc;
use parking_lot::Mutex;
use std::collections::HashMap;
use std::sync::Arc;
use tauri_plugin_mihomo::models::Connection;
use tokio::sync::mpsc;

use super::types::{ConnectionSnapshot, TrafficRecord};
use crate::core::handle;
use clash_verge_logging::{logging, Type};

/// 轮询间隔 (毫秒) - 1秒，平衡性能和准确性
const POLL_INTERVAL_MS: u64 = 1000;

pub struct ConnectionMonitor {
    /// 当前活跃连接快照
    active_connections: Arc<Mutex<HashMap<String, ConnectionSnapshot>>>,
    /// 记录发送通道
    record_tx: mpsc::Sender<TrafficRecord>,
}

impl ConnectionMonitor {
    pub fn new(record_tx: mpsc::Sender<TrafficRecord>) -> Self {
        Self {
            active_connections: Arc::new(Mutex::new(HashMap::new())),
            record_tx,
        }
    }

    /// 启动监控任务 (轮询模式)
    pub async fn start(&self) {
        logging!(info, Type::System, "Traffic stats monitor started");
        let mut interval =
            tokio::time::interval(std::time::Duration::from_millis(POLL_INTERVAL_MS));

        loop {
            interval.tick().await;

            if handle::Handle::global().is_exiting() {
                // 退出时刷新所有活跃连接
                self.flush_active_connections();
                break;
            }

            // 获取当前连接列表
            match self.fetch_connections().await {
                Ok(connections) => {
                    let active_count = connections.len();
                    if active_count > 0 {
                        logging!(debug, Type::System, "Fetched {} active connections", active_count);
                    }
                    self.process_connections(connections)
                }
                Err(e) => {
                    // 连接失败时记录日志
                    logging!(warn, Type::System, "Failed to fetch connections: {}", e);
                }
            }
        }
    }

    /// 退出时将所有活跃连接转换为记录
    fn flush_active_connections(&self) {
        let now = Utc::now();
        let mut active = self.active_connections.lock();

        for snapshot in active.values() {
            let record = TrafficRecord {
                id: 0,
                start_time: snapshot.start_time,
                end_time: now,
                host: snapshot.host.clone(),
                domain: snapshot.domain.clone(),
                process: snapshot.process.clone(),
                process_path: snapshot.process_path.clone(),
                upload: snapshot.upload,
                download: snapshot.download,
                chains: snapshot.chains.join(" / "),
                rule: snapshot.rule.clone(),
                network: snapshot.network.clone(),
                source_ip: snapshot.source_ip.clone(),
            };
            let _ = self.record_tx.try_send(record);
        }

        active.clear();
    }

    /// 获取连接列表
    async fn fetch_connections(&self) -> Result<Vec<Connection>> {
        let mihomo = handle::Handle::mihomo().await;
        let response = mihomo.get_connections().await?;
        Ok(response.connections.unwrap_or_default())
    }

    /// 处理连接列表，检测关闭的连接
    fn process_connections(&self, connections: Vec<Connection>) {
        let now = Utc::now();
        let mut active = self.active_connections.lock();

        // 构建新连接 ID 集合
        let new_ids: std::collections::HashSet<String> =
            connections.iter().map(|c| c.id.clone()).collect();

        // 检测关闭的连接，生成记录
        let closed_records: Vec<TrafficRecord> = active
            .iter()
            .filter(|(id, _)| !new_ids.contains(*id))
            .map(|(_, snapshot)| TrafficRecord {
                id: 0,
                start_time: snapshot.start_time,
                end_time: now,
                host: snapshot.host.clone(),
                domain: snapshot.domain.clone(),
                process: snapshot.process.clone(),
                process_path: snapshot.process_path.clone(),
                upload: snapshot.upload,
                download: snapshot.download,
                chains: snapshot.chains.join(" / "),
                rule: snapshot.rule.clone(),
                network: snapshot.network.clone(),
                source_ip: snapshot.source_ip.clone(),
            })
            .collect();

        let closed_count = closed_records.len();
        // 发送关闭记录到批量写入通道
        for record in closed_records {
            let _ = self.record_tx.try_send(record);
        }

        if closed_count > 0 {
            logging!(info, Type::System, "Traffic stats: detected {} closed connections", closed_count);
        }

        // 更新活跃连接快照：移除已关闭的，更新/添加活跃的
        active.retain(|id, _| new_ids.contains(id));

        for conn in connections {
            if let Some(snapshot) = self.parse_connection(&conn) {
                active.insert(conn.id, snapshot);
            }
        }
    }

    /// 解析连接数据
    fn parse_connection(&self, conn: &Connection) -> Option<ConnectionSnapshot> {
        let metadata = &conn.metadata;

        // 提取域名和主机
        // 优先级: host > destination_ip > remote_destination
        let (host, domain) = if !metadata.host.is_empty() {
            let host = format!("{}:{}", metadata.host, metadata.destination_port);
            (host, metadata.host.clone())
        } else if !metadata.destination_ip.is_empty() {
            let host = format!("{}:{}", metadata.destination_ip, metadata.destination_port);
            (host, metadata.destination_ip.clone())
        } else if !metadata.remote_destination.is_empty() {
            let host = format!("{}:{}", metadata.remote_destination, metadata.destination_port);
            (host, metadata.remote_destination.clone())
        } else {
            let host = format!("unknown:{}", metadata.destination_port);
            (host, "unknown".to_string())
        };

        // 处理进程名
        let process = if metadata.process.is_empty() {
            "unknown".to_string()
        } else {
            metadata.process.clone()
        };

        // 处理进程路径
        let process_path = if metadata.process_path.is_empty() {
            None
        } else {
            Some(metadata.process_path.clone())
        };

        Some(ConnectionSnapshot {
            id: conn.id.clone(),
            host,
            domain,
            process,
            process_path,
            upload: conn.upload,
            download: conn.download,
            start_time: chrono::DateTime::parse_from_rfc3339(&conn.start)
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now()),
            chains: conn.chains.clone(),
            rule: conn.rule.clone(),
            network: format!("{:?}", metadata.network).to_lowercase(),
            source_ip: metadata.source_ip.clone(),
        })
    }
}
