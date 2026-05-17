# Sixten

A REPL with a cause

## MIDI System Exclusive REPL

Sixten is a REPL for sending MIDI messages to synthesizers, and
for receiving them. In computing, the term REPL stands for 
"Read-Evaluate-Print Loop". It is a simple, interactive command
environment, much like the operating system shell.

Sixten specializes in MIDI System Exclusive (SysEx) messages, which let you
control the sound content and operation of any synthesizer that
supports this technology. Most of the older digital synthesizers,
but also some newer analog hybrids, can be controlled with SysEx.
This control ranges from sending parameter changes to uploading
or downloading complete sound patches.

Sixten will support various existing synthesizers. The support 
depends on the libraries that the program uses.

## Documentation

There is no comprehensive documentation yet. Enter the `help`
command at the Sixten command prompt to get a list of the
available commands.

## Personal use

Sixten is developed for personal use, so it does not have packaged
releases, at least for the time being. Instead, you will need the Rust
development tools to prepare an executable for your system.

Sixten was primarily developed in and for macOS, but in theory
it should work well in Linux, since there are no macOS-specific
dependencies.

## Dependencies

Initially Sixten uses the SendMIDI and ReceiveMIDI programs invoked
as external processes. If MIDI libraries for Rust turn out to be able
to handle long SysEx messages properly, this dependency may be removed.
For the time being you will need to have these programs in your
`PATH` so that they can be invoked by Sixten.

## Copyright

Copyright (C) 2026 Conifer Productions Oy. Licensed under the
MIT License (see LICENSE file in this repository).

