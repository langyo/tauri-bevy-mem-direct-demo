use std::sync::{Arc, Mutex};
use tokio::sync::Notify;

pub struct FrameBuffer {
    pub front: Arc<Mutex<Vec<u8>>>,
    pub back: Arc<Mutex<Vec<u8>>>,
    pub width: Arc<Mutex<u32>>,
    pub height: Arc<Mutex<u32>>,
    pub seq: Arc<Mutex<u64>>,
    pub notify: Arc<Notify>,
}

impl FrameBuffer {
    pub fn new() -> Self {
        Self {
            front: Arc::new(Mutex::new(Vec::new())),
            back: Arc::new(Mutex::new(Vec::new())),
            width: Arc::new(Mutex::new(0)),
            height: Arc::new(Mutex::new(0)),
            seq: Arc::new(Mutex::new(0)),
            notify: Arc::new(Notify::new()),
        }
    }

    pub fn swap(&self) {
        let mut f = self.front.lock().unwrap();
        let mut b = self.back.lock().unwrap();
        std::mem::swap(&mut *f, &mut *b);
        *self.seq.lock().unwrap() += 1;
        self.notify.notify_waiters();
    }
}
