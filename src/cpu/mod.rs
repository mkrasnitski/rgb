mod instruction;
mod registers;

use crate::bus::MemoryBus;
use crate::utils::BitExtract;
use anyhow::{bail, Result};
use instruction::*;
use num_traits::FromPrimitive;
use registers::{Reg16, Reg8, RegWrite, Registers};

pub struct Cpu {
    registers: Registers,
    memory: MemoryBus,
    cycles: u64,
    ime: bool,
    halted: bool,
}

impl Cpu {
    pub fn new(bootrom: [u8; 0x100], cartridge: Vec<u8>) -> Self {
        let memory = MemoryBus::new(bootrom, cartridge);
        let mut cpu = Self {
            memory,
            registers: Registers::default(),
            cycles: 0,
            ime: false,
            halted: false,
        };

        cpu.registers.pc = 0x100;
        cpu
    }

    pub fn run(mut self) -> Result<()> {
        loop {
            self.check_for_interrupts();
            if self.halted {
                self.mtick();
            } else {
                let instr = self.decode_instr()?;
                let len = instr.length() as u16;
                #[cfg(debug_assertions)]
                {
                    let bytes = {
                        (self.registers.pc..self.registers.pc + len)
                            .map(|addr| format!("{:02x}", self.memory.read(addr)))
                            .collect::<Vec<_>>()
                            .join(" ")
                    };
                    println!(
                        "{} | {:04X} | {bytes:>8} | {:?} | {instr:?}",
                        self.cycles,
                        self.registers.pc,
                        instr.mcycles(),
                    );
                }
                self.registers.pc += len;
                let cycles = self.execute_instr(instr);
                for _ in 0..cycles {
                    self.mtick();
                }
            }
        }
    }

    fn check_for_interrupts(&mut self) {
        let int = self.memory.int_flag & self.memory.int_enable;
        for i in 0..5 {
            if int.bit(i) {
                self.halted = false;
                if self.ime {
                    self.memory.int_flag &= !(1 << i);
                    self.ime = false;
                    self.push16(self.registers.pc);
                    self.registers.pc = 0x40 + (i << 3) as u16;
                    break;
                }
            }
        }
    }

    fn mtick(&mut self) {
        if self.memory.timers.increment() {
            self.request_interrupt(2);
        }
        self.cycles += 1;
    }

    fn request_interrupt(&mut self, int: u8) {
        if int < 5 {
            self.memory
                .write(0xff0f, self.memory.read(0xff0f) | 1 << int);
        } else {
            unreachable!()
        }
    }

