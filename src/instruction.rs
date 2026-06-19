use std::{collections::HashSet, vec};

#[derive(Clone, Copy)]
pub enum ExecutionUnit {
    Alu = 0,
    Mul = 1,
    Mem = 2,
    Branch = 3,
}

impl ExecutionUnit {
    pub fn latency(&self) -> u8 {
        match self {
            ExecutionUnit::Mul => 3,
            _ => 1,
        }
    }

    pub fn nb_available(&self) -> usize {
        match self {
            ExecutionUnit::Alu => 2,
            _ => 1,
        }
    }

    pub fn iter() -> Vec<ExecutionUnit> {
        use crate::ExecutionUnit::*;
        return vec![Alu, Mul, Mem, Branch];
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum SpecialRegister {
    LoopCount,
    EpCount,
}

impl SpecialRegister {
    pub fn to_string(&self) -> String {
        match self {
            SpecialRegister::LoopCount => "LC".to_string(),
            SpecialRegister::EpCount => "EC".to_string(),
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum Instruction {
    Add(u8, u8, u8),
    Addi(u8, u8, i16),
    Sub(u8, u8, u8),
    Mulu(u8, u8, u8),
    Ld(u8, i16, u8),
    St(u8, i16, u8),
    Loop(usize),
    LoopPip(usize),
    MovP(u8, bool),
    MovL(SpecialRegister, i16),
    MovI(u8, i16),
    Mov(u8, u8),
    Nop,
}

impl Instruction {
    pub fn unit(&self) -> ExecutionUnit {
        match self {
            Instruction::Ld(_, _, _) | Instruction::St(_, _, _) => ExecutionUnit::Mem,
            Instruction::Loop(_) | Instruction::LoopPip(_) => ExecutionUnit::Branch,
            Instruction::Mulu(_, _, _) => ExecutionUnit::Mul,
            _ => ExecutionUnit::Alu,
        }
    }

    pub fn to_string(&self) -> String {
        match self {
            Instruction::Add(dest, opa, opb) => format!(" add x{dest}, x{opa}, x{opb}"),
            Instruction::Addi(dest, opa, imm) => format!(" addi x{dest}, x{opa}, {imm}"),
            Instruction::Sub(dest, opa, opb) => format!(" sub x{dest}, x{opa}, x{opb}"),
            Instruction::Mulu(dest, opa, opb) => format!(" mulu x{dest}, x{opa}, x{opb}"),
            Instruction::Ld(dest, imm, addr) => format!(" ld x{}, {}(x{})", dest, imm, addr),
            Instruction::St(source, imm, addr) => format!(" st x{}, {}(x{})", source, imm, addr),
            Instruction::Loop(imm) => format!(" loop {imm}"),
            Instruction::LoopPip(imm) => format!(" loop.pip {imm}"),
            Instruction::MovP(dest, pred) => format!(" mov p{0}, {pred}", dest & 0b0111_111),
            Instruction::MovL(dest, imm) => format!(" mov {0}, {imm}", dest.to_string()),
            Instruction::MovI(dest, imm) => format!(" mov x{dest}, {imm}"),
            Instruction::Mov(dest, src) => format!(" mov x{dest}, x{src}"),
            _ => format!(" nop"),
        }
    }

    pub fn dest(&self) -> Option<u8> {
        match self {
            Instruction::Add(dest, _, _) => Some(*dest),
            Instruction::Addi(dest, _, _) => Some(*dest),
            Instruction::Sub(dest, _, _) => Some(*dest),
            Instruction::Mulu(dest, _, _) => Some(*dest),
            Instruction::Ld(dest, _, _) => Some(*dest),
            Instruction::Mov(dest, _) => Some(*dest),
            Instruction::MovI(dest, _) => Some(*dest),
            _ => None,
        }
    }

    pub fn sources(&self) -> Vec<u8> {
        let mut result = HashSet::new();
        match self {
            Instruction::Add(_, opa, opb)
            | Instruction::Sub(_, opa, opb)
            | Instruction::Mulu(_, opa, opb)
            | Instruction::St(opa, _, opb) => {
                result.insert(*opa);
                result.insert(*opb);
            }
            Instruction::Addi(_, opa, _)
            | Instruction::Ld(_, _, opa)
            | Instruction::Mov(_, opa) => {
                result.insert(*opa);
            }
            _ => {}
        }
        result.into_iter().collect()
    }

    // same as above but allows repetitions
    pub fn operands(&self) -> Vec<u8> {
        match self {
            Instruction::Add(_, opa, opb) => vec![*opa, *opb],
            Instruction::Addi(_, opa, _) => vec![*opa],
            Instruction::Sub(_, opa, opb) => vec![*opa, *opb],
            Instruction::Mulu(_, opa, opb) => vec![*opa, *opb],
            Instruction::Ld(_, _, addr) => vec![*addr],
            Instruction::St(source, _, addr) => vec![*source, *addr],
            Instruction::Mov(_, src) => vec![*src],
            _ => vec![],
        }
    }
}

#[derive(Copy, Clone, Debug)]
pub enum PredicatedInstruction {
    Add(u8, u8, u8, u8),
    Addi(u8, u8, u8, i16),
    Sub(u8, u8, u8, u8),
    Mulu(u8, u8, u8, u8),
    Ld(u8, u8, i16, u8),
    St(u8, u8, i16, u8),
    Loop(usize),
    LoopPip(usize),
    MovP(u8, bool),
    MovL(SpecialRegister, i16),
    MovI(u8, u8, i16),
    Mov(u8, u8, u8),
    Nop,
}

impl PredicatedInstruction {
    pub fn from(ins: &Instruction, pred: u8) -> PredicatedInstruction {
        match *ins {
            Instruction::Add(dest, opa, opb) => PredicatedInstruction::Add(pred, dest, opa, opb),
            Instruction::Addi(dest, opa, imm) => PredicatedInstruction::Addi(pred, dest, opa, imm),
            Instruction::Sub(dest, opa, opb) => PredicatedInstruction::Sub(pred, dest, opa, opb),
            Instruction::Mulu(dest, opa, opb) => PredicatedInstruction::Mulu(pred, dest, opa, opb),
            Instruction::Ld(dest, imm, addr) => PredicatedInstruction::Ld(pred, dest, imm, addr),
            Instruction::St(source, imm, addr) => {
                PredicatedInstruction::St(pred, source, imm, addr)
            }
            Instruction::Loop(usize) => PredicatedInstruction::Loop(usize),
            Instruction::LoopPip(usize) => PredicatedInstruction::LoopPip(usize),
            Instruction::MovP(dest, val) => PredicatedInstruction::MovP(dest, val),
            Instruction::MovL(spec_reg, imm) => PredicatedInstruction::MovL(spec_reg, imm),
            Instruction::MovI(dest, imm) => PredicatedInstruction::MovI(pred, dest, imm),
            Instruction::Mov(dest, source) => PredicatedInstruction::Mov(pred, dest, source),
            _ => PredicatedInstruction::Nop,
        }
    }

    pub fn to_ins(&self) -> Instruction {
        match *self {
            PredicatedInstruction::Add(_, dest, opa, opb) => Instruction::Add(dest, opa, opb),
            PredicatedInstruction::Addi(_, dest, opa, imm) => Instruction::Addi(dest, opa, imm),
            PredicatedInstruction::Sub(_, dest, opa, opb) => Instruction::Sub(dest, opa, opb),
            PredicatedInstruction::Mulu(_, dest, opa, opb) => Instruction::Mulu(dest, opa, opb),
            PredicatedInstruction::Ld(_, dest, imm, addr) => Instruction::Ld(dest, imm, addr),
            PredicatedInstruction::St(_, src, imm, addr) => Instruction::St(src, imm, addr),
            PredicatedInstruction::Loop(usize) => Instruction::Loop(usize),
            PredicatedInstruction::LoopPip(usize) => Instruction::LoopPip(usize),
            PredicatedInstruction::MovP(dest, val) => Instruction::MovP(dest, val),
            PredicatedInstruction::MovL(spec_reg, imm) => Instruction::MovL(spec_reg, imm),
            PredicatedInstruction::MovI(_, dest, imm) => Instruction::MovI(dest, imm),
            PredicatedInstruction::Mov(_, dest, source) => Instruction::Mov(dest, source),
            _ => Instruction::Nop,
        }
    }

    pub fn to_string(&self) -> String {
        match self {
            PredicatedInstruction::Add(pred, _, _, _) => {
                if *pred < 32 {
                    (*self).to_ins().to_string()
                } else {
                    format!("(p{}) {}", *pred, (*self).to_ins().to_string())
                }
            }
            PredicatedInstruction::Addi(pred, _, _, _) => {
                if *pred < 32 {
                    (*self).to_ins().to_string()
                } else {
                    format!("(p{}) {}", *pred, (*self).to_ins().to_string())
                }
            }
            PredicatedInstruction::Sub(pred, _, _, _) => {
                if *pred < 32 {
                    (*self).to_ins().to_string()
                } else {
                    format!("(p{}) {}", *pred, (*self).to_ins().to_string())
                }
            }
            PredicatedInstruction::Mulu(pred, _, _, _) => {
                if *pred < 32 {
                    (*self).to_ins().to_string()
                } else {
                    format!("(p{}) {}", *pred, (*self).to_ins().to_string())
                }
            }
            PredicatedInstruction::Ld(pred, _, _, _) => {
                if *pred < 32 {
                    (*self).to_ins().to_string()
                } else {
                    format!("(p{}) {}", *pred, (*self).to_ins().to_string())
                }
            }
            PredicatedInstruction::St(pred, _, _, _) => {
                if *pred < 32 {
                    (*self).to_ins().to_string()
                } else {
                    format!("(p{}) {}", *pred, (*self).to_ins().to_string())
                }
            }
            PredicatedInstruction::Loop(_) => (*self).to_ins().to_string(),
            PredicatedInstruction::LoopPip(_) => (*self).to_ins().to_string(),
            PredicatedInstruction::MovP(_, _) => (*self).to_ins().to_string(),
            PredicatedInstruction::MovL(_, _) => (*self).to_ins().to_string(),
            PredicatedInstruction::MovI(pred, _, _) => {
                if *pred < 32 {
                    (*self).to_ins().to_string()
                } else {
                    format!("(p{}) {}", *pred, (*self).to_ins().to_string())
                }
            }
            PredicatedInstruction::Mov(pred, _, _) => {
                if *pred < 32 {
                    (*self).to_ins().to_string()
                } else {
                    format!("(p{}) {}", *pred, (*self).to_ins().to_string())
                }
            }
            _ => Instruction::Nop.to_string(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct VLIW {
    inner: [Vec<Instruction>; 4],
}

impl VLIW {
    pub fn new() -> Self {
        Self {
            inner: [Vec::new(), Vec::new(), Vec::new(), Vec::new()],
        }
    }

    pub fn to_string(&self) -> String {
        let mut out: Vec<String> = vec![];
        for unit in ExecutionUnit::iter() {
            for index in 0..unit.nb_available() {
                if let Some(instr) = self.inner[unit as usize].get(index) {
                    out.push(instr.to_string());
                } else {
                    out.push("nop".to_string());
                }
            }
        }
        return format!(
            "[{}]",
            out.iter()
                .map(|s| format!("{:?}", s))
                .collect::<Vec<_>>()
                .join(",")
        )
        .to_string();
    }

    pub fn iter(&mut self, unit: ExecutionUnit) -> &mut Vec<Instruction> {
        return &mut self.inner[unit as usize];
    }

    pub fn is_available(&self, unit: ExecutionUnit) -> bool {
        return self.inner[unit as usize].len() < unit.nb_available();
    }

    pub fn add(&mut self, instruction: Instruction) -> bool {
        if self.is_available(instruction.unit()) {
            self.inner[instruction.unit() as usize].push(instruction);
            return true;
        }
        return false;
    }

    pub fn pop(&mut self, unit: ExecutionUnit) -> Option<Instruction> {
        self.inner[unit as usize].pop()
    }
}
