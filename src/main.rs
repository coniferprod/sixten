use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::sync::mpsc::{Sender, Receiver};
use std::thread::{self, JoinHandle};
use std::io::{self, BufRead, BufReader, Write};

enum ReplCommand {
    ListInputs,
    ListOutputs,
    Quit,
}

enum MidiEvent {
    SysEx(Vec<u8>),
    Raw(String),
}

const PROMPT: &str = "610> ";

fn parse_command(line: &str) -> Option<ReplCommand> {
    let line = line.trim();
    if line.is_empty() {
        return None;
    }

    match line {
        "quit" => Some(ReplCommand::Quit),

        cmd if cmd.starts_with("list") => {
            let parts: Vec<&str> = cmd.split(' ').collect();
            let param = parts[1];
            if param == "inputs" {
                Some(ReplCommand::ListInputs)
            } else if param == "outputs" {
                Some(ReplCommand::ListOutputs)
            } else {
                println!("usage: list inputs | outputs");
                None
            }
        }

        _ => {
            println!("Unknown command");
            None
        }
    }
}

fn spawn_repl_thread(tx: Sender<Event>) -> JoinHandle<()> {
    thread::spawn(move || {
        let stdin = io::stdin();
        loop {
            print!("{}", PROMPT);
            io::stdout().flush().unwrap();

            let mut line = String::new();
            stdin.read_line(&mut line).unwrap();

            let line = line.trim();
            match parse_command(line) {
                Some(cmd) => {
                    tx.send(Event::ReplCommand(cmd)).unwrap();
                },

                None => {
                    // ignore empty/invalid input
                }
            }
        }
    })
}

fn spawn_midi_receive_thread(tx: Sender<Event>) -> JoinHandle<()> {
    thread::spawn(move || {
        let mut child = Command::new("receivemidi")
            .arg("dev")
            .arg("WM-1 Bluetooth")
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();

        let stdout = child.stdout.take().unwrap();
        let reader = BufReader::new(stdout);
        for line in reader.lines() {
            let line = line.unwrap();
            let event = parse_midi_line(&line);
            tx.send(Event::MidiEvent(event)).unwrap();
        }
    })
}

fn parse_midi_line(line: &str) -> MidiEvent {
    if line.starts_with("syx") {
        //let parts = line.split(' ').collect();
        // TODO: parse SysEx message data into vector
        MidiEvent::SysEx(Vec::new())
    } else {
        MidiEvent::Raw(String::from(line))
    }
}

const SEND_MIDI: &str = "sendmidi";
const RECEIVE_MIDI: &str = "receivemidi";

enum Event {
    ReplCommand(ReplCommand),
    MidiEvent(MidiEvent),
}

struct Sixten {
    // List of MIDI inputs
    inputs: Vec<String>,

    // List of MIDI outputs
    outputs: Vec<String>,

    event_rx: Receiver<Event>,

    should_quit: bool,
}

impl Sixten {
    fn new() -> Self {
        let (tx, rx) = mpsc::channel();
        spawn_repl_thread(tx.clone());
        spawn_midi_receive_thread(tx.clone());
        Self {
            inputs: Vec::new(),
            outputs: Vec::new(),
            event_rx: rx,
            should_quit: false,
        }
    }

    fn run(&mut self) {
        while let Ok(event) = self.event_rx.recv() {
            self.handle_event(&event);

            if self.should_quit {
                break;
            }
        }

        println!("Quitting...");
        std::process::exit(0);
    }

    fn handle_event(&mut self, event: &Event) {
        match event {
            Event::ReplCommand(cmd) => {
                self.handle_command(cmd);
            }

            Event::MidiEvent(msg) => {
                self.handle_midi(msg);
            }
        }
    }

    fn handle_command(&mut self, cmd: &ReplCommand) {
        match cmd {
            ReplCommand::Quit => {
                self.should_quit = true;
                return;
            }

            ReplCommand::ListInputs => {
                let child_output = Command::new(RECEIVE_MIDI)
                    .arg("list")
                    .output()
                    .expect("should have captured process output");
                let child_output_text = String::from_utf8(child_output.stdout).unwrap();
                for (index, raw_line) in child_output_text.lines().enumerate() {
                    println!("{}: {}", index, raw_line);
                    self.inputs.push(raw_line.to_string());
                }
            }

            ReplCommand::ListOutputs => {
                let child_output = Command::new(SEND_MIDI)
                    .arg("list")
                    .output()
                    .expect("should have captured process output");
                let child_output_text = String::from_utf8(child_output.stdout).unwrap();
                for (index, raw_line) in child_output_text.lines().enumerate() {
                    println!("{}: {}", index, raw_line);
                    self.outputs.push(raw_line.to_string());
                }
            }
        }

        //print!("{}", PROMPT);
        //io::stdout().flush().unwrap();
    }

    fn handle_midi(&self, msg: &MidiEvent) {
        match msg {
            MidiEvent::SysEx(data) => {
                println!("SysEx data length = {}", data.len());
            }

            MidiEvent::Raw(line) => {
                if !line.starts_with("midi-clock") {
                    println!("{}", line);

                    print!("{}", PROMPT);
                    io::stdout().flush().unwrap();
                }
            }
        }
    }
}

fn main() -> Result<(), std::io::Error> {
    let mut sixten = Sixten::new();
    sixten.run();

    Ok(())
}
