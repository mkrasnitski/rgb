use enum_primitive_derive::Primitive;
use std::fmt;

#[derive(Copy, Clone)]
pub enum Instruction {
    Nop,
    Ld(LdType),

    Add(AluSrc),
    Adc(AluSrc),
    Sub(AluSrc),
    Sbc(AluSrc),
    And(AluSrc),
    Xor(AluSrc),
    Or(AluSrc),
    Cp(AluSrc),

    IncR8(R8),
    DecR8(R8),
    IncR16(R16),
    DecR16(R16),

    AddHL(R16),
    AddSP(i8),

    Rlc(R8),
    Rrc(R8),
    Rl(R8),
    Rr(R8),
    Sla(R8),
    Sra(R8),
    Swap(R8),
    Srl(R8),
    Bit(BitPos, R8),
    Res(BitPos, R8),
    Set(BitPos, R8),

    Rlca,
    Rla,
    Rrca,
    Rra,

    Jr(BranchCond, i8),
    JrAlways(i8),
    Jp(BranchCond, u16),
    JpAlways(u16),
    Call(BranchCond, u16),
    CallAlways(u16),
    Ret(BranchCond),
    RetAlways,
    Rst(u8),
    Reti,
    JpHL,

    Push(PushPop),
    Pop(PushPop),

    Scf,
    Ccf,

    Daa,
    Cpl,

    Stop,
    Halt,

    Di,
    Ei,
}

#[derive(Copy, Clone, Debug)]
pub enum LdType {
    R8(R8, R8),
    R8Imm(R8, u8),
    R16Imm(R16, u16),
    AFromInd(Indirect),
    IndFromA(Indirect),
    AFromIoReg(Io),
    IoRegFromA(Io),
    AFromMem(u16),
    MemFromA(u16),
    StoreSP(u16),
    HLFromSP(i8),
    SPFromHL,
}

#[derive(Copy, Clone, Primitive)]
pub enum R8 {
    A = 7,
    B = 0,
    C = 1,
    D = 2,
    E = 3,
    H = 4,
    L = 5,
    HLInd = 6,
}

impl fmt::Debug for R8 {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "{}",
            match self {
                R8::A => "A",
                R8::B => "B",
                R8::C => "C",
                R8::D => "D",
                R8::E => "E",
                R8::H => "H",
                R8::L => "L",
                R8::HLInd => "(HL)",
            }
        )
    }
}

#[derive(Copy, Clone, Debug, Primitive)]
pub enum R16 {
    BC = 0,
    DE = 1,
    HL = 2,
    SP = 3,
}

#[derive(Copy, Clone, Primitive)]
pub enum Indirect {
    BC = 0,
    DE = 1,
    HLInc = 2,
    HLDec = 3,
}

#[derive(Copy, Clone, Debug)]
pub enum Io {
    C,
    Imm(u8),
}

#[derive(Copy, Clone)]
pub enum AluSrc {
    R8(R8),
    Imm(u8),
}

#[derive(Copy, Clone, Primitive)]
pub enum BitPos {
    Zero = 0,
    One = 1,
    Two = 2,
    Three = 3,
    Four = 4,
    Five = 5,
    Six = 6,
    Seven = 7,
}

#[derive(Copy, Clone, Debug, Primitive)]
pub enum BranchCond {
    NZ = 0,
    Z = 1,
    NC = 2,
    C = 3,
}

#[derive(Copy, Clone, Debug, Primitive)]
pub enum PushPop {
    BC = 0,
    DE = 1,
    HL = 2,
    AF = 3,
}

pub enum CycleCount {
    Const(u64),
    Branch(u64, u64),
}

impl fmt::Debug for CycleCount {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            CycleCount::Const(val) => write!(f, "[{val}]   "),
            CycleCount::Branch(not_taken, taken) => write!(f, "[{not_taken}, {taken}]"),
        }
    }
}