    fn decode_instr(&self) -> Result<Instruction> {
        let byte = self.memory.read(self.registers.pc);
        let lo_3bit = byte & 0b111;
        let hi_3bit = (byte & 0b111111) >> 3;
        let hi_2bit = hi_3bit >> 1;
        let r8_lo = R8::from_u8(lo_3bit).unwrap();
        let r8_hi = R8::from_u8(hi_3bit).unwrap();
        let r16 = R16::from_u8(hi_2bit).unwrap();
        let ind = Indirect::from_u8(hi_2bit).unwrap();
        let branch = BranchCond::from_u8(hi_3bit & 0b11).unwrap();
        let push_pop = PushPop::from_u8(hi_2bit).unwrap();
        Ok(match byte {
            0x00 => Instruction::Nop,
            0x01 | 0x11 | 0x21 | 0x31 => Instruction::Ld(LdType::R16Imm(r16, self.u16_arg())),
            0x02 | 0x12 | 0x22 | 0x32 => Instruction::Ld(LdType::IndFromA(ind)),
            0x03 | 0x13 | 0x23 | 0x33 => Instruction::IncR16(r16),
            0x04 | 0x14 | 0x24 | 0x34 | 0x0c | 0x1c | 0x2c | 0x3c => Instruction::IncR8(r8_hi),
            0x05 | 0x15 | 0x25 | 0x35 | 0x0d | 0x1d | 0x2d | 0x3d => Instruction::DecR8(r8_hi),
            0x06 | 0x16 | 0x26 | 0x36 | 0x0e | 0x1e | 0x2e | 0x3e => {
                Instruction::Ld(LdType::R8Imm(r8_hi, self.u8_arg()))
            }
            0x08 => Instruction::Ld(LdType::StoreSP(self.u16_arg())),
            0x09 | 0x19 | 0x29 | 0x39 => Instruction::AddHL(r16),
            0x0a | 0x1a | 0x2a | 0x3a => Instruction::Ld(LdType::AFromInd(ind)),
            0x0b | 0x1b | 0x2b | 0x3b => Instruction::DecR16(r16),
            0x20 | 0x28 | 0x30 | 0x38 => Instruction::Jr(branch, self.u8_arg() as i8),
            0x40..=0x75 | 0x77..=0x7f => Instruction::Ld(LdType::R8(r8_hi, r8_lo)),
            0x80..=0xbf => self.decode_alu_instr(byte, AluSrc::R8(r8_lo)),
            0xc0 | 0xd0 | 0xc8 | 0xd8 => Instruction::Ret(branch),
            0xc1 | 0xd1 | 0xe1 | 0xf1 => Instruction::Pop(push_pop),
            0xc2 | 0xd2 | 0xca | 0xda => Instruction::Jp(branch, self.u16_arg()),
            0xc4 | 0xd4 | 0xcc | 0xdc => Instruction::Call(branch, self.u16_arg()),
            0xc5 | 0xd5 | 0xe5 | 0xf5 => Instruction::Push(push_pop),
            0xc6 | 0xd6 | 0xe6 | 0xf6 | 0xce | 0xde | 0xee | 0xfe => {
                self.decode_alu_instr(byte, AluSrc::Imm(self.u8_arg()))
            }
            0xc7 | 0xd7 | 0xe7 | 0xf7 | 0xcf | 0xdf | 0xef | 0xff => Instruction::Rst(hi_3bit << 3),
            0xcb => {
                let second_byte = self.u8_arg();
                let bit = BitPos::from_u8((second_byte & 0b111111) >> 3).unwrap();
                let r8 = R8::from_u8(second_byte & 0b111).unwrap();
                match second_byte {
                    0x00..=0x07 => Instruction::Rlc(r8),
                    0x08..=0x0f => Instruction::Rrc(r8),
                    0x10..=0x17 => Instruction::Rl(r8),
                    0x18..=0x1f => Instruction::Rr(r8),
                    0x20..=0x27 => Instruction::Sla(r8),
                    0x28..=0x2f => Instruction::Sra(r8),
                    0x30..=0x37 => Instruction::Swap(r8),
                    0x38..=0x3f => Instruction::Srl(r8),
                    0x40..=0x7f => Instruction::Bit(bit, r8),
                    0x80..=0xbf => Instruction::Res(bit, r8),
                    0xc0..=0xff => Instruction::Set(bit, r8),
                }
            }
            0xe0 => Instruction::Ld(LdType::IoRegFromA(Io::Imm(self.u8_arg()))),
            0xe2 => Instruction::Ld(LdType::IoRegFromA(Io::C)),
            0xea => Instruction::Ld(LdType::MemFromA(self.u16_arg())),
            0xf0 => Instruction::Ld(LdType::AFromIoReg(Io::Imm(self.u8_arg()))),
            0xf2 => Instruction::Ld(LdType::AFromIoReg(Io::C)),
            0xfa => Instruction::Ld(LdType::AFromMem(self.u16_arg())),

            0x07 => Instruction::Rlca,
            0x0f => Instruction::Rrca,
            0x10 => Instruction::Stop,
            0x17 => Instruction::Rla,
            0x18 => Instruction::JrAlways(self.u8_arg() as i8),
            0x1f => Instruction::Rra,
            0x27 => Instruction::Daa,
            0x2f => Instruction::Cpl,
            0x37 => Instruction::Scf,
            0x3f => Instruction::Ccf,
            0x76 => Instruction::Halt,
            0xc3 => Instruction::JpAlways(self.u16_arg()),
            0xc9 => Instruction::RetAlways,
            0xcd => Instruction::CallAlways(self.u16_arg()),
            0xd9 => Instruction::Reti,
            0xe8 => Instruction::AddSP(self.u8_arg() as i8),
            0xe9 => Instruction::JpHL,
            0xf3 => Instruction::Di,
            0xfb => Instruction::Ei,
            0xf8 => Instruction::Ld(LdType::HLFromSP(self.u8_arg() as i8)),
            0xf9 => Instruction::Ld(LdType::SPFromHL),

            0xd3 | 0xdb | 0xdd | 0xe3 | 0xe4 | 0xeb | 0xec | 0xed | 0xf4 | 0xfc | 0xfd => {
                bail!("Crash opcode: {:02x}", byte)
            }
        })
    }

