use bevy::prelude::*;
use flume::Receiver;
use shared::protocol::{IpcEnvelope, IpcPayload, LogLevel, MoveDirection, ToDesktop, ToRenderer};
use std::io::{BufRead, Write};

#[derive(Resource)]
pub struct IpcChannel {
    pending_moves: Vec<MoveDirection>,
    pending_picks: Vec<(u64, f32, f32)>,
    pending_resolution: Option<(u64, u32, u32)>,
    stdin_rx: Receiver<IpcEnvelope>,
    next_id: u64,
}

impl IpcChannel {
    pub fn new() -> Self {
        let (tx, rx) = flume::unbounded();

        std::thread::spawn(move || {
            let stdin = std::io::stdin();
            for line in stdin.lock().lines().map_while(Result::ok) {
                let trimmed = line.trim();
                if trimmed.is_empty() {
                    continue;
                }
                match IpcEnvelope::decode_line(trimmed) {
                    Ok(envelope) => {
                        if tx.send(envelope).is_err() {
                            break;
                        }
                    }
                    Err(e) => {
                        eprintln!("[ipc] failed to decode message: {} — {}", e, trimmed);
                    }
                }
            }
        });

        Self {
            pending_moves: Vec::new(),
            pending_picks: Vec::new(),
            pending_resolution: None,
            stdin_rx: rx,
            next_id: 1,
        }
    }

    pub fn poll(&mut self) {
        while let Ok(envelope) = self.stdin_rx.try_recv() {
            match envelope.payload {
                IpcPayload::ToRenderer(msg) => match msg {
                    ToRenderer::MoveCamera { direction } => {
                        self.pending_moves.push(direction);
                    }
                    ToRenderer::PickRay { screen_x, screen_y } => {
                        self.pending_picks.push((envelope.id, screen_x, screen_y));
                    }
                    ToRenderer::SetResolution { width, height } => {
                        self.pending_resolution = Some((envelope.id, width, height));
                    }
                    ToRenderer::Ping { timestamp } => {
                        self.send(ToDesktop::Pong { timestamp });
                    }
                },
                IpcPayload::ToDesktop(_) => {
                    eprintln!("[ipc] received ToDesktop message on stdin, ignoring");
                }
            }
        }
    }

    pub fn drain_moves(&mut self) -> Vec<MoveDirection> {
        std::mem::take(&mut self.pending_moves)
    }

    pub fn drain_picks(&mut self) -> Vec<(u64, f32, f32)> {
        std::mem::take(&mut self.pending_picks)
    }

    pub fn drain_resolution(&mut self) -> Option<(u64, u32, u32)> {
        self.pending_resolution.take()
    }

    pub fn send(&mut self, payload: ToDesktop) {
        let line = IpcEnvelope::to_desktop(payload).encode_line();
        let stderr = std::io::stderr();
        let mut out = stderr.lock();
        let _ = writeln!(out, "__IPC__{}", line);
        let _ = out.flush();
    }

    pub fn send_reply(&mut self, request_id: u64, payload: ToDesktop) {
        let line = IpcEnvelope::to_desktop_reply(request_id, payload).encode_line();
        let stderr = std::io::stderr();
        let mut out = stderr.lock();
        let _ = writeln!(out, "__IPC__{}", line);
        let _ = out.flush();
    }

    pub fn send_log(&mut self, level: LogLevel, message: String) {
        let _ = self;
        eprintln!("[{:?}] {}", level, message);
    }

    pub fn alloc_id(&mut self) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }
}

pub fn ipc_read_system(mut ipc: ResMut<IpcChannel>) {
    ipc.poll();
}
