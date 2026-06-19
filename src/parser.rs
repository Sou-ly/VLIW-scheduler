use crate::instruction::*;

pub fn parse_instruction(input: &str) -> Result<Instruction, String> {
    let delimiters = |c: char| c == ',' || c == ' ' || c == '(' || c == ')';
    let parts: Vec<&str> = input.split(delimiters).filter(|&s| !s.is_empty()).collect();

    if parts.is_empty() {
        return Err("Empty input".to_string());
    }

    match parts[0] {
        "add" => {
            if parts.len() != 4 {
                return Err("Incorrect number of operands for 'add' instruction".to_string());
            }
            let dest = parse_register(parts[1])?;
            let opa = parse_register(parts[2])?;
            let opb = parse_register(parts[3])?;
            Ok(Instruction::Add(dest, opa, opb))
        }
        "addi" => {
            if parts.len() != 4 {
                return Err("Incorrect number of operands for 'addi' instruction".to_string());
            }
            let dest = parse_register(parts[1])?;
            let opa = parse_register(parts[2])?;
            let imm = parse_imm(parts[3])?;
            Ok(Instruction::Addi(dest, opa, imm))
        }
        "sub" => {
            if parts.len() != 4 {
                return Err("Incorrect number of operands for 'sub' instruction".to_string());
            }
            let dest = parse_register(parts[1])?;
            let opa = parse_register(parts[2])?;
            let opb = parse_register(parts[3])?;
            Ok(Instruction::Sub(dest, opa, opb))
        }
        "mulu" => {
            if parts.len() != 4 {
                return Err("Incorrect number of operands for 'mulu' instruction".to_string());
            }
            let dest = parse_register(parts[1])?;
            let opa = parse_register(parts[2])?;
            let opb = parse_register(parts[3])?;
            Ok(Instruction::Mulu(dest, opa, opb))
        }
        "ld" => {
            if parts.len() != 4 {
                return Err("Incorrect number of operands for 'ld' instruction".to_string());
            }
            let dest = parse_register(parts[1])?;
            let imm = parse_imm(parts[2])?;
            let addr = parse_register(parts[3])?;
            Ok(Instruction::Ld(dest, imm, addr))
        }
        "st" => {
            if parts.len() != 4 {
                return Err("Incorrect number of operands for 'st' instruction".to_string());
            }
            let source = parse_register(parts[1])?;
            let imm = parse_imm(parts[2])?;
            let addr = parse_register(parts[3])?;
            Ok(Instruction::St(source, imm, addr))
        }
        "loop" => {
            if parts.len() != 2 {
                return Err("Incorrect number of operands for 'loop' instruction".to_string());
            }
            let imm = parse_imm(parts[1])?;
            Ok(Instruction::Loop(imm as usize))
        }
        "loop.pip" => {
            if parts.len() != 2 {
                return Err("Incorrect number of operands for 'loop.pip' instruction".to_string());
            }
            let imm = parse_imm(parts[1])?;
            Ok(Instruction::LoopPip(imm as usize))
        }
        "mov" => {
            if parts.len() != 3 {
                return Err("Incorrect number of operands for 'mov' instruction".to_string());
            }
            match parts[1] {
                "LC" => {
                    let imm = parse_imm(parts[2])?;
                    Ok(Instruction::MovL(SpecialRegister::LoopCount, imm))
                }
                "EC" => {
                    let imm = parse_imm(parts[2])?;
                    Ok(Instruction::MovL(SpecialRegister::EpCount, imm))
                }
                _ => {
                    let dest = parse_register(parts[1])?;
                    match parts[1].chars().nth(0) {
                        Some('p') => {
                            let pred = parse_pred(parts[2])?;
                            Ok(Instruction::MovP(dest, pred))
                        }
                        Some('x') => match parts[2].chars().nth(0) {
                            Some('x') => {
                                let src = parse_register(parts[2])?;
                                Ok(Instruction::Mov(dest, src))
                            }
                            _ => {
                                let imm = parse_imm(parts[2])?;
                                Ok(Instruction::MovI(dest, imm))
                            }
                        },
                        _ => Err("Incorrect format of operands for 'mov' instruction".to_string()),
                    }
                }
            }
        }
        _ => Err("Unknown instruction".to_string()),
    }
}

fn parse_register(s: &str) -> Result<u8, String> {
    let trimmed = s.trim_start_matches(['x', 'p']).trim_end_matches(",");
    match trimmed.parse::<u8>() {
        Ok(num) if num <= 96 => Ok(num as u8),
        _ => Err(format!("Invalid register: {}", s)),
    }
}

fn parse_imm(s: &str) -> Result<i16, String> {
    match s.parse::<i16>() {
        Ok(num) => Ok(num),
        Err(_) => match i16::from_str_radix(s.trim_start_matches("0x"), 16) {
            Ok(num) => Ok(num),
            Err(_) => Err(format!("Invalid immediate value: {}", s)),
        },
    }
}

fn parse_pred(s: &str) -> Result<bool, String> {
    match s.parse::<bool>() {
        Ok(pred) => Ok(pred),
        _ => Err(format!("Invalid predicate value: {}", s)),
    }
}