    fn execute_instr(&mut self, instr: Instruction) -> u64 {
        let mut branch_taken = false;
        match instr {
            Instruction::Nop => (),

            Instruction::Ld(ld_type) => match ld_type {
                LdType::R8(dest, src) => self.move8(dest, src),
                LdType::R8Imm(dest, val) => self.write8(dest, val),
                LdType::R16Imm(dest, val) => self.write16(dest, val),
                LdType::AFromInd(src) => {
                    let val = self.read_ind(src.into());
                    self.write8(R8::A, val);
                }
                LdType::IndFromA(dest) => {
                    let a = self.read8(R8::A);
                    self.write_ind(dest.into(), a);
                }
                LdType::AFromIoReg(src) => {
                    let val = self.read_io(src);
                    self.write8(R8::A, val);
                }
                LdType::IoRegFromA(dest) => {
                    let val = self.read8(R8::A);
                    self.write_io(dest, val);
                }
                LdType::AFromMem(addr) => {
                    let val = self.memory.read(addr);
                    self.write8(R8::A, val);
                }
                LdType::MemFromA(addr) => {
                    let val = self.read8(R8::A);
                    self.memory.write(addr, val);
                }
                LdType::StoreSP(addr) => {
                    let [lsb, msb] = self.read16(R16::SP).to_le_bytes();
                    self.memory.write(addr, lsb);
                    self.memory.write(addr + 1, msb);
                }
                LdType::HLFromSP(offset) => {
                    let (result, h, c) = self.read16(R16::SP).half_overflowing_add_signed(offset);
                    self.registers.write(RegWrite::HL(result));
                    self.set_flags(Some(false), Some(false), Some(h), Some(c));
                }
                LdType::SPFromHL => {
                    let val = self.read16(R16::HL);
                    self.registers.write(RegWrite::SP(val));
                }
            },

            Instruction::IncR8(r8) => {
                let (val, h, _) = self.read8(r8).half_overflowing_add(1);
                self.write8(r8, val);
                self.set_flags(Some(val == 0), Some(false), Some(h), None);
            }
            Instruction::DecR8(r8) => {
                let (val, h, _) = self.read8(r8).half_overflowing_sub(1);
                self.write8(r8, val);
                self.set_flags(Some(val == 0), Some(true), Some(h), None);
            }
            Instruction::IncR16(r16) => {
                let val = self.read16(r16).wrapping_add(1);
                self.write16(r16, val);
            }
            Instruction::DecR16(r16) => {
                let val = self.read16(r16).wrapping_sub(1);
                self.write16(r16, val);
            }

            Instruction::Add(src) => {
                let val = self.read_alu(src);
                let a = self.read8(R8::A);
                let (result, h, c) = a.half_overflowing_add(val);
                self.write8(R8::A, result);
                self.set_flags(Some(result == 0), Some(false), Some(h), Some(c));
            }
            Instruction::Adc(src) => {
                let val = self.read_alu(src);
                let a = self.read8(R8::A);
                let (result, h1, c1) = a.half_overflowing_add(self.registers.flags.c as u8);
                let (result, h2, c2) = result.half_overflowing_add(val);
                self.write8(R8::A, result);
                self.set_flags(Some(result == 0), Some(false), Some(h1 | h2), Some(c1 | c2));
            }
            Instruction::Sub(src) => {
                let val = self.read_alu(src);
                let a = self.read8(R8::A);
                let (result, h, c) = a.half_overflowing_sub(val);
                self.write8(R8::A, result);
                self.set_flags(Some(result == 0), Some(true), Some(h), Some(c));
            }
            Instruction::Sbc(src) => {
                let val = self.read_alu(src);
                let a = self.read8(R8::A);
                let (result, h1, c1) = a.half_overflowing_sub(self.registers.flags.c as u8);
                let (result, h2, c2) = result.half_overflowing_sub(val);
                self.write8(R8::A, result);
                self.set_flags(Some(result == 0), Some(true), Some(h1 | h2), Some(c1 | c2));
            }
            Instruction::And(src) => {
                let val = self.read_alu(src);
                let a = self.read8(R8::A);
                let result = a & val;
                self.write8(R8::A, result);
                self.set_flags(Some(result == 0), Some(false), Some(true), Some(false));
            }
            Instruction::Xor(src) => {
                let val = self.read_alu(src);
                let a = self.read8(R8::A);
                let result = a ^ val;
                self.write8(R8::A, result);
                self.set_flags(Some(result == 0), Some(false), Some(false), Some(false));
            }
            Instruction::Or(src) => {
                let val = self.read_alu(src);
                let a = self.read8(R8::A);
                let result = a | val;
                self.write8(R8::A, result);
                self.set_flags(Some(result == 0), Some(false), Some(false), Some(false));
            }
            Instruction::Cp(src) => {
                let val = self.read_alu(src);
                let a = self.read8(R8::A);
                let (result, h, c) = a.half_overflowing_sub(val);
                self.set_flags(Some(result == 0), Some(true), Some(h), Some(c));
            }

            Instruction::AddHL(src) => {
                let val = self.read16(src);
                let hl = self.read16(R16::HL);
                let (result, h, c) = val.half_overflowing_add(hl);
                self.write16(R16::HL, result);
                self.set_flags(None, Some(false), Some(h), Some(c));
            }
            Instruction::AddSP(offset) => {
                let (result, h, c) = self.read16(R16::SP).half_overflowing_add_signed(offset);
                self.registers.write(RegWrite::SP(result));
                self.set_flags(Some(false), Some(false), Some(h), Some(c));
            }

            Instruction::Rlc(r8) => {
                let val = self.read8(r8);
                let result = val.rotate_left(1);
                self.write8(r8, result);
                self.set_flags(
                    Some(result == 0),
                    Some(false),
                    Some(false),
                    Some(val.bit(7)),
                );
            }
            Instruction::Rrc(r8) => {
                let val = self.read8(r8);
                let result = val.rotate_right(1);
                self.write8(r8, result);
                self.set_flags(
                    Some(result == 0),
                    Some(false),
                    Some(false),
                    Some(val.bit(0)),
                );
            }
            Instruction::Rl(r8) => {
                let val = self.read8(r8);
                let result = (val << 1) | (self.registers.flags.c as u8);
                self.write8(r8, result);
                self.set_flags(
                    Some(result == 0),
                    Some(false),
                    Some(false),
                    Some(val.bit(7)),
                );
            }
            Instruction::Rr(r8) => {
                let val = self.read8(r8);
                let result = (val >> 1) | ((self.registers.flags.c as u8) << 7);
                self.write8(r8, result);
                self.set_flags(
                    Some(result == 0),
                    Some(false),
                    Some(false),
                    Some(val.bit(0)),
                );
            }
            Instruction::Sla(r8) => {
                let val = self.read8(r8);
                let result = val << 1;
                self.write8(r8, result);
                self.set_flags(
                    Some(result == 0),
                    Some(false),
                    Some(false),
                    Some(val.bit(7)),
                );
            }
            Instruction::Sra(r8) => {
                let val = self.read8(r8);
                let result = ((val as i8) >> 1) as u8;
                self.write8(r8, result);
                self.set_flags(
                    Some(result == 0),
                    Some(false),
                    Some(false),
                    Some(val.bit(0)),
                );
            }
            Instruction::Swap(r8) => {
                let val = self.read8(r8);
                let result = val.rotate_right(4);
                self.write8(r8, result);
                self.set_flags(Some(result == 0), Some(false), Some(false), Some(false));
            }
            Instruction::Srl(r8) => {
                let val = self.read8(r8);
                let result = val >> 1;
                self.write8(r8, result);
                self.set_flags(
                    Some(result == 0),
                    Some(false),
                    Some(false),
                    Some(val.bit(0)),
                );
            }
            Instruction::Bit(pos, r8) => {
                let val = self.read8(r8);
                let bit = (val >> (pos as u8)) & 1;
                self.set_flags(Some(bit == 0), Some(false), Some(true), None);
            }
            Instruction::Res(pos, r8) => {
                let val = self.read8(r8);
                self.write8(r8, val & (!(1 << (pos as u8))));
            }
            Instruction::Set(pos, r8) => {
                let val = self.read8(r8);
                self.write8(r8, val | (1 << (pos as u8)));
            }

            Instruction::Rlca => {
                let val = self.read8(R8::A);
                let result = val.rotate_left(1);
                self.write8(R8::A, result);
                self.set_flags(Some(false), Some(false), Some(false), Some(val.bit(7)));
            }
            Instruction::Rla => {
                let val = self.read8(R8::A);
                let result = (val << 1) | (self.registers.flags.c as u8);
                self.write8(R8::A, result);
                self.set_flags(Some(false), Some(false), Some(false), Some(val.bit(7)));
            }
            Instruction::Rrca => {
                let val = self.read8(R8::A);
                let result = val.rotate_right(1);
                self.write8(R8::A, result);
                self.set_flags(Some(false), Some(false), Some(false), Some(val.bit(0)));
            }
            Instruction::Rra => {
                let val = self.read8(R8::A);
                let result = (val >> 1) | ((self.registers.flags.c as u8) << 7);
                self.write8(R8::A, result);
                self.set_flags(Some(false), Some(false), Some(false), Some(val.bit(0)));
            }

            Instruction::Jr(cond, offset) => {
                if self.read_branch_cond(cond) {
                    branch_taken = true;
                    self.registers.pc = self.registers.pc.wrapping_add_signed(offset as i16);
                }
            }
            Instruction::JrAlways(offset) => {
                self.registers.pc = self.registers.pc.wrapping_add_signed(offset as i16);
            }
            Instruction::Jp(cond, addr) => {
                if self.read_branch_cond(cond) {
                    branch_taken = true;
                    self.registers.pc = addr;
                }
            }
            Instruction::JpAlways(addr) => self.registers.pc = addr,
            Instruction::Call(cond, addr) => {
                if self.read_branch_cond(cond) {
                    branch_taken = true;
                    self.push16(self.registers.pc);
                    self.registers.pc = addr;
                }
            }
            Instruction::CallAlways(addr) => {
                self.push16(self.registers.pc);
                self.registers.pc = addr;
            }
            Instruction::Ret(cond) => {
                if self.read_branch_cond(cond) {
                    branch_taken = true;
                    self.registers.pc = self.pop16();
                }
            }
            Instruction::RetAlways => self.registers.pc = self.pop16(),
            Instruction::Rst(addr) => {
                self.push16(self.registers.pc);
                self.registers.pc = addr as u16;
            }
            Instruction::Reti => {
                self.registers.pc = self.pop16();
                self.ime = true;
            }
            Instruction::JpHL => self.registers.pc = self.registers.reg16(Reg16::HL),

            Instruction::Push(src) => {
                let val = self.read_push_pop(src);
                self.push16(val);
            }
            Instruction::Pop(dest) => {
                let val = self.pop16();
                self.write_push_pop(dest, val);
            }

            Instruction::Scf => self.set_flags(None, Some(false), Some(false), Some(true)),
            Instruction::Ccf => {
                let c = self.registers.flags.c;
                self.set_flags(None, Some(false), Some(false), Some(!c));
            }

            Instruction::Daa => {
                let mut val = self.read8(R8::A);
                // addition
                if !self.registers.flags.n {
                    if self.registers.flags.c || val > 0x99 {
                        self.set_flags(None, None, None, Some(true));
                        val = val.wrapping_add(0x60);
                    }
                    if self.registers.flags.h || (val & 0xf) > 0x9 {
                        val = val.wrapping_add(0x06);
                    }
                }
                // subtraction
                else {
                    if self.registers.flags.c {
                        val = val.wrapping_sub(0x60);
                    }
                    if self.registers.flags.h {
                        val = val.wrapping_sub(0x06);
                    }
                };
                self.write8(R8::A, val);
                self.set_flags(Some(val == 0), None, Some(false), None);
            }
            Instruction::Cpl => {
                let val = self.read8(R8::A);
                let result = !val;
                self.write8(R8::A, result);
                self.set_flags(None, Some(true), Some(true), None);
            }

            Instruction::Di => self.ime = false,
            Instruction::Ei => self.ime = true,

            Instruction::Halt => self.halted = true,
            Instruction::Stop => {
                panic!("Unimplemented instruction: {:?}", instr)
            }
        }

        match instr.mcycles() {
            CycleCount::Const(c) => c,
            CycleCount::Branch(not_taken, taken) => {
                if branch_taken {
                    taken
                } else {
                    not_taken
                }
            }
        }
    }