impl Instruction {
    pub const fn length(self) -> usize {
        use Instruction::*;
        match self {
            Nop => 1,
            Ld(ld_type) => match ld_type {
                LdType::R8(_, _) => 1,
                LdType::R8Imm(_, _) => 2,
                LdType::R16Imm(_, _) => 3,
                LdType::AFromInd(_) | LdType::IndFromA(_) => 1,
                LdType::AFromIoReg(io) | LdType::IoRegFromA(io) => match io {
                    Io::C => 1,
                    Io::Imm(_) => 2,
                },
                LdType::AFromMem(_) | LdType::MemFromA(_) => 3,
                LdType::StoreSP(_) => 3,
                LdType::HLFromSP(_) => 2,
                LdType::SPFromHL => 1,
            },

            Add(src) | Adc(src) | Sub(src) | Sbc(src) | And(src) | Xor(src) | Or(src) | Cp(src) => {
                match src {
                    AluSrc::R8(_) => 1,
                    AluSrc::Imm(_) => 2,
                }
            }

            IncR8(_) | DecR8(_) | IncR16(_) | DecR16(_) => 1,

            AddHL(_) => 1,
            AddSP(_) => 2,

            Rlc(_)
            | Rrc(_)
            | Rl(_)
            | Rr(_)
            | Sla(_)
            | Sra(_)
            | Swap(_)
            | Srl(_)
            | Bit(_, _)
            | Res(_, _)
            | Set(_, _) => 2,

            Rlca | Rla | Rrca | Rra => 1,

            Jr(_, _) | JrAlways(_) => 2,
            Jp(_, _) | JpAlways(_) => 3,
            Call(_, _) | CallAlways(_) => 3,
            Ret(_) | RetAlways => 1,
            Rst(_) => 1,
            Reti => 1,
            JpHL => 1,

            Push(_) | Pop(_) => 1,

            Scf | Ccf => 1,

            Daa => 1,
            Cpl => 1,

            Stop | Halt => 1,

            Di | Ei => 1,
        }
    }

    pub const fn mcycles(self) -> CycleCount {
        use CycleCount::*;
        use Instruction::*;
        match self {
            Nop => Const(1),
            Ld(ld_type) => Const(match ld_type {
                LdType::R8(dest, src) => match (dest, src) {
                    (R8::HLInd, _) | (_, R8::HLInd) => 2,
                    _ => 1,
                },
                LdType::R8Imm(dest, _) => match dest {
                    R8::HLInd => 3,
                    _ => 2,
                },
                LdType::R16Imm(_, _) => 3,
                LdType::AFromInd(_) | LdType::IndFromA(_) => 2,
                LdType::AFromIoReg(io) | LdType::IoRegFromA(io) => match io {
                    Io::C => 2,
                    Io::Imm(_) => 3,
                },
                LdType::AFromMem(_) | LdType::MemFromA(_) => 4,
                LdType::StoreSP(_) => 5,
                LdType::HLFromSP(_) => 3,
                LdType::SPFromHL => 2,
            }),

            Add(src) | Adc(src) | Sub(src) | Sbc(src) | And(src) | Xor(src) | Or(src) | Cp(src) => {
                Const(match src {
                    AluSrc::R8(R8::HLInd) | AluSrc::Imm(_) => 2,
                    _ => 1,
                })
            }

            IncR8(r8) | DecR8(r8) => Const(match r8 {
                R8::HLInd => 3,
                _ => 1,
            }),
            IncR16(_) | DecR16(_) => Const(2),

            AddHL(_) => Const(2),
            AddSP(_) => Const(4),

            Rlc(r8)
            | Rrc(r8)
            | Rl(r8)
            | Rr(r8)
            | Sla(r8)
            | Sra(r8)
            | Swap(r8)
            | Srl(r8)
            | Res(_, r8)
            | Set(_, r8) => Const(match r8 {
                R8::HLInd => 4,
                _ => 2,
            }),

            Bit(_, r8) => Const(match r8 {
                R8::HLInd => 3,
                _ => 2,
            }),

            Rlca | Rla | Rrca | Rra => Const(1),

            Jr(_, _) => Branch(2, 3),
            JrAlways(_) => Const(3),
            Jp(_, _) => Branch(3, 4),
            JpAlways(_) => Const(4),
            Call(_, _) => Branch(3, 6),
            CallAlways(_) => Const(6),
            Ret(_) => Branch(2, 5),
            RetAlways => Const(4),
            Rst(_) => Const(4),
            Reti => Const(4),
            JpHL => Const(1),

            Push(_) => Const(4),
            Pop(_) => Const(3),

            Scf | Ccf => Const(1),
            Daa => Const(1),
            Cpl => Const(1),

            Stop | Halt => Const(1),

            Di | Ei => Const(1),
        }
    }
}

