use anyhow::Result;
use once_cell::sync::OnceCell;
use parking_lot::Mutex;
use std::sync::Arc;
use tauri::async_runtime::JoinHandle;
use tokio::sync::mpsc;

use super::batcher::RecordBatcher;
use super::monitor::ConnectionMonitor;
use super::store::SQLiteStore;
use super::types::{TrafficAggregate, TrafficQueryParams, TrafficRecord, TrafficSummary};
use crate::process::AsyncHandler;
use crate::utils::dirs;
use clash_verge_logging::{logging, Type};

pub struct TrafficStatsManager {
    store: Arc<SQLiteStore>,
    task_handle: Arc<Mutex<Option<JoinHandle<()>>>>,
    batcher_handle: Arc<Mutex<Option<std::thread::JoinHandle<()>>>>,
}

// 全局单例
static TRAFFIC_STATS_MANAGER: OnceCell<TrafficStatsManager> = OnceCell::new();

impl TrafficStatsManager {
    pub fn new() -> Result<Self> {
        // 数据库路径: {app_home_dir}/traffic_stats.db
        let db_path = dirs::app_home_dir()?.join("traffic_stats.db");
        logging!(info, Type::System, "Traffic stats database path: {:?}", db_path);
        let store = Arc::new(SQLiteStore::new(db_path)?);

        Ok(Self {
            store,
            task_handle: Arc::new(Mutex::new(None)),
            batcher_handle: Arc::new(Mutex::new(None)),
        })
    }

    pub fn global() -> &'static Self {
        TRAFFIC_STATS_MANAGER.get_or_init(|| {
            TrafficStatsManager::new().expect("Failed to initialize TrafficStatsManager")
        })
    }

    /// 启动后台采集任务
    pub fn start(&self) {
        let mut handle = self.task_handle.lock();

        if handle
            .as_ref()
            .is_some_and(|h| !h.inner().is_finished())
        {
            return;
        }

        let store = Arc::clone(&self.store);
        let batcher_handle = Arc::clone(&self.batcher_handle);

        let task = AsyncHandler::spawn(move || async move {
            // 创建记录通道
            let (record_tx, record_rx) = mpsc::channel(1000);

            // 启动批量写入器 (std::thread 独立线程)
            let batcher = RecordBatcher::new(store);
            let thread_handle = batcher.spawn(record_rx);
            *batcher_handle.lock() = Some(thread_handle);

            // 启动连接监控器
            let monitor = ConnectionMonitor::new(record_tx);
            monitor.start().await;
        });

        *handle = Some(task);
    }

    /// 停止后台采集任务 (优雅退出)
    pub fn stop(&self) {
        // 停止监控任务
        if let Some(handle) = self.task_handle.lock().take() {
            handle.abort();
        }

        // 等待 batcher 线程结束
        if let Some(handle) = self.batcher_handle.lock().take() {
            let _ = handle.join();
        }
    }

    // === 查询接口 ===

    pub fn query_by_domain(&self, params: &TrafficQueryParams) -> Result<Vec<TrafficAggregate>> {
        self.store.query_by_domain(params)
    }

    pub fn query_by_process(&self, params: &TrafficQueryParams) -> Result<Vec<TrafficAggregate>> {
        self.store.query_by_process(params)
    }

    pub fn query_records(&self, params: &TrafficQueryParams) -> Result<Vec<TrafficRecord>> {
        self.store.query_records(params)
    }

    pub fn get_summary(
        &self,
        start: Option<chrono::DateTime<chrono::Utc>>,
        end: Option<chrono::DateTime<chrono::Utc>>,
    ) -> Result<TrafficSummary> {
        self.store.get_summary(start, end)
    }
}
