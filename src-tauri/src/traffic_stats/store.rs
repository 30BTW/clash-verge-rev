use anyhow::Result;
use chrono::{DateTime, Utc};
use parking_lot::Mutex;
use rusqlite::{Connection, params};
use std::path::PathBuf;
use std::sync::Arc;

use super::types::{TrafficAggregate, TrafficQueryParams, TrafficRecord, TrafficSummary};

/// 数据库最大大小 (100 MB)
const MAX_DB_SIZE_BYTES: u64 = 100 * 1024 * 1024;
/// 数据库大小检查间隔 (每 1000 次写入检查一次)
const DB_SIZE_CHECK_INTERVAL: u64 = 1000;

pub struct SQLiteStore {
    conn: Arc<Mutex<Connection>>,
    db_path: PathBuf,
    write_count: Arc<Mutex<u64>>,
}

impl SQLiteStore {
    pub fn new(db_path: PathBuf) -> Result<Self> {
        // 确保父目录存在
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        // 打开数据库连接
        let conn = Connection::open(&db_path)?;

        // 启用 WAL 模式
        conn.pragma_update(None, "journal_mode", "WAL")?;
        // 设置同步模式为 NORMAL (性能与安全平衡)
        conn.pragma_update(None, "synchronous", "NORMAL")?;
        // 设置缓存大小 (负值表示 KB，正值表示页数)
        conn.pragma_update(None, "cache_size", -64000)?; // 64MB 缓存
        // 启用增量自动清理 (避免 VACUUM 锁死数据库)
        conn.pragma_update(None, "auto_vacuum", "INCREMENTAL")?;

        // 创建表和索引
        Self::init_schema(&conn)?;

        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
            db_path,
            write_count: Arc::new(Mutex::new(0)),
        })
    }

    fn init_schema(conn: &Connection) -> Result<()> {
        conn.execute_batch(
            r"
            CREATE TABLE IF NOT EXISTS traffic_records (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                start_time TEXT NOT NULL,
                end_time TEXT NOT NULL,
                host TEXT NOT NULL,
                domain TEXT NOT NULL,
                process TEXT NOT NULL,
                process_path TEXT,
                upload INTEGER NOT NULL,
                download INTEGER NOT NULL,
                chains TEXT NOT NULL,
                rule TEXT NOT NULL,
                network TEXT NOT NULL,
                source_ip TEXT NOT NULL
            );

            CREATE INDEX IF NOT EXISTS idx_domain ON traffic_records(domain);
            CREATE INDEX IF NOT EXISTS idx_process ON traffic_records(process);
            CREATE INDEX IF NOT EXISTS idx_end_time ON traffic_records(end_time);
            ",
        )?;
        Ok(())
    }

    /// 批量插入记录 (同步操作，需在 spawn_blocking 或独立线程中调用)
    #[expect(clippy::significant_drop_tightening)]
    pub fn insert_batch(&self, records: &[TrafficRecord]) -> Result<()> {
        if records.is_empty() {
            return Ok(());
        }

        {
            let mut conn = self.conn.lock();
            let tx = conn.transaction()?;

            {
                let mut stmt = tx.prepare_cached(
                    r"
                    INSERT INTO traffic_records (
                        start_time, end_time, host, domain, process, process_path,
                        upload, download, chains, rule, network, source_ip
                    ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)
                    ",
                )?;

                for record in records {
                    stmt.execute(params![
                        record.start_time.to_rfc3339(),
                        record.end_time.to_rfc3339(),
                        record.host,
                        record.domain,
                        record.process,
                        record.process_path,
                        record.upload as i64,
                        record.download as i64,
                        record.chains,
                        record.rule,
                        record.network,
                        record.source_ip,
                    ])?;
                }
            }

            tx.commit()?;
        }

        // 检查数据库大小
        self.check_db_size()?;

        Ok(())
    }

    /// 检查数据库大小，超过限制时清理旧数据
    fn check_db_size(&self) -> Result<()> {
        let mut count = self.write_count.lock();
        *count += 1;

        if (*count).is_multiple_of(DB_SIZE_CHECK_INTERVAL) {
            *count = 0;
            drop(count); // 释放锁

            if std::fs::metadata(&self.db_path).is_ok_and(|m| m.len() > MAX_DB_SIZE_BYTES) {
                self.cleanup_old_records()?;
            }
        }

        Ok(())
    }

    /// 清理旧记录 (保留最近 70% 的数据)
    #[expect(clippy::significant_drop_tightening)]
    fn cleanup_old_records(&self) -> Result<()> {
        let (_, to_delete) = {
            let conn = self.conn.lock();

            let total: i64 = conn.query_row("SELECT COUNT(*) FROM traffic_records", [], |row| row.get(0))?;

            // 计算要删除的记录数 (删除 30%)
            let to_delete = (total as f64 * 0.3) as i64;

            (total, to_delete)
        };

        if to_delete > 0 {
            let conn = self.conn.lock();
            conn.execute(
                "DELETE FROM traffic_records WHERE id IN (
                    SELECT id FROM traffic_records ORDER BY end_time ASC LIMIT ?1
                )",
                params![to_delete],
            )?;

            // 执行增量清理 (不会锁死数据库，只清理一页)
            // SQLite 会自动在后续操作中复用空闲页
            conn.execute("PRAGMA incremental_vacuum", [])?;
        }

        Ok(())
    }

    /// 按域名聚合查询
    #[expect(clippy::significant_drop_tightening)]
    pub fn query_by_domain(&self, params: &TrafficQueryParams) -> Result<Vec<TrafficAggregate>> {
        let conn = self.conn.lock();
        let limit = params.limit.unwrap_or(100);

        let mut stmt = conn.prepare_cached(
            r"
            SELECT
                domain as key,
                SUM(upload) as total_upload,
                SUM(download) as total_download,
                COUNT(*) as connection_count,
                MAX(end_time) as last_active
            FROM traffic_records
            WHERE (?1 IS NULL OR end_time >= ?1)
              AND (?2 IS NULL OR end_time <= ?2)
            GROUP BY domain
            ORDER BY total_download DESC
            LIMIT ?3
            ",
        )?;

        let rows = stmt.query_map(
            params![
                params.start.as_ref().map(|t| t.to_rfc3339()),
                params.end.as_ref().map(|t| t.to_rfc3339()),
                limit,
            ],
            |row| {
                let last_active_str: String = row.get(4)?;
                let last_active = DateTime::parse_from_rfc3339(&last_active_str)
                    .map(|dt| dt.with_timezone(&Utc))
                    .unwrap_or_else(|_| Utc::now());

                Ok(TrafficAggregate {
                    key: row.get(0)?,
                    total_upload: row.get::<_, i64>(1)? as u64,
                    total_download: row.get::<_, i64>(2)? as u64,
                    connection_count: row.get::<_, i64>(3)? as u64,
                    last_active,
                })
            },
        )?;

        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|e| anyhow::anyhow!("Query error: {}", e))
    }

    /// 按进程聚合查询
    #[expect(clippy::significant_drop_tightening)]
    pub fn query_by_process(&self, params: &TrafficQueryParams) -> Result<Vec<TrafficAggregate>> {
        let conn = self.conn.lock();
        let limit = params.limit.unwrap_or(100);

        let mut stmt = conn.prepare_cached(
            r"
            SELECT
                process as key,
                SUM(upload) as total_upload,
                SUM(download) as total_download,
                COUNT(*) as connection_count,
                MAX(end_time) as last_active
            FROM traffic_records
            WHERE (?1 IS NULL OR end_time >= ?1)
              AND (?2 IS NULL OR end_time <= ?2)
            GROUP BY process
            ORDER BY total_download DESC
            LIMIT ?3
            ",
        )?;

        let rows = stmt.query_map(
            params![
                params.start.as_ref().map(|t| t.to_rfc3339()),
                params.end.as_ref().map(|t| t.to_rfc3339()),
                limit,
            ],
            |row| {
                let last_active_str: String = row.get(4)?;
                let last_active = DateTime::parse_from_rfc3339(&last_active_str)
                    .map(|dt| dt.with_timezone(&Utc))
                    .unwrap_or_else(|_| Utc::now());

                Ok(TrafficAggregate {
                    key: row.get(0)?,
                    total_upload: row.get::<_, i64>(1)? as u64,
                    total_download: row.get::<_, i64>(2)? as u64,
                    connection_count: row.get::<_, i64>(3)? as u64,
                    last_active,
                })
            },
        )?;

        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|e| anyhow::anyhow!("Query error: {}", e))
    }

    /// 按时间段查询原始记录
    #[expect(clippy::significant_drop_tightening)]
    pub fn query_records(&self, params: &TrafficQueryParams) -> Result<Vec<TrafficRecord>> {
        let conn = self.conn.lock();
        let limit = params.limit.unwrap_or(100);

        let mut stmt = conn.prepare_cached(
            r"
            SELECT
                id, start_time, end_time, host, domain, process, process_path,
                upload, download, chains, rule, network, source_ip
            FROM traffic_records
            WHERE (?1 IS NULL OR end_time >= ?1)
              AND (?2 IS NULL OR end_time <= ?2)
            ORDER BY end_time DESC
            LIMIT ?3
            ",
        )?;

        let rows = stmt.query_map(
            params![
                params.start.as_ref().map(|t| t.to_rfc3339()),
                params.end.as_ref().map(|t| t.to_rfc3339()),
                limit,
            ],
            |row| {
                let start_time_str: String = row.get(1)?;
                let end_time_str: String = row.get(2)?;
                let process_path: Option<String> = row.get(6)?;

                Ok(TrafficRecord {
                    id: row.get(0)?,
                    start_time: DateTime::parse_from_rfc3339(&start_time_str)
                        .map(|dt| dt.with_timezone(&Utc))
                        .unwrap_or_else(|_| Utc::now()),
                    end_time: DateTime::parse_from_rfc3339(&end_time_str)
                        .map(|dt| dt.with_timezone(&Utc))
                        .unwrap_or_else(|_| Utc::now()),
                    host: row.get(3)?,
                    domain: row.get(4)?,
                    process: row.get(5)?,
                    process_path,
                    upload: row.get::<_, i64>(7)? as u64,
                    download: row.get::<_, i64>(8)? as u64,
                    chains: row.get(9)?,
                    rule: row.get(10)?,
                    network: row.get(11)?,
                    source_ip: row.get(12)?,
                })
            },
        )?;

        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|e| anyhow::anyhow!("Query error: {}", e))
    }

    /// 获取统计概览
    pub fn get_summary(&self, start: Option<DateTime<Utc>>, end: Option<DateTime<Utc>>) -> Result<TrafficSummary> {
        let conn = self.conn.lock();

        let total_upload: i64 = conn.query_row(
            "SELECT COALESCE(SUM(upload), 0) FROM traffic_records WHERE (?1 IS NULL OR end_time >= ?1) AND (?2 IS NULL OR end_time <= ?2)",
            params![start.as_ref().map(|t| t.to_rfc3339()), end.as_ref().map(|t| t.to_rfc3339())],
            |row| row.get(0),
        )?;

        let total_download: i64 = conn.query_row(
            "SELECT COALESCE(SUM(download), 0) FROM traffic_records WHERE (?1 IS NULL OR end_time >= ?1) AND (?2 IS NULL OR end_time <= ?2)",
            params![start.as_ref().map(|t| t.to_rfc3339()), end.as_ref().map(|t| t.to_rfc3339())],
            |row| row.get(0),
        )?;

        let total_connections: i64 = conn.query_row(
            "SELECT COUNT(*) FROM traffic_records WHERE (?1 IS NULL OR end_time >= ?1) AND (?2 IS NULL OR end_time <= ?2)",
            params![start.as_ref().map(|t| t.to_rfc3339()), end.as_ref().map(|t| t.to_rfc3339())],
            |row| row.get(0),
        )?;

        let unique_domains: i64 = conn.query_row(
            "SELECT COUNT(DISTINCT domain) FROM traffic_records WHERE (?1 IS NULL OR end_time >= ?1) AND (?2 IS NULL OR end_time <= ?2)",
            params![start.as_ref().map(|t| t.to_rfc3339()), end.as_ref().map(|t| t.to_rfc3339())],
            |row| row.get(0),
        )?;

        let unique_processes: i64 = conn.query_row(
            "SELECT COUNT(DISTINCT process) FROM traffic_records WHERE (?1 IS NULL OR end_time >= ?1) AND (?2 IS NULL OR end_time <= ?2)",
            params![start.as_ref().map(|t| t.to_rfc3339()), end.as_ref().map(|t| t.to_rfc3339())],
            |row| row.get(0),
        )?;
        drop(conn);

        Ok(TrafficSummary {
            total_upload: total_upload as u64,
            total_download: total_download as u64,
            total_connections: total_connections as u64,
            unique_domains: unique_domains as u64,
            unique_processes: unique_processes as u64,
        })
    }
}