impl fmt::Debug for Instruction {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        use Instruction::*;
        match self {
            Nop => write!(f, "NOP"),
            Ld(ld_type) => match ld_type {
                LdType::R8(dest, src) => write!(f, "LD {dest:?}, {src:?}"),
                LdType::R8Imm(dest, val) => write!(f, "LD {dest:?}, ${val:02x}"),
                LdType::R16Imm(dest, val) => write!(f, "LD {dest:?}, ${val:04x}"),
                LdType::AFromInd(ind) => write!(f, "LD A, {ind:?}"),
                LdType::IndFromA(ind) => write!(f, "LD {ind:?}, A"),
                LdType::AFromIoReg(io) => match io {
                    Io::C => write!(f, "LD A, (FF00 + C)"),
                    Io::Imm(val) => write!(f, "LD A, ($FF00 + ${val:02x})"),
                },
                LdType::IoRegFromA(io) => match io {
                    Io::C => write!(f, "LD (FF00 + C), A"),
                    Io::Imm(val) => write!(f, "LD ($FF00 + ${val:02x}), A"),
                },
                LdType::AFromMem(val) => write!(f, "LD A, (${val:04x})"),
                LdType::MemFromA(val) => write!(f, "LD (${val:04x}), A"),
                LdType::StoreSP(val) => write!(f, "LD (${val:04x}), SP"),
                LdType::HLFromSP(val) => write!(f, "LD HL, SP{val:+}"),
                LdType::SPFromHL => write!(f, "LD SP, HL"),
            },

            IncR8(r8) => write!(f, "INC {r8:?}"),
            DecR8(r8) => write!(f, "DEC {r8:?}"),
            IncR16(r16) => write!(f, "INC {r16:?}"),
            DecR16(r16) => write!(f, "DEC {r16:?}"),

            Add(src) => write!(f, "ADD A, {src:?}"),
            Adc(src) => write!(f, "ADC A, {src:?}"),
            Sub(src) => write!(f, "SUB A, {src:?}"),
            Sbc(src) => write!(f, "SBC A, {src:?}"),
            And(src) => write!(f, "AND A, {src:?}"),
            Xor(src) => write!(f, "XOR A, {src:?}"),
            Or(src) => write!(f, "OR A, {src:?}"),
            Cp(src) => write!(f, "CP A, {src:?}"),

            AddHL(r16) => write!(f, "ADD HL, {r16:?}"),
            AddSP(val) => write!(f, "ADD SP, {val:+}"),

            Rlc(r8) => write!(f, "RLC {r8:?}"),
            Rrc(r8) => write!(f, "RRC {r8:?}"),
            Rl(r8) => write!(f, "RL {r8:?}"),
            Rr(r8) => write!(f, "RR {r8:?}"),
            Sla(r8) => write!(f, "SLA {r8:?}"),
            Sra(r8) => write!(f, "SRA {r8:?}"),
            Swap(r8) => write!(f, "SWAP {r8:?}"),
            Srl(r8) => write!(f, "SRL {r8:?}"),
            Bit(bit, r8) => write!(f, "BIT {bit:?}, {r8:?}"),
            Res(bit, r8) => write!(f, "RES {bit:?}, {r8:?}"),
            Set(bit, r8) => write!(f, "SET {bit:?}, {r8:?}"),

            Rlca => write!(f, "RLCA"),
            Rla => write!(f, "RLA"),
            Rrca => write!(f, "RRCA"),
            Rra => write!(f, "RRA"),

            Jr(cond, val) => write!(f, "JR {cond:?}, {val:+}"),
            JrAlways(val) => write!(f, "JR {val:+}"),
            Jp(cond, val) => write!(f, "JP {cond:?}, {val:04X}"),
            JpAlways(val) => write!(f, "JP {val:04X}"),
            Call(cond, val) => write!(f, "CALL {cond:?}, {val:04X}"),
            CallAlways(val) => write!(f, "CALL {val:04X}"),
            Ret(cond) => write!(f, "RET {cond:?}"),
            RetAlways => write!(f, "RET"),
            Rst(val) => write!(f, "RST ${val:02x}"),
            Reti => write!(f, "RETI"),
            JpHL => write!(f, "JP HL"),

            Push(src) => write!(f, "PUSH {src:?}"),
            Pop(dest) => write!(f, "POP {dest:?}"),

            Scf => write!(f, "SCF"),
            Ccf => write!(f, "CCF"),

            Daa => write!(f, "DAA"),
            Cpl => write!(f, "CPL"),

            Stop => write!(f, "STOP"),
            Halt => write!(f, "HALT"),

            Di => write!(f, "DI"),
            Ei => write!(f, "EI"),
        }
    }
}

impl fmt::Debug for Indirect {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "({})",
            match self {
                Indirect::BC => "BE",
                Indirect::DE => "DE",
                Indirect::HLInc => "HL+",
                Indirect::HLDec => "HL-",
            }
        )
    }
}

impl fmt::Debug for BitPos {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        (*self as u8).fmt(f)
    }
}

impl fmt::Debug for AluSrc {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            AluSrc::R8(r8) => r8.fmt(f),
            AluSrc::Imm(val) => write!(f, "${val:02x}"),
        }
    }
}
