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

impl Surfer {
    fn new(trace_path: &str) -> Self {
        let trace_path = trace_path.to_owned();
        let mut surfer = Self {
            stream: None,
            trace_path,
            commands: Vec::new(),
            loaded_signals: Vec::new(),
        };
        surfer.connect();
        surfer
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
        let buf = serde_json::to_string(message).expect("Message should be serializable");
        stream.write_all(buf.as_bytes())?;
        stream.write_all(b"\0")?;

        self.await_reponse()
    }

    fn send_message(&mut self, message: &WcpCSMessage) -> Option<WcpSCMessage> {
        match self.send_message_internal(message) {
            Ok(response) => Some(response),
            Err(_) => {
                self.connect();
                self.send_message_internal(message).ok()
            }
        }
    }

    fn send_message_without_response(
        &mut self,
        message: &WcpCSMessage,
    ) -> Result<(), Box<dyn Error>> {
        let stream = &mut self.stream.as_mut().ok_or("No connection to Surfer")?;
        let buf = serde_json::to_string(message).expect("Message should be serializable");
        stream.write_all(buf.as_bytes())?;
        stream.write_all(b"\0")?;
        Ok(())
    }

    fn load_signals(&mut self, signals: Vec<String>) {
        let Some(WcpSCMessage::response(proto::WcpResponse::get_item_list { ids })) =
            self.send_message(&WcpCSMessage::command(proto::WcpCommand::get_item_list))
        else {
            eprintln!(
                "{}",
                "[ERROR] Did not receive response for get_item_list".bright_red()
            );
            return;
        };
        if ids.len() != self.loaded_signals.len() {
            let Some(WcpSCMessage::response(proto::WcpResponse::get_item_info { results })) = self
                .send_message(&WcpCSMessage::command(proto::WcpCommand::get_item_info {
                    ids,
                }))
            else {
                eprintln!(
                    "{}",
                    "[ERROR] Did not receive response for get_item_list".bright_red()
                );
                return;
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
    }

    fn connect(&mut self) {
        self.stream = connect_or_start_surfer();
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
                    eprintln!("[Info] Succesfully connected to Surfer.")
                }
                response => {
                    eprintln!(
                        "{} {response:?}",
                        "[ERROR] Received other response from surfer for load".bright_red()
                    )
                }
            }
        } else {
            eprintln!(
                "{}",
                "[ERROR] Did not receive response for a load from surfer".bright_red()
            );
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn connect_or_start_surfer() -> Option<TcpStream> {
    // 54321 is the default port used by surfer wcp server
    match TcpStream::connect("127.0.0.1:54321") {
        Ok(stream) => {
            eprintln!("[Info] Connecting to Surfer...");
            Some(stream)
        }
        Err(_) => {
            eprintln!("[Info] Starting Surfer...");
            let (port_sender, port_receiver) = channel();
            let (tx, rx) = channel();
            std::thread::spawn(move || {
                let listener =
                    TcpListener::bind("127.0.0.1:0").expect("There should be some free port");
                port_sender
                    .send(listener.local_addr().unwrap().port())
                    .expect("Port number should be sent");
                if let Ok((stream, _)) = listener.accept()
                    && let Ok(_) = tx.send(stream)
                {}
            });
            let Ok(port) = port_receiver.recv() else {
                eprintln!("{}", "[ERROR] Failed to open a socket".bright_red());
                return None;
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
                                eprintln!(
                                    "{}\nstdout: {}\n{} {}",
                                    "[ERROR] Surfer stopped unexpectedly".bright_red(),
                                    stdout,
                                    "stderr:".bright_red(),
                                    stderr
                                );
                            } else {
                                eprintln!("[ERROR] Surfer stopped unexpectedly");
                            }
                        }
                    }
                    Err(e) => eprintln!("{} {e}", "[ERROR] Failed to run surfer:".bright_red()),
                };
            });
            let ret = rx.recv_timeout(Duration::from_secs(1)).ok();
            // if Surfer fails to start above timeout returns None, then we want to close the socket that is waiting in accept
            if ret.is_none() && TcpStream::connect(format!("127.0.0.1:{port}")).is_err() {
                eprintln!("{}", "[ERROR] Failed to cleanup the listener".bright_red());
            }
            ret
        }
    }
}