    fn decode_alu_instr(&self, byte: u8, src: AluSrc) -> Instruction {
        match (byte & 0b111111) >> 3 {
            0 => Instruction::Add(src),
            1 => Instruction::Adc(src),
            2 => Instruction::Sub(src),
            3 => Instruction::Sbc(src),
            4 => Instruction::And(src),
            5 => Instruction::Xor(src),
            6 => Instruction::Or(src),
            7 => Instruction::Cp(src),
            _ => unreachable!(),
        }
    }

    fn u8_arg(&self) -> u8 {
        self.memory.read(self.registers.pc + 1)
    }

    fn u16_arg(&self) -> u16 {
        u16::from_le_bytes([
            self.memory.read(self.registers.pc + 1),
            self.memory.read(self.registers.pc + 2),
        ])
    }

    fn read8(&mut self, r8: R8) -> u8 {
        match r8 {
            R8::A => self.registers.reg8(Reg8::A),
            R8::B => self.registers.reg8(Reg8::B),
            R8::C => self.registers.reg8(Reg8::C),
            R8::D => self.registers.reg8(Reg8::D),
            R8::E => self.registers.reg8(Reg8::E),
            R8::H => self.registers.reg8(Reg8::H),
            R8::L => self.registers.reg8(Reg8::L),
            R8::HLInd => self.read_ind(MemIndirect::HL),
        }
    }

