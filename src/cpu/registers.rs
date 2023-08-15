#[derive(Default)]
pub struct Registers {
    a: u8,
    b: u8,
    c: u8,
    d: u8,
    e: u8,
    h: u8,
    l: u8,
    sp: u16,
    pub pc: u16,
    pub flags: Flags,
}

impl Registers {
    pub fn reg8(&self, src: Reg8) -> u8 {
        match src {
            Reg8::A => self.a,
            Reg8::B => self.b,
            Reg8::C => self.c,
            Reg8::D => self.d,
            Reg8::E => self.e,
            Reg8::H => self.h,
            Reg8::L => self.l,
        }
    }

    pub fn reg16(&self, src: Reg16) -> u16 {
        match src {
            Reg16::AF => u16::from_be_bytes([self.a, self.flags.into()]),
            Reg16::BC => u16::from_be_bytes([self.b, self.c]),
            Reg16::DE => u16::from_be_bytes([self.d, self.e]),
            Reg16::HL => u16::from_be_bytes([self.h, self.l]),
            Reg16::SP => self.sp,
        }
    }

    pub fn write(&mut self, dest: RegWrite) {
        use RegWrite::*;
        match dest {
            A(val) => self.a = val,
            B(val) => self.b = val,
            C(val) => self.c = val,
            D(val) => self.d = val,
            E(val) => self.e = val,
            H(val) => self.h = val,
            L(val) => self.l = val,
            AF(val) => {
                let f;
                [self.a, f] = val.to_be_bytes();
                self.flags = f.into();
            }
            BC(val) => [self.b, self.c] = val.to_be_bytes(),
            DE(val) => [self.d, self.e] = val.to_be_bytes(),
            HL(val) => [self.h, self.l] = val.to_be_bytes(),
            SP(val) => self.sp = val,
        }
    }
}

#[derive(Copy, Clone, Default)]
pub struct Flags {
    pub z: bool,
    pub n: bool,
    pub h: bool,
    pub c: bool,
}

impl From<u8> for Flags {
    fn from(val: u8) -> Self {
        Self {
            z: val & (1 << 7) != 0,
            n: val & (1 << 6) != 0,
            h: val & (1 << 5) != 0,
            c: val & (1 << 4) != 0,
        }
    }
}

impl From<Flags> for u8 {
    fn from(f: Flags) -> Self {
        ((f.z as u8) << 7) | ((f.n as u8) << 6) | ((f.h as u8) << 5) | ((f.c as u8) << 4)
    }
}

pub enum Reg8 {
    A,
    B,
    C,
    D,
    E,
    H,
    L,
}

pub enum Reg16 {
    AF,
    BC,
    DE,
    HL,
    SP,
}

pub enum RegWrite {
    A(u8),
    B(u8),
    C(u8),
    D(u8),
    E(u8),
    H(u8),
    L(u8),
    AF(u16),
    BC(u16),
    DE(u16),
    HL(u16),
    SP(u16),
}
