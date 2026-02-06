#![cfg(not(target_arch = "wasm32"))]

use std::{
    error::Error,
    io::{BufRead, BufReader, Write},
    net::{TcpListener, TcpStream},
    process::Command,
    sync::{Mutex, OnceLock, mpsc::channel},
    time::Duration,
};

use num::{BigInt, FromPrimitive};

use owo_colors::OwoColorize;
use proto::{MarkerInfo, WcpCSMessage, WcpCommand, WcpSCMessage};

#[allow(dead_code)]
mod proto;

static CONNECTION: OnceLock<Mutex<Surfer>> = OnceLock::new();

struct Surfer {
    stream: Option<TcpStream>,
    trace_path: String,
    commands: Vec<String>,
    loaded_signals: Vec<String>,
}

#[derive(Clone)]
pub enum SurferCommand {
    LoadSignals(Vec<String>),
    MarkTime(f64),
    MarkPeriods(Vec<(u64, u64)>, String, String),
    Zoom(u64, u64),
}

impl Surfer {
    fn new(trace_path: &str) -> Self {
        let trace_path = trace_path.to_owned();
        Self {
            stream: None,
            trace_path,
            commands: Vec::new(),
            loaded_signals: Vec::new(),
        }
    }
    fn send_command(&mut self, command: SurferCommand) -> Result<(), Box<dyn Error>> {
        match command {
            SurferCommand::LoadSignals(signals) => Ok(self.load_signals(signals)?),
            SurferCommand::MarkTime(time) => {
                Ok(self.mark_times(vec![(time, "marker".into())], String::from("Red"))?)
            }
            SurferCommand::MarkPeriods(items, suffix, color) => {
                let markers = items
                    .into_iter()
                    .enumerate()
                    .flat_map(|(i, (start, end))| {
                        [
                            (start as f64, format!("start {i} {suffix}")),
                            (end as f64, format!("end {i}")),
                        ]
                    })
                    .collect();
                Ok(self.mark_times(markers, color)?)
            }
            SurferCommand::Zoom(start, end) => Ok(self.zoom_to_range(start, end)?),
        }
    }
    fn await_reponse(&mut self) -> Result<WcpSCMessage, Box<dyn Error>> {
        let mut stream = self.stream.as_mut().ok_or("No connection to Surfer")?;
        let mut reader = BufReader::new(&mut stream);

        let mut response = Vec::new();
        if let Ok(len) = reader.read_until(0, &mut response)
            && len > 0
        {
            let response: Result<WcpSCMessage, _> = serde_json::from_slice(&response[..len - 1]);

            return match response {
                Ok(r) => Ok(r),
                Err(e) => Err(Box::new(e)),
            };
        }
        Err("No response from Surfer")?
    }
    fn send_message_internal(
        &mut self,
        message: &WcpCSMessage,
    ) -> Result<WcpSCMessage, Box<dyn Error>> {
        let stream = &mut self.stream.as_mut().ok_or("No connection to Surfer")?;
        let buf = serde_json::to_string(message).map_err(|_| "Message could not be serialized")?;
        stream.write_all(buf.as_bytes())?;
        stream.write_all(b"\0")?;

        self.await_reponse()
    }

    fn send_message(&mut self, message: &WcpCSMessage) -> Option<WcpSCMessage> {
        self.send_message_internal(message).ok()
    }

    fn send_message_without_response(
        &mut self,
        message: &WcpCSMessage,
    ) -> Result<(), Box<dyn Error>> {
        let stream = &mut self.stream.as_mut().ok_or("No connection to Surfer")?;
        let buf = serde_json::to_string(message).map_err(|_| "Message should be serializable")?;
        stream.write_all(buf.as_bytes())?;
        stream.write_all(b"\0")?;
        Ok(())
    }

