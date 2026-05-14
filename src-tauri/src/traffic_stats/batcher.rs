use parking_lot::Mutex;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio::sync::mpsc::error::TryRecvError;

use super::store::SQLiteStore;
use super::types::TrafficRecord;
use clash_verge_logging::{Type, logging};

/// 批量写入触发阈值 (记录数)
const BATCH_SIZE: usize = 100;
/// 批量写入最大间隔 (毫秒)
const BATCH_INTERVAL_MS: u64 = 5000;

pub struct RecordBatcher {
    store: Arc<SQLiteStore>,
    buffer: Arc<Mutex<Vec<TrafficRecord>>>,
}

impl RecordBatcher {
    pub fn new(store: Arc<SQLiteStore>) -> Self {
        Self {
            store,
            buffer: Arc::new(Mutex::new(Vec::with_capacity(BATCH_SIZE * 2))),
        }
    }

    /// 启动批量写入任务 (使用 std::thread 避免阻塞 tokio 运行时)
    pub fn spawn(self, mut record_rx: mpsc::Receiver<TrafficRecord>) -> std::thread::JoinHandle<()> {
        #[expect(clippy::expect_used)]
        std::thread::Builder::new()
            .name("traffic-stats-batcher".into())
            .spawn(move || {
                let mut last_flush = std::time::Instant::now();
                let flush_interval = std::time::Duration::from_millis(BATCH_INTERVAL_MS);

                loop {
                    // 非阻塞检查通道
                    match record_rx.try_recv() {
                        Ok(record) => {
                            let mut buffer = self.buffer.lock();
                            buffer.push(record);

                            // 达到批量阈值时立即写入
                            if buffer.len() >= BATCH_SIZE {
                                self.flush_buffer(&mut buffer);
                                last_flush = std::time::Instant::now();
                            }
                        }
                        Err(TryRecvError::Empty) => {
                            // 通道为空，检查是否需要定时刷新
                            if last_flush.elapsed() >= flush_interval {
                                let mut buffer = self.buffer.lock();
                                if !buffer.is_empty() {
                                    self.flush_buffer(&mut buffer);
                                }
                                drop(buffer);
                                last_flush = std::time::Instant::now();
                            }
                            // 短暂休眠避免 CPU 空转
                            std::thread::sleep(std::time::Duration::from_millis(100));
                        }
                        Err(TryRecvError::Disconnected) => {
                            // 通道关闭，刷新剩余数据后退出
                            let mut buffer = self.buffer.lock();
                            if !buffer.is_empty() {
                                self.flush_buffer(&mut buffer);
                            }
                            drop(buffer);
                            break;
                        }
                    }
                }
            })
            .expect("Failed to spawn batcher thread")
    }

    /// 刷新缓冲区到数据库 (在独立线程中同步执行)
    fn flush_buffer(&self, buffer: &mut Vec<TrafficRecord>) {
        if buffer.is_empty() {
            return;
        }

        let count = buffer.len();
        let records = std::mem::take(buffer);

        if let Err(err) = self.store.insert_batch(&records) {
            logging!(error, Type::System, "Traffic stats batch write failed: {err}");
        } else {
            logging!(info, Type::System, "Traffic stats: wrote {} records to database", count);
        }
    }

    /// 强制刷新 (用于退出时)
    #[expect(dead_code)]
    pub fn force_flush(&self) {
        let mut buffer = self.buffer.lock();
        if !buffer.is_empty() {
            self.flush_buffer(&mut buffer);
        }
    }
}