pub fn open_at_time(trace_path: &str, signals: Vec<String>, time: f64) {
    let mut surfer = CONNECTION
        .get_or_init(|| Mutex::new(Surfer::new(trace_path)))
        .lock()
        .unwrap();
    surfer.load_signals(signals);
    if surfer.commands.contains(&String::from("add_markers")) {
        surfer.send_message(&WcpCSMessage::command(WcpCommand::add_markers {
            markers: vec![MarkerInfo {
                time: BigInt::from_f64(time).expect("Should be valid"),
                name: Some("marker".into()),
                move_focus: true,
            }],
        }));
        surfer.loaded_signals.push("Marker".to_string());
    } else {
        eprintln!("[Info] Surfer version does not support adding markers. Skipping");
    }
}

pub fn open_and_mark_periods(
    trace_path: &str,
    signals: Vec<String>,
    periods: &[(u64, u64)],
    suffix: &str,
    color: &str,
) {
    let mut surfer = CONNECTION
        .get_or_init(|| Mutex::new(Surfer::new(trace_path)))
        .lock()
        .unwrap();
    surfer.load_signals(signals);
    if surfer.commands.contains(&String::from("add_markers")) {
        let mut markers = Vec::with_capacity(periods.len() * 2);
        for (i, (start, end)) in periods.iter().enumerate() {
            let name = format!("start {i} {suffix}");
            markers.push(MarkerInfo {
                time: BigInt::from_u64(*start).expect("Should be valid"),
                name: Some(name),
                move_focus: false,
            });
            let name = format!("end {i} {suffix}");
            markers.push(MarkerInfo {
                time: BigInt::from_u64(*end).expect("Should be valid"),
                name: Some(name),
                move_focus: false,
            });
        }
        let markers_len = markers.len();
        if let Some(response) =
            surfer.send_message(&WcpCSMessage::command(WcpCommand::add_markers { markers }))
        {
            if let WcpSCMessage::response(proto::WcpResponse::add_markers { ids }) = response {
                let ids_len = ids.len();
                if markers_len != ids_len {
                    eprintln!("[WARN] Cannot add more markers in surfer");
                }
                for id in ids {
                    surfer.loaded_signals.push(format!("marker {}", id.0));
                    surfer
                        .send_message_without_response(&WcpCSMessage::command(
                            WcpCommand::set_item_color {
                                id,
                                color: String::from(color),
                            },
                        ))
                        .unwrap()
                }
                if ids_len > 0 {
                    surfer.await_reponse().unwrap();
                }
            } else if let WcpSCMessage::error { message, .. } = response {
                eprintln!("[WARN] Received error from surfer {message}");
            }
        }
    } else {
        eprintln!("[Info] Surfer version does not support adding markers. Skipping");
    }
}

pub fn zoom_to_range(start: u64, end: u64) {
    if let Some(surfer) = CONNECTION.get() {
        let mut surfer = surfer.lock().unwrap();
        if surfer
            .commands
            .contains(&String::from("set_viewport_range"))
        {
            let start = BigInt::from_u64(start).expect("Should be valid");
            let end = BigInt::from_u64(end).expect("Should be valid");
            surfer.send_message(&WcpCSMessage::command(WcpCommand::set_viewport_range {
                start,
                end,
            }));
        } else {
            eprintln!("[Info] Surfer version does not support setting viewport range. Skipping");
        }
    } else {
        eprintln!(
            "{}",
            "[ERROR] Failed to zoom to range: Connection to surfer invalid.".bright_red()
        );
    }
}