    fn write8(&mut self, r8: R8, val: u8) {
        match r8 {
            R8::A => self.registers.write(RegWrite::A(val)),
            R8::B => self.registers.write(RegWrite::B(val)),
            R8::C => self.registers.write(RegWrite::C(val)),
            R8::D => self.registers.write(RegWrite::D(val)),
            R8::E => self.registers.write(RegWrite::E(val)),
            R8::H => self.registers.write(RegWrite::H(val)),
            R8::L => self.registers.write(RegWrite::L(val)),
            R8::HLInd => self.memory.write(self.registers.reg16(Reg16::HL), val),
        }
    }

    fn move8(&mut self, dest: R8, src: R8) {
        let val = self.read8(src);
        self.write8(dest, val);
    }

    fn read16(&mut self, r16: R16) -> u16 {
        match r16 {
            R16::BC => self.registers.reg16(Reg16::BC),
            R16::DE => self.registers.reg16(Reg16::DE),
            R16::HL => self.registers.reg16(Reg16::HL),
            R16::SP => self.registers.reg16(Reg16::SP),
        }
    }

    fn write16(&mut self, r16: R16, val: u16) {
        match r16 {
            R16::BC => self.registers.write(RegWrite::BC(val)),
            R16::DE => self.registers.write(RegWrite::DE(val)),
            R16::HL => self.registers.write(RegWrite::HL(val)),
            R16::SP => self.registers.write(RegWrite::SP(val)),
        }
    }