    fn load_signals(&mut self, signals: Vec<String>) -> Result<(), &'static str> {
        let Some(WcpSCMessage::response(proto::WcpResponse::get_item_list { ids })) =
            self.send_message(&WcpCSMessage::command(proto::WcpCommand::get_item_list))
        else {
            return Err("[ERROR] Did not receive response for get_item_list");
        };
        if ids.len() != self.loaded_signals.len() {
            let Some(WcpSCMessage::response(proto::WcpResponse::get_item_info { results })) = self
                .send_message(&WcpCSMessage::command(proto::WcpCommand::get_item_info {
                    ids,
                }))
            else {
                return Err("[ERROR] Did not receive response for get_item_info");
            };
            self.loaded_signals = results.into_iter().map(|r| r.name).collect();
        }
        let mut signals = signals
            .into_iter()
            .filter(|s| !self.loaded_signals.contains(s))
            .collect::<Vec<_>>();
        self.send_message(&WcpCSMessage::command(proto::WcpCommand::add_variables {
            variables: signals.clone(),
        }));
        self.loaded_signals.append(&mut signals);
        Ok(())
    }

    fn mark_times(&mut self, markers: Vec<(f64, String)>, color: String) -> Result<(), String> {
        if self.commands.contains(&String::from("add_markers")) {
            let markers: Vec<_> = markers
                .into_iter()
                .map(|(time, name)| MarkerInfo {
                    time: BigInt::from_f64(time).expect("Should be valid"),
                    name: Some(name),
                    move_focus: true,
                })
                .collect();
            let markers_len = markers.len();

            if let Some(response) =
                self.send_message(&WcpCSMessage::command(WcpCommand::add_markers { markers }))
            {
                if let WcpSCMessage::response(proto::WcpResponse::add_markers { ids }) = response {
                    let ids_len = ids.len();
                    if markers_len != ids_len {
                        return Err("[WARN] Cannot add more markers in surfer".into());
                    }
                    let color = &color;
                    for id in ids {
                        self.loaded_signals.push(format!("marker {}", id.0));
                        if self
                            .send_message_without_response(&WcpCSMessage::command(
                                WcpCommand::set_item_color {
                                    id,
                                    color: String::from(color),
                                },
                            ))
                            .is_err()
                        {
                            return Err("[Error] Failed to send add markers".into());
                        }
                    }
                    if ids_len > 0 && self.await_reponse().is_err() {
                        return Err("[Error] No response from surfer for add markers".into());
                    }
                } else if let WcpSCMessage::error { message, .. } = response {
                    return Err(format!("[WARN] Received error from surfer {message}"));
                }
            }
        } else {
            return Err(
                "[Info] Surfer version does not support adding markers. Skipping".to_owned(),
            );
        }
        Ok(())
    }

    fn zoom_to_range(&mut self, start: u64, end: u64) -> Result<(), &'static str> {
        if self.commands.contains(&String::from("set_viewport_range")) {
            let start = BigInt::from_u64(start).expect("Should be valid");
            let end = BigInt::from_u64(end).expect("Should be valid");
            self.send_message(&WcpCSMessage::command(WcpCommand::set_viewport_range {
                start,
                end,
            }));
            Ok(())
        } else {
            Err("[Info] Surfer version does not support setting viewport range. Skipping")
        }
    }

    fn connect(&mut self) -> Result<(), Box<dyn Error>> {
        self.stream = Some(connect_or_start_surfer()?);
        if let Some(response) = self.send_message(&WcpCSMessage::greeting {
            version: String::from("0"),
            commands: vec![],
        }) {
            match response {
                WcpSCMessage::greeting {
                    version: _,
                    commands,
                } => self.commands = commands,
                response => {
                    eprintln!(
                        "{} {response:?}",
                        "[ERROR] Received other response from surfer for greeting".bright_red()
                    )
                }
            }
        } else {
            eprintln!(
                "{}",
                "[ERROR] Did not receive response for a greeting from surfer".bright_red()
            );
        }

        let mut trace_full_path =
            std::env::current_dir().expect("Current directory should be valid");
        trace_full_path.push(self.trace_path.as_str());
        if let Some(response) = self.send_message(&WcpCSMessage::command(WcpCommand::load {
            source: trace_full_path.display().to_string(),
        })) {
            match response {
                WcpSCMessage::response(proto::WcpResponse::ack) => {
                    eprintln!("[Info] Succesfully connected to Surfer.");
                    Ok(())
                }
                response => Err(format!(
                    "[ERROR] Received other response from surfer for load {response:?}"
                )
                .into()),
            }
        } else {
            Err("[ERROR] Did not receive response for a load from surfer".into())
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn connect_or_start_surfer() -> Result<TcpStream, Box<dyn Error>> {
    // 54321 is the default port used by surfer wcp server
    match TcpStream::connect("127.0.0.1:54321") {
        Ok(stream) => {
            eprintln!("[Info] Connecting to Surfer...");
            Ok(stream)
        }
        Err(_) => {
            eprintln!("[Info] Starting Surfer...");
            let (port_sender, port_receiver) = channel();
            let (tx, rx) = channel();
            let surfer_failed_sender = tx.clone();
            std::thread::spawn(move || {
                let Ok(listener) = TcpListener::bind("127.0.0.1:0") else {
                    eprintln!("{}", "[ERROR] no free port".bright_red());
                    return;
                };
                port_sender
                    .send(
                        listener
                            .local_addr()
                            .expect("Listener should have an address")
                            .port(),
                    )
                    .expect("Main thread should not close channel before receiving port");
                if let Ok((stream, _)) = listener.accept()
                    && let Ok(_) = tx.send(Ok(stream))
                {}
            });
            let Ok(port) = port_receiver.recv() else {
                return Err("[ERROR] Failed to open a socket".into());
            };
            std::thread::spawn(move || {
                match Command::new("surfer")
                    .arg("--wcp-initiate")
                    .arg(port.to_string())
                    .output()
                {
                    Ok(output) => {
                        if !output.status.success() {
                            if let Ok(stdout) = str::from_utf8(&output.stdout)
                                && let Ok(stderr) = str::from_utf8(&output.stderr)
                            {
                                let _ = surfer_failed_sender.send(Err(format!(
                                    "{}\nstdout: {}\n{} {}",
                                    "[ERROR] Surfer stopped unexpectedly".bright_red(),
                                    stdout,
                                    "stderr:".bright_red(),
                                    stderr
                                )));
                            } else {
                                let _ = surfer_failed_sender
                                    .send(Err("[ERROR] Surfer stopped unexpectedly".to_owned()));
                            }
                        }
                    }
                    Err(e) => {
                        let _ = surfer_failed_sender
                            .send(Err(format!("[ERROR] Failed to run surfer: {e}"))); // We ignore this error because this thread will stop after this call
                    }
                };
            });
            let mut ret = Err("[ERROR] Failed to run surfer".into());
            for t in (0..10).rev() {
                let recv = rx.recv_timeout(Duration::from_secs(1)).ok();
                if let Some(r) = recv {
                    ret = r;
                    break;
                }
                eprintln!("[WARN] Waiting for surfer... {}", t);
            }
            // if Surfer fails to start above timeout returns None, then we want to close the socket that is waiting in accept
            if ret.is_err() && TcpStream::connect(format!("127.0.0.1:{port}")).is_err() {
                eprintln!("{}", "[ERROR] Failed to cleanup the listener".bright_red());
            }
            ret.map_err(|e| e.into())
        }
    }
}

pub fn send_to_surfer(trace_path: &str, commands: Vec<SurferCommand>) {
    if commands.is_empty() {
        return;
    }
    let trace_path = trace_path.to_owned();
    std::thread::spawn(move || {
        let mut surfer = CONNECTION
            .get_or_init(|| Mutex::new(Surfer::new(&trace_path)))
            .lock()
            .expect("Connection mutex got poisoned");

        let mut commands = commands.into_iter();
        let first_command = commands.next().expect("Is not empty");
        if surfer.send_command(first_command.clone()).is_err() {
            if let Err(e) = surfer.connect() {
                eprintln!("{}", e.bright_red());
                return;
            }
            if let Err(e) = surfer.send_command(first_command) {
                eprintln!("{}", e.bright_red());
                return;
            }
        }
        for c in commands {
            if let Err(e) = surfer.send_command(c) {
                eprintln!("{}", e.bright_red());
                return;
            }
        }
    });
}
