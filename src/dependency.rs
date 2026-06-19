use crate::instruction::*;
use std::collections::HashMap;
use std::collections::HashSet;

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Dependency {
    Local = 0,
    InterLoop = 1,
    LoopInvariant = 2,
    PostLoop = 3,
}

impl Dependency {
    pub fn iter() -> Vec<Dependency> {
        use crate::Dependency::*;
        return vec![Local, InterLoop, LoopInvariant, PostLoop];
    }
}

#[derive(Debug)]
pub struct DependencyList {
    inner: [Vec<usize>; 4],
}

impl DependencyList {
    pub fn new() -> Self {
        Self {
            inner: [Vec::new(), Vec::new(), Vec::new(), Vec::new()],
        }
    }

    pub fn push(&mut self, dep_type: Dependency, producer_pc: usize) {
        self.inner[dep_type as usize].push(producer_pc);
    }

    pub fn pop(&mut self, dep_type: Dependency) {
        self.inner[dep_type as usize].pop();
    }

    pub fn iter(&self, dep_type: Dependency) -> &Vec<usize> {
        return &self.inner[dep_type as usize];
    }

    #[allow(dead_code)]
    pub fn is_empty(&self) -> bool {
        self.inner.iter().all(|deps| deps.is_empty())
    }

    pub fn sort(&mut self) {
        for dep_type in Dependency::iter() {
            self.inner[dep_type as usize].sort();
        }
    }
}

fn get_dependency_type(
    writer_pc: usize,
    reader_pc: usize,
    loop_start: usize,
    loop_end: usize,
    flag: bool,
) -> Option<Dependency> {
    if writer_pc < loop_start {
        if reader_pc <= writer_pc {
            return None;
        } else if reader_pc < loop_start {
            return Some(Dependency::Local);
        } else if flag && reader_pc < loop_end {
            return Some(Dependency::InterLoop);
        } else {
            return Some(Dependency::LoopInvariant);
        }
    } else if writer_pc < loop_end {
        if reader_pc < loop_start {
            return None;
        } else if reader_pc >= loop_end {
            return Some(Dependency::PostLoop);
        } else if reader_pc > writer_pc {
            return Some(Dependency::Local);
        } else {
            return Some(Dependency::InterLoop);
        }
    } else {
        if reader_pc <= writer_pc {
            return None;
        } else {
            return Some(Dependency::Local);
        }
    }
}

pub fn get_dependency_table(program: &Vec<Instruction>) -> Vec<DependencyList> {
    let mut register_writers = HashMap::<u8, Vec<usize>>::new();

    let mut loop_start = program.len();
    let mut loop_end = program.len();

    //Locate loop
    for (index, instr) in program.iter().enumerate() {
        if let Instruction::Loop(imm) | Instruction::LoopPip(imm) = instr {
            loop_start = *imm as usize;
            loop_end = index;
        }

        // save writers in a table
        // Map keys are registers, map values are PCs
        if let Some(dest) = instr.dest() {
            register_writers
                .entry(dest)
                .or_insert_with(Vec::new)
                .push(index);
        }
    }

    // detect potential interloop dependencies
    // Contains registers
    let mut modified_in_loop = HashSet::<u8>::new();
    for (reg, writers) in register_writers.clone().into_iter() {
        for instr in writers {
            if instr >= loop_start && instr < loop_end {
                modified_in_loop.insert(reg);
            }
        }
    }

    // build table
    let mut dependency_table = Vec::<DependencyList>::new();
    for (reader_pc, reader) in program.iter().enumerate() {
        let mut dep_list = DependencyList::new();
        //Iterate over sources REGISTERS
        for src in reader.sources() {
            //Iterate over PCs which are writing into the register the current instruction is
            //trying to read
            let flag = modified_in_loop.contains(&src);
            for writer_pc in register_writers.entry(src).or_insert_with(Vec::new) {
                //Figure out the type of dependency
                if let Some(dep_type) =
                    get_dependency_type(*writer_pc, reader_pc, loop_start, loop_end, flag)
                {
                    // ensure only latest writer is saved
                    if let Some(last) = dep_list.iter(dep_type).last() {
                        if program[*last].dest() == program[*writer_pc].dest()
                            && ((*last < reader_pc && *writer_pc < reader_pc)
                                || (*last >= reader_pc && *writer_pc >= reader_pc))
                        {
                            dep_list.pop(dep_type);
                        }
                    }
                    dep_list.push(dep_type, *writer_pc);
                }
            }
        }
        dep_list.sort();
        dependency_table.push(dep_list);
    }

    return dependency_table;
}

pub fn get_depended_upon(
    program: &Vec<Instruction>,
    dep_table: &Vec<DependencyList>,
    write_pc: usize,
) -> Vec<usize> {
    let mut res = Vec::new();
    for (pc, _) in program.iter().enumerate() {
        for dep_type in Dependency::iter() {
            if dep_table[pc].iter(dep_type).contains(&write_pc) {
                res.push(pc);
            }
        }
    }
    res
}

#[cfg(test)]
mod tests {
    use crate::dependency::*;
    use crate::instruction::*;
    use crate::Dependency::*;

    fn get_simple_loop() -> Vec<Instruction> {
        return vec![
            Instruction::MovL(SpecialRegister::LoopCount, 100),
            Instruction::MovI(2, 5),
            Instruction::Mulu(2, 2, 2),
            Instruction::Add(2, 2, 2),
            Instruction::Loop(3),
            Instruction::St(2, 0x1000, 0),
        ];
    }

    fn get_program() -> Vec<Instruction> {
        return vec![
            Instruction::MovL(SpecialRegister::LoopCount, 100),
            Instruction::MovI(2, 0x1000),
            Instruction::MovI(3, 1),
            Instruction::MovI(4, 25),
            Instruction::Ld(5, 0, 2),
            Instruction::Mulu(6, 5, 4),
            Instruction::Mulu(3, 3, 5),
            Instruction::St(6, 0, 2),
            Instruction::Addi(2, 2, 1),
            Instruction::Loop(4),
            Instruction::St(3, 0, 2),
        ];
    }

    #[test]
    fn get_dependency_table_works_large_loop() {
        let program = get_program();

        let table = get_dependency_table(&program);

        assert_eq!(table.len(), program.len());
        assert!(table[0].is_empty());
        assert!(table[1].is_empty());
        assert!(table[2].is_empty());
        assert!(table[3].is_empty());
        assert_eq!(*table[4].iter(InterLoop), vec![1, 8]);
        assert_eq!(*table[5].iter(Local), vec![4]);
        assert_eq!(*table[5].iter(LoopInvariant), vec![3]);
        assert_eq!(*table[6].iter(Local), vec![4]);
        assert_eq!(*table[6].iter(InterLoop), vec![2, 6]);
        assert_eq!(*table[7].iter(Local), vec![5]);
        assert_eq!(*table[7].iter(InterLoop), vec![1, 8]);
        assert_eq!(*table[8].iter(InterLoop), vec![1, 8]);
        assert!(table[9].is_empty());
        assert_eq!(*table[10].iter(PostLoop), vec![6, 8]);
    }

    #[test]
    fn get_dependency_table_works_simple_loop() {
        let program = get_simple_loop();

        let table = get_dependency_table(&program);
        assert!(table[0].is_empty());
        assert!(table[1].is_empty());
        assert_eq!(*table[2].iter(Dependency::Local), vec![1]);
        assert_eq!(*table[3].iter(Dependency::InterLoop), vec![2, 3]);
        assert!(table[4].is_empty());
        assert_eq!(*table[5].iter(Dependency::PostLoop), vec![3]);
    }
}