    fn read_ind(&mut self, ind: MemIndirect) -> u8 {
        match ind {
            MemIndirect::BC => self.memory.read(self.registers.reg16(Reg16::BC)),
            MemIndirect::DE => self.memory.read(self.registers.reg16(Reg16::DE)),
            MemIndirect::HL => self.memory.read(self.registers.reg16(Reg16::HL)),
            MemIndirect::HLInc => {
                let hl = self.registers.reg16(Reg16::HL);
                let val = self.memory.read(hl);
                self.registers.write(RegWrite::HL(hl.wrapping_add(1)));
                val
            }
            MemIndirect::HLDec => {
                let hl = self.registers.reg16(Reg16::HL);
                let val = self.memory.read(hl);
                self.registers.write(RegWrite::HL(hl.wrapping_sub(1)));
                val
            }
        }
    }

    fn write_ind(&mut self, ind: MemIndirect, val: u8) {
        match ind {
            MemIndirect::BC => self.memory.write(self.registers.reg16(Reg16::BC), val),
            MemIndirect::DE => self.memory.write(self.registers.reg16(Reg16::DE), val),
            MemIndirect::HL => self.memory.write(self.registers.reg16(Reg16::HL), val),
            MemIndirect::HLInc => {
                let hl = self.registers.reg16(Reg16::HL);
                self.memory.write(hl, val);
                self.registers.write(RegWrite::HL(hl.wrapping_add(1)));
            }
            MemIndirect::HLDec => {
                let hl = self.registers.reg16(Reg16::HL);
                self.memory.write(hl, val);
                self.registers.write(RegWrite::HL(hl.wrapping_sub(1)));
            }
        }
    }

