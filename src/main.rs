use std::collections::HashMap;
use std::process::{Child, Command, Stdio};
use std::sync::mpsc;
use std::sync::mpsc::{Sender, Receiver};
use std::thread::{self, JoinHandle};
use std::io::{self, BufRead, BufReader, Write};
use std::fmt;
use std::str::FromStr;

use hex;

enum ReplCommand {
    Set(String, String),  // set named variable to value
    ListInputs,
    ListOutputs,
    Quit,
    Status,  // shows the environment
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

        cmd if cmd.starts_with("set ") => {
            let parts: Vec<&str> = cmd.split(' ').collect();
            if parts.len() != 3 {
                println!("usage: set variable value");
                None
            } else {
                let variable_name = parts[1];
                let new_value = parts[2];
                Some(ReplCommand::Set(variable_name.to_string(), new_value.to_string()))
            }
        }

        cmd if cmd.starts_with("list ") => {
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

        cmd if cmd.starts_with("status") => {
            Some(ReplCommand::Status)
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

struct MidiReceiver {
    join_handle: Option<JoinHandle<()>>,
    child: Child,
}

impl MidiReceiver {
    fn stop(&mut self) {
        self.child.kill().ok();
        if let Some(handle) = self.join_handle.take() {
            handle.join().ok();
        }
    }
}

fn spawn_midi_receive_thread(tx: Sender<Event>, device_name: String) -> MidiReceiver {
    let mut child = Command::new("receivemidi")
        .arg("dev")
        .arg(device_name)
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();

    let stdout = child.stdout.take().unwrap();

    let join_handle = thread::spawn(move || {
        let reader = BufReader::new(stdout);
        for line in reader.lines() {
            let line = line.unwrap();
            let event = parse_midi_line(&line);
            tx.send(Event::MidiEvent(event)).unwrap();
        }
    });

    MidiReceiver {
        child,
        join_handle: Some(join_handle),
    }
}

fn parse_midi_line(line: &str) -> MidiEvent {
    if line.starts_with("system-exclusive") {
        //let offset = "system-exclusive hex ".len();
        //let mut data_string = line[offset..].to_string();
        //data_string.retain(|c| !c.is_whitespace());

        // part 0 is "system-exclusive"
        // part 1 is "hex"
        // parts 2..len-1 are the hex string bytes
        // last part is "dec"

        let parts: Vec<&str> = line.split(' ').collect();
        // Drop the first two parts ("system-exclusive hex") 
        // and the last one ("dec")
        let hex_parts = &parts[2..parts.len()-1];  

        // Make one big hex string with no spaces
        let mut hex_string = String::new();
        for h in hex_parts {
            hex_string.push_str(h);
        }

        // We trust the data since it comes from ReceiveMIDI
        let data = hex::decode(hex_string).unwrap();

        MidiEvent::SysEx(data)
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

fn get_inputs() -> Vec<String> {
    let mut result = Vec::new();

    let child_output = Command::new(RECEIVE_MIDI)
        .arg("list")
        .output()
        .expect("should have captured process output");
    let child_output_text = String::from_utf8(child_output.stdout).unwrap();
    for (index, raw_line) in child_output_text.lines().enumerate() {
        println!("{}: {}", index, raw_line);
        result.push(raw_line.to_string());
    }

    result
}

/// Represents a synthesizer known by Sixten.
struct Synth {
    make: String,
    model: String,
}

impl fmt::Display for Synth {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}/{}", self.make, self.model)
    }
}

impl FromStr for Synth {
    type Err = String;
    
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let s = s.trim();
        if s.is_empty() {
            return Err(String::from("empty category"));
        }

        let parts: Vec<&str> = s.split("/").collect();
        if parts.len() == 2 {
            Ok(Synth {
                make: parts[0].to_string(),
                model: parts[1].to_string(),
            })
        } else {
            Err(String::from("use the make/model format"))
        }
    }
}

struct Variables {
    input: usize,
    synth: Option<Synth>,  // current synthesizer
}

struct Sixten {
    // List of MIDI inputs
    inputs: Vec<String>,

    // List of MIDI outputs
    outputs: Vec<String>,

    event_tx: Sender<Event>,
    event_rx: Receiver<Event>,
    midi_receiver: MidiReceiver,

    should_quit: bool,  // will be set to true by the 'quit' command

    variables: Variables,  // status variables
}

impl Sixten {
    fn new(inputs: &Vec<String>) -> Self {
        // Initialize the "input" variable with the index of the first input.
        let variables = Variables { 
            input: 0,
            synth: None,
        };

        let (tx, rx) = mpsc::channel();
        spawn_repl_thread(tx.clone());
        let midi_receiver = spawn_midi_receive_thread(tx.clone(), inputs[0].clone());
        Self {
            inputs: inputs.to_vec(),
            outputs: Vec::new(),
            event_tx: tx,
            event_rx: rx,
            midi_receiver,
            should_quit: false,
            variables,
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

            ReplCommand::Status => {
                let index = self.variables.input;
                println!("input = {}: {}", index, self.inputs[index]);
                match &self.variables.synth {
                    Some(synth) => println!("synth = {}", synth),
                    None => {},
                }
            }

            ReplCommand::Set(variable_name, new_value) => {
                match variable_name.as_str() {
                    "input" => {
                        let index = new_value.parse().unwrap();
                        self.variables.input = index;
                        let device_name = self.inputs[index].clone();
                        println!("Input changed, now '{}'. Stopping MIDI receiver and respawning thread", device_name);
                        self.midi_receiver.stop();

                        self.midi_receiver = spawn_midi_receive_thread(self.event_tx.clone(), device_name);
                    },
                    "synth" => {
                        match Synth::from_str(new_value) {
                            Ok(synth) => self.variables.synth = Some(synth),
                            Err(e) => eprintln!("{}", e),
                        }
                    }
                    _ => eprintln!("unknown variable '{}'", variable_name),
                }
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

fn main() -> Result<(), &'static str> {
    // Get the list of MIDI inputs.
    // If there are none, just say it and quit.
    let inputs = get_inputs();
    if inputs.is_empty() {
        let message = "Error: no MIDI inputs";
        eprintln!("{}", message);
        return Err(message);
    }

    println!("MIDI inputs:");
    for name in &inputs {
        println!("{}", name);
    }

    let mut sixten = Sixten::new(&inputs);
    sixten.run();

    Ok(())
}
