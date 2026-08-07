use std::io::Write;
use std::num::ParseIntError;
use std::str::FromStr;

use anyhow::Result;

#[derive(Debug)]
pub enum DebuggerAction {
    Continue,
    Step,
    SetBreakpoint(u16),
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

impl FromStr for DebuggerAction {
    type Err = DebuggerError;

    fn from_str(s: &str) -> Result<DebuggerAction, Self::Err> {
        let (cmd, rest) = s.split_once(' ').unwrap_or((s, ""));
        let action = match cmd {
            "c" => DebuggerAction::Continue,
            "s" => DebuggerAction::Step,
            "b" => match parse_address(rest) {
                Ok(addr) => DebuggerAction::SetBreakpoint(addr),
                Err(e) => return Err(DebuggerError::InvalidValue(rest.to_string(), e)),
            },
            "d" => match rest.parse() {
                Ok(index) => DebuggerAction::DeleteBreakpoint(index),
                Err(e) => return Err(DebuggerError::InvalidValue(rest.to_string(), e)),
            },
            _ => return Err(DebuggerError::UnknownAction),
        };
        Ok(action)
    }
}

pub struct Debugger {
    breakpoints: Vec<Option<u16>>,
    trap: bool,
}

impl Debugger {
    pub fn new() -> Self {
        Self {
            breakpoints: Vec::new(),
            trap: true,
        }
    }

    pub fn handle_action(&mut self) -> Result<()> {
        while self.trap {
            match self.parse_next_action()? {
                DebuggerAction::Continue => self.trap = false,
                DebuggerAction::Step => {}
                DebuggerAction::SetBreakpoint(address) => {
                    if self.find_breakpoint(address).is_none() {
                        self.breakpoints.push(Some(address))
                    }
                }
                DebuggerAction::DeleteBreakpoint(index) => {
                    if let Some(breakpoint) = self.breakpoints.get_mut(index) {
                        breakpoint.take();
                        let end = self.breakpoints.iter().rposition(Option::is_some);
                        self.breakpoints.truncate(end.map_or(0, |idx| idx + 1));
                    }
                }
            }
        }
        Ok(())
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

    pub fn check_breakpoints(&mut self, address: u16) {
        if let Some(idx) = self.find_breakpoint(address) {
            println!("Breakpoint {idx} @ {address:#06x}");
            self.trap = true;
        }
    }

    fn find_breakpoint(&mut self, address: u16) -> Option<usize> {
        self.breakpoints.iter().position(|&b| b == Some(address))
    }
}