    fn read_alu(&mut self, alu: AluSrc) -> u8 {
        match alu {
            AluSrc::R8(r8) => self.read8(r8),
            AluSrc::Imm(val) => val,
        }
    }

    fn read_io(&self, io: Io) -> u8 {
        match io {
            Io::C => self
                .memory
                .read(0xFF00 + self.registers.reg8(Reg8::C) as u16),
            Io::Imm(imm) => self.memory.read(0xFF00 + imm as u16),
        }
    }

    fn write_io(&mut self, io: Io, val: u8) {
        match io {
            Io::C => self
                .memory
                .write(0xFF00 + self.registers.reg8(Reg8::C) as u16, val),
            Io::Imm(imm) => self.memory.write(0xFF00 + imm as u16, val),
        }
    }

    fn read_push_pop(&self, push_pop: PushPop) -> u16 {
        match push_pop {
            PushPop::BC => self.registers.reg16(Reg16::BC),
            PushPop::DE => self.registers.reg16(Reg16::DE),
            PushPop::HL => self.registers.reg16(Reg16::HL),
            PushPop::AF => self.registers.reg16(Reg16::AF),
        }
    }

    fn write_push_pop(&mut self, push_pop: PushPop, val: u16) {
        match push_pop {
            PushPop::BC => self.registers.write(RegWrite::BC(val)),
            PushPop::DE => self.registers.write(RegWrite::DE(val)),
            PushPop::HL => self.registers.write(RegWrite::HL(val)),
            PushPop::AF => self.registers.write(RegWrite::AF(val)),
        }
    }

