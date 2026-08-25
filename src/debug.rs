use std::io::Write;
use std::num::{NonZeroU32, ParseIntError};
use std::path::PathBuf;
use std::str::FromStr;

use anyhow::Result;

#[derive(Debug)]
pub enum DebuggerAction {
    Info,
    Continue,
    Step,
    FrameAdvance(NonZeroU32),
    Screenshot(Option<PathBuf>),
    SetBreakpoint(BreakpointTarget),
    DeleteBreakpoint(usize),
}

pub enum DebuggerError {
    UnknownAction,
    InvalidValue(String, ParseIntError),
}

fn parse_address(s: &str) -> Result<u16, ParseIntError> {
    if let Some(hex) = s.strip_prefix("0x") {
        u16::from_str_radix(hex, 16)
    } else {
        s.parse()
    }
}

fn parse_frame_count(s: &str) -> Result<NonZeroU32, ParseIntError> {
    if s.is_empty() {
        Ok(NonZeroU32::MIN)
    } else {
        s.parse()
    }
}

impl FromStr for DebuggerAction {
    type Err = DebuggerError;

    fn from_str(s: &str) -> Result<DebuggerAction, Self::Err> {
        let (cmd, rest) = s.split_once(' ').unwrap_or((s, ""));
        let rest = rest.trim();
        let action = match cmd {
            "i" | "info" => DebuggerAction::Info,
            "c" | "continue" => DebuggerAction::Continue,
            "s" | "step" => DebuggerAction::Step,
            "f" | "frame" => {
                let n = parse_frame_count(rest)
                    .map_err(|e| DebuggerError::InvalidValue(rest.to_string(), e))?;
                DebuggerAction::FrameAdvance(n)
            }
            "ss" | "screenshot" => {
                DebuggerAction::Screenshot((!rest.is_empty()).then(|| rest.trim().into()))
            }
            "b" | "break" => match parse_address(rest) {
                Ok(addr) => DebuggerAction::SetBreakpoint(BreakpointTarget::Address(addr)),
                Err(e) => {
                    if let Ok(DebuggerAction::FrameAdvance(n)) = rest.parse() {
                        DebuggerAction::SetBreakpoint(BreakpointTarget::Frames(n))
                    } else {
                        return Err(DebuggerError::InvalidValue(rest.to_string(), e));
                    }
                }
            },
            "d" | "del" | "delete" => match rest.parse() {
                Ok(index) => DebuggerAction::DeleteBreakpoint(index),
                Err(e) => return Err(DebuggerError::InvalidValue(rest.to_string(), e)),
            },
            _ => return Err(DebuggerError::UnknownAction),
        };
        Ok(action)
    }
}

#[derive(Debug)]
pub enum BreakpointTarget {
    Address(u16),
    Frames(NonZeroU32),
}

pub struct Debugger {
    breakpoints: Vec<Option<u16>>,
    trap: bool,
    frames_to_wait: Option<NonZeroU32>,
}

impl Debugger {
    pub fn new() -> Self {
        Self {
            breakpoints: Vec::new(),
            trap: true,
            frames_to_wait: None,
        }
    }

    pub fn next_action(&mut self) -> Option<Result<DebuggerAction>> {
        self.trap.then(|| self.parse_next_action())
    }

    fn parse_next_action(&mut self) -> Result<DebuggerAction> {
        loop {
            let mut input = String::new();
            print!("> ");
            std::io::stdout().flush()?;
            std::io::stdin().read_line(&mut input)?;
            let input = input.trim();
            match input.parse() {
                Ok(action) => return Ok(action),
                Err(e) => match e {
                    DebuggerError::UnknownAction => {
                        println!("Unknown debugger command: {input}");
                    }
                    DebuggerError::InvalidValue(value, e) => {
                        println!("Invalid value `{value}`: {e} - \"{input}\"");
                    }
                },
            }
        }
    }

    pub fn try_break(&mut self, address: u16) {
        if let Some(idx) = self.find_breakpoint(address) {
            println!("Breakpoint {idx} @ {address:#06x}");
            self.trap = true;
        }
    }

    pub fn trap_frame(&mut self) {
        if let Some(frames) = self.frames_to_wait {
            self.frames_to_wait = NonZeroU32::new(frames.get() - 1);
            if self.frames_to_wait.is_none() {
                self.trap = true;
            }
        }
    }

    pub fn untrap(&mut self) {
        self.trap = false;
    }

    pub fn set_breakpoint(&mut self, target: BreakpointTarget) {
        match target {
            BreakpointTarget::Address(address) => {
                if self.find_breakpoint(address).is_none() {
                    self.breakpoints.push(Some(address))
                }
            }
            BreakpointTarget::Frames(n) => self.frames_to_wait = Some(n),
        }
    }

    pub fn delete_breakpoint(&mut self, index: usize) {
        if let Some(breakpoint) = self.breakpoints.get_mut(index) {
            breakpoint.take();
            let end = self.breakpoints.iter().rposition(Option::is_some);
            self.breakpoints.truncate(end.map_or(0, |idx| idx + 1));
        }
    }

    fn find_breakpoint(&mut self, address: u16) -> Option<usize> {
        self.breakpoints.iter().position(|&b| b == Some(address))
    }
}
