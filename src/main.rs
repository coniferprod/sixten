use std::process::{Command, Stdio};
use std::sync::mpsc;
//use std::sync::mpsc::{Sender};
use std::thread::{self, JoinHandle};
use std::io::{self, BufRead, BufReader, Write};

use crossbeam_channel::{unbounded, Sender, Receiver, select};

enum ReplCommand {
    ListInputs,
    ListOutputs,
    Quit,
}

enum MidiEvent {
    SysEx(Vec<u8>),
    Raw(String),
}

fn spawn_repl_thread(tx: Sender<ReplCommand>) -> JoinHandle<()> {
    thread::spawn(move || {
        let stdin = io::stdin();
        loop {
            print!("610> ");
            io::stdout().flush().unwrap();

            let mut line = String::new();
            stdin.read_line(&mut line).unwrap();

            let line = line.trim();
            match line {
                "quit" => {
                    tx.send(ReplCommand::Quit).unwrap();
                    break;
                }

                cmd if cmd.starts_with("list") => {
                    let parts: Vec<&str> = cmd.split(' ').collect();
                    let param = parts[1];
                    if param == "inputs" {
                        tx.send(ReplCommand::ListInputs).unwrap();
                    } else if param == "outputs" {
                        tx.send(ReplCommand::ListOutputs).unwrap();
                    } else {
                        println!("usage: list inputs | outputs");
                    }
                }

                _ => {
                    println!("unknown command: {}", line);
                }
            }
        }
    })
}

fn spawn_midi_receive_thread(tx: Sender<MidiEvent>) -> JoinHandle<()> {
    thread::spawn(move || {
        let mut child = Command::new("receivemidi")
            .arg("dev")
            .arg("") // TODO: device name here
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();

        let stdout = child.stdout.take().unwrap();
        let reader = BufReader::new(stdout);
        for line in reader.lines() {
            let line = line.unwrap();
            let event = parse_midi_line(&line);
            tx.send(event).unwrap();
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

fn main_loop(repl_rx: Receiver<ReplCommand>, midi_rx: Receiver<MidiEvent>) {
    loop {
        select! {
            recv(repl_rx) -> cmd => {
                match cmd.unwrap() {
                    ReplCommand::Quit => {
                        break;
                    }

                    ReplCommand::ListInputs => {
                        println!("listing inputs");
                    }

                    ReplCommand::ListOutputs => {
                        println!("listing outputs");
                    }
                }
            }

            recv(midi_rx) -> msg => {
                match msg.unwrap() {
                    MidiEvent::SysEx(data) => {
                        println!("SysEx data length = {}", data.len());
                    }

                    MidiEvent::Raw(line) => {
                        println!("midi: {}", line);
                    }
                }
            }

        }
    }
}

const SEND_MIDI: &str = "sendmidi";
const RECEIVE_MIDI: &str = "receivemidi";


fn main() -> Result<(), std::io::Error> {
    //let (midi_tx, midi_rx) = mpsc::channel::<MidiEvent>();
    let (midi_tx, midi_rx) = unbounded();
    let receiver = spawn_midi_receive_thread(midi_tx);

    //let (repl_tx, repl_rx) = mpsc::channel::<ReplCommand>();
    let (repl_tx, repl_rx) = unbounded();

    let repl = spawn_repl_thread(repl_tx);

    main_loop(repl_rx, midi_rx);

    Ok(())
}