    fn read_branch_cond(&self, cond: BranchCond) -> bool {
        match cond {
            BranchCond::NZ => !self.registers.flags.z,
            BranchCond::Z => self.registers.flags.z,
            BranchCond::NC => !self.registers.flags.c,
            BranchCond::C => self.registers.flags.c,
        }
    }

    fn push8(&mut self, val: u8) {
        let sp = self.registers.reg16(Reg16::SP);
        self.registers.write(RegWrite::SP(sp - 1));
        self.memory.write(sp - 1, val);
    }

    fn push16(&mut self, val: u16) {
        let [lsb, msb] = val.to_le_bytes();
        self.push8(msb);
        self.push8(lsb);
    }

    fn pop8(&mut self) -> u8 {
        let sp = self.registers.reg16(Reg16::SP);
        let val = self.memory.read(sp);
        self.registers.write(RegWrite::SP(sp + 1));
        val
    }

    fn pop16(&mut self) -> u16 {
        let lsb = self.pop8();
        let msb = self.pop8();
        u16::from_le_bytes([lsb, msb])
    }

    fn set_flags(&mut self, z: Option<bool>, n: Option<bool>, h: Option<bool>, c: Option<bool>) {
        if let Some(z) = z {
            self.registers.flags.z = z;
        }
        if let Some(n) = n {
            self.registers.flags.n = n;
        }
        if let Some(h) = h {
            self.registers.flags.h = h;
        }
        if let Some(c) = c {
            self.registers.flags.c = c;
        }
    }
}

pub enum MemIndirect {
    BC,
    DE,
    HL,
    HLInc,
    HLDec,
}

impl From<Indirect> for MemIndirect {
    fn from(ind: Indirect) -> Self {
        match ind {
            Indirect::BC => MemIndirect::BC,
            Indirect::DE => MemIndirect::DE,
            Indirect::HLInc => MemIndirect::HLInc,
            Indirect::HLDec => MemIndirect::HLDec,
        }
    }
}

trait HalfCarry: Sized {
    fn half_overflowing_add(&self, rhs: Self) -> (Self, bool, bool);
    fn half_overflowing_sub(&self, rhs: Self) -> (Self, bool, bool);
}

trait HalfCarrySigned: Sized {
    fn half_overflowing_add_signed(&self, rhs: i8) -> (Self, bool, bool);
}

impl HalfCarry for u8 {
    fn half_overflowing_add(&self, rhs: u8) -> (u8, bool, bool) {
        let (val, c) = self.overflowing_add(rhs);
        let h = ((self & 0xf) + (rhs & 0xf)) & 0x10 == 0x10;
        (val, h, c)
    }

    fn half_overflowing_sub(&self, rhs: u8) -> (u8, bool, bool) {
        let (val, c) = self.overflowing_sub(rhs);
        let h = (self & 0xf) < (rhs & 0xf);
        (val, h, c)
    }
}

impl HalfCarry for u16 {
    fn half_overflowing_add(&self, rhs: u16) -> (u16, bool, bool) {
        let (val, c) = self.overflowing_add(rhs);
        let h = ((self & 0xfff) + (rhs & 0xfff)) & 0x1000 == 0x1000;
        (val, h, c)
    }

    fn half_overflowing_sub(&self, rhs: u16) -> (u16, bool, bool) {
        let (val, c) = self.overflowing_sub(rhs);
        let h = (self & 0xfff) < (rhs & 0xfff);
        (val, h, c)
    }
}

impl HalfCarrySigned for u16 {
    fn half_overflowing_add_signed(&self, rhs: i8) -> (Self, bool, bool) {
        let result = self.wrapping_add_signed(rhs as i16);
        let (_, h, c) = (*self as u8).half_overflowing_add(rhs as u8);
        (result, h, c)
    }
}
