use crate::dependency::*;
use crate::instruction::*;
use std::{
    cmp::max,
    collections::{HashMap, HashSet},
    vec,
};

#[allow(dead_code)]
pub fn compute_initiation_interval(program: &Vec<Instruction>) -> usize {
    let mut counts: [usize; 4] = [0; 4];

    for instr in program {
        counts[instr.unit() as usize] = counts[instr.unit() as usize] + 1;
    }

    let mut max: usize = 0;
    for unit in ExecutionUnit::iter() {
        let nb_available = unit.nb_available();
        let interval = (counts[unit as usize] + nb_available - 1) / nb_available;
        if interval > max {
            max = interval;
        }
    }

    return max;
}

fn get_local_schedule(
    instructions: &[Instruction],
    dependency_table: &Vec<DependencyList>,
    start: usize,
) -> Vec<usize> {
    let mut schedule: Vec<usize> = vec![0; instructions.len()];
    let mut used: Vec<Vec<usize>> = vec![Vec::new(); ExecutionUnit::iter().len()];

    for (pc, instr) in instructions.iter().enumerate() {
        let unit_id = instr.unit() as usize;
        for write_pc in dependency_table[pc + start].iter(Dependency::Local) {
            let writer_latency = instructions[*write_pc - start].unit().latency() as usize;
            schedule[pc] = max(schedule[pc], schedule[*write_pc - start] + writer_latency);
        }
        while used[unit_id].len() > schedule[pc]
            && used[unit_id][schedule[pc]] >= instr.unit().nb_available()
        {
            schedule[pc] = schedule[pc] + 1;
        }
        while used[unit_id].len() <= schedule[pc] {
            used[instr.unit() as usize].push(0);
        }
        used[unit_id][schedule[pc]] = used[unit_id][schedule[pc]] + 1;
    }

    return schedule;
}

fn get_loop_schedule(
    instructions: &[Instruction],
    dependency_table: &Vec<DependencyList>,
    start: usize,
) -> Vec<usize> {
    if instructions.len() == 0 {
        return vec![];
    }
    let mut schedule = get_local_schedule(instructions, dependency_table, start);
    let loop_pc = schedule.len() - 1;
    if loop_pc >= 1 {
        for i in 0..loop_pc {
            schedule[loop_pc] = max(schedule[loop_pc], schedule[i]);
        }
    }
    for pc in 0..loop_pc {
        for writer_pc in dependency_table[pc + start].iter(Dependency::InterLoop) {
            if *writer_pc >= start {
                let writer_latency = instructions[*writer_pc - start].unit().latency() as usize;
                while schedule[pc] + schedule[loop_pc] - schedule[*writer_pc - start]
                    < writer_latency - 1
                {
                    schedule[loop_pc] = schedule[loop_pc] + 1;
                }
            }
        }
    }
    return schedule;
}

pub fn fix_schedule_offsets(
    schedule: &mut Vec<usize>,
    program: &Vec<Instruction>,
    dependency_table: &Vec<DependencyList>,
    start: usize,
) {
    let mut offset: usize = 0;
    for pc in 0..start {
        offset = max(offset, schedule[pc] + 1);
    }
    for pc in start..schedule.len() {
        for dep_type in Dependency::iter() {
            for writer_pc in dependency_table[pc].iter(dep_type) {
                if *writer_pc >= start {
                    continue;
                }
                let writer_latency = program[*writer_pc].unit().latency() as usize;
                while schedule[pc] + offset < writer_latency + schedule[*writer_pc] {
                    offset += 1;
                }
            }
        }
    }
    for pc in start..schedule.len() {
        schedule[pc] = schedule[pc] + offset;
    }
}

fn get_asap_schedule(
    program: &Vec<Instruction>,
    dependency_table: &Vec<DependencyList>,
    loop_start: usize,
    loop_end: usize,
) -> Vec<usize> {
    let mut schedule = get_local_schedule(&program[0..loop_start], dependency_table, 0);
    schedule.append(&mut get_loop_schedule(
        &program[loop_start..loop_end],
        dependency_table,
        loop_start,
    ));
    fix_schedule_offsets(&mut schedule, program, dependency_table, loop_start);
    schedule.append(&mut get_local_schedule(
        &program[loop_end..],
        dependency_table,
        loop_end,
    ));
    fix_schedule_offsets(&mut schedule, program, dependency_table, loop_end);
    return schedule;
}

fn update_dest(instr: &mut Instruction, new_dest: u8) {
    match instr {
        Instruction::Add(old_dest, _, _)
        | Instruction::Addi(old_dest, _, _)
        | Instruction::Sub(old_dest, _, _)
        | Instruction::Mulu(old_dest, _, _)
        | Instruction::Ld(old_dest, _, _)
        | Instruction::Mov(old_dest, _)
        | Instruction::MovI(old_dest, _) => *old_dest = new_dest,
        _ => {}
    }
}

fn update_ops(instr: &mut Instruction, new_ops: Vec<u8>) {
    match instr {
        Instruction::Add(_, a, b)
        | Instruction::Sub(_, a, b)
        | Instruction::St(a, _, b)
        | Instruction::Mulu(_, a, b) => {
            *a = new_ops[0];
            *b = new_ops[1];
        }
        Instruction::Mov(_, a) | Instruction::Ld(_, _, a) | Instruction::Addi(_, a, _) => {
            *a = new_ops[0]
        }
        _ => {}
    }
}

pub fn get_vliw_schedule(
    program: &Vec<Instruction>,
    dependency_table: &Vec<DependencyList>,
) -> Vec<VLIW> {
    let mut loop_start = program.len();
    let mut loop_end = program.len();
    for (pc, instr) in program.iter().enumerate() {
        match instr {
            Instruction::Loop(val) => {
                loop_start = *val;
                loop_end = pc + 1;
            }
            _ => {}
        }
    }

    // construct bundles
    let schedule = &get_asap_schedule(program, dependency_table, loop_start, loop_end);
    let mut nb_bundles = 0;
    for bundle_pc in schedule {
        nb_bundles = max(nb_bundles, bundle_pc + 1);
    }

    let mut offsets: Vec<usize> = vec![0; program.len()];
    let mut bundles = vec![VLIW::new(); nb_bundles];
    for (pc, instr) in program.iter().enumerate() {
        let index = schedule[pc];
        let mut new_instr = instr.clone();
        if let Instruction::Loop(_) = instr {
            new_instr = Instruction::Loop(schedule[loop_start]);
            loop_end = index;
        }
        bundles[index].add(new_instr);
        offsets[pc] = bundles[index].iter(instr.unit()).len() - 1;
    }

    // update destination registers
    let mut new_register = 1;
    for bundle in &mut bundles {
        for unit in ExecutionUnit::iter() {
            for mut instr in bundle.iter(unit) {
                if let Some(_old_register) = instr.dest() {
                    update_dest(&mut instr, new_register);
                    new_register += 1;
                }
            }
        }
    }

    // update operands
    for (old_pc, old_instr) in program.iter().enumerate() {
        if old_instr.operands().is_empty() {
            continue;
        }
        let mut new_ops = Vec::<u8>::new();
        // for each operand check who is the earliest writer
        for old_operand in old_instr.operands() {
            let mut modified = false;
            for dep_type in Dependency::iter() {
                let dep_list = dependency_table[old_pc].iter(dep_type);
                for old_producer_pc in dep_list {
                    let old_producer = program[*old_producer_pc];
                    if old_producer.dest() == Some(old_operand) {
                        let new_producer_pc = schedule[*old_producer_pc];
                        let prod_offset = offsets[*old_producer_pc];
                        let new_producer = bundles[new_producer_pc]
                            .iter(old_producer.unit())
                            .get(prod_offset)
                            .unwrap();
                        let new_op = new_producer.dest().unwrap();
                        new_ops.push(new_op);
                        modified = true;
                        break;
                    }
                }
                if modified {
                    break;
                }
            }
            if !modified {
                new_ops.push(0); // later used to detect unrenamed operands
            }
        }
        let new_pc = schedule[old_pc];
        let offset = offsets[old_pc];
        let new_instr = bundles[new_pc]
            .iter(old_instr.unit())
            .get_mut(offset)
            .unwrap();
        update_ops(new_instr, new_ops);
    }

    // find instructions with pair interloop dependencies
    let mut movs_set = HashSet::<Instruction>::new();
    for pc in 0..program.len() {
        let mut pair_dependencies = HashMap::<u8, usize>::new();
        for dep_pc in dependency_table[pc].iter(Dependency::InterLoop) {
            let reg = program[*dep_pc].dest().unwrap();
            if pair_dependencies.contains_key(&reg) {
                // add the mov instruction for later insertion
                let bb0_pc = pair_dependencies.get(&reg).unwrap();
                let bb0_unit = program[*bb0_pc].unit();
                let bb0_dest = bundles[schedule[*bb0_pc]]
                    .iter(bb0_unit)
                    .get(offsets[*bb0_pc])
                    .unwrap()
                    .dest()
                    .unwrap();
                let bb1_pc = dep_pc;
                let bb1_unit = program[*bb1_pc].unit();
                let bb1_dest = bundles[schedule[*bb1_pc]]
                    .iter(bb1_unit)
                    .get(offsets[*bb1_pc])
                    .unwrap()
                    .dest()
                    .unwrap();
                movs_set.insert(Instruction::Mov(bb0_dest, bb1_dest));
            } else {
                pair_dependencies.insert(reg, *dep_pc);
            }
        }
    }

    let movs: Vec<Instruction> = movs_set.into_iter().collect();

    let insert_start = loop_end;
    let mut insert_stop = insert_start + 1;
    for mov in movs {
        // update post loop bundles
        for index in insert_stop..bundles.len() {
            for unit_type in ExecutionUnit::iter() {
                for instr in bundles[index].iter(unit_type) {
                    let mut new_ops: Vec<u8> = vec![];
                    for op in instr.operands() {
                        if op == mov.dest().unwrap() {
                            new_ops.push(mov.operands()[0]);
                        } else {
                            new_ops.push(op);
                        }
                    }
                    update_ops(instr, new_ops);
                }
            }
        }
        // try to insert
        let mut has_inserted = false;
        for index in insert_start..insert_stop {
            // check for enough space in bundle
            let mut can_insert = true;
            if !bundles[index].is_available(ExecutionUnit::Alu) {
                can_insert = false;
            }
            // check for potential dependencies
            let mut s = 0;
            if index >= 2 {
                s = index - 2;
            }
            for j in s..=index {
                for unit in ExecutionUnit::iter() {
                    for prev_instr in bundles[j].iter(unit) {
                        if let Some(dest) = prev_instr.dest() {
                            if dest == mov.operands()[0] && unit.latency() as usize + j > index {
                                can_insert = false;
                            }
                        }
                    }
                }
            }
            // insert
            if can_insert {
                bundles[index].add(mov);
                has_inserted = true;
                break;
            }
        }
        // add new bundle then insert
        if !has_inserted {
            bundles.insert(insert_stop, VLIW::new());
            bundles[insert_stop].add(mov);
            insert_stop += 1;
        }
    }

    if insert_start < bundles.len() {
        if let Some(loop_instr) = bundles[insert_start].pop(ExecutionUnit::Branch) {
            bundles[insert_stop - 1].add(loop_instr);
        }
    }

    // add remaining unused register operands in scheduling order
    for bundle in &mut bundles {
        for unit_type in ExecutionUnit::iter() {
            for instr in bundle.iter(unit_type) {
                let mut renamed_operands: Vec<u8> = vec![];
                for operand in instr.operands() {
                    if operand == 0 {
                        renamed_operands.push(new_register);
                        new_register += 1;
                    } else {
                        renamed_operands.push(operand);
                    }
                }
                update_ops(instr, renamed_operands);
            }
        }
    }

    bundles
}

#[cfg(test)]
mod tests {
    use crate::dependency::*;
    use crate::scheduler::*;

    use crate::compute_initiation_interval;

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

    fn get_large_loop() -> Vec<Instruction> {
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
    fn compute_initiation_interval_works() {
        let program = vec![
            Instruction::Add(0, 0, 0),
            Instruction::Add(0, 0, 0),
            Instruction::Add(0, 0, 0),
            Instruction::Add(0, 0, 0),
            Instruction::Mulu(0, 0, 0),
            Instruction::Mulu(0, 0, 0),
            Instruction::Ld(0, 0, 0),
            Instruction::Ld(0, 0, 0),
            Instruction::Ld(0, 0, 0),
            Instruction::Ld(0, 0, 0),
            Instruction::Loop(0),
        ];
        assert_eq!(compute_initiation_interval(&program), 4);
    }

    #[test]
    fn get_local_schedule_works_bb0() {
        let program = get_simple_loop();
        let dep_table = get_dependency_table(&program);
        let schedule = get_local_schedule(&program[0..3], &dep_table, 0);
        let expected = vec![0, 0, 1];
        assert_eq!(schedule, expected);
    }

    #[test]
    fn get_local_schedule_works_bb2() {
        let program = get_simple_loop();
        let dep_table = get_dependency_table(&program);
        let schedule = get_local_schedule(&program[5..], &dep_table, 5);
        assert_eq!(schedule, vec![0]);
    }

    #[test]
    fn fix_schedule_offsets_works() {
        let program = get_simple_loop();
        let dep_table = get_dependency_table(&program);
        let mut schedule = get_local_schedule(&program[0..3], &dep_table, 0);
        let mut body = get_loop_schedule(&program[3..5], &dep_table, 3);
        let mut tail = get_loop_schedule(&program[5..], &dep_table, 5);
        schedule.append(&mut body);
        fix_schedule_offsets(&mut schedule, &program, &dep_table, 3);
        assert_eq!(schedule, vec![0, 0, 1, 4, 4]);
    }

    #[test]
    fn get_loop_schedule_works_on_small_loop() {
        let program = get_simple_loop();
        let dep_table = get_dependency_table(&program);
        let schedule = get_loop_schedule(&program[3..5], &dep_table, 3);
        let expected = vec![0, 0];
        assert_eq!(schedule, expected);
    }

    #[test]
    fn get_loop_schedule_works_on_large_loop() {
        let program = get_large_loop();
        let dep_table = get_dependency_table(&program);
        let schedule = get_loop_schedule(&program[4..10], &dep_table, 4);
        let expected = vec![0, 1, 2, 4, 0, 4];
        assert_eq!(schedule, expected);
    }

    #[test]
    fn get_asap_schedule_works_simple_loop() {
        let program = get_simple_loop();
        let dep_table = get_dependency_table(&program);
        let schedule = get_asap_schedule(&program, &dep_table, 3, 5);
        let expected = vec![0, 0, 1, 4, 4, 5];
        assert_eq!(schedule, expected);
    }

    #[test]
    fn get_asap_schedule_works_large_loop() {
        let program = get_large_loop();
        let dep_table = get_dependency_table(&program);
        let schedule = get_asap_schedule(&program, &dep_table, 4, 10);
        let expected = vec![0, 0, 1, 1, 2, 3, 4, 6, 2, 6, 7];
        assert_eq!(schedule, expected);
    }

    #[test]
    fn vliw_schedule_works_large_loop_dest() {
        let mut program = get_large_loop();
        let mut dep_table = get_dependency_table(&program);
        let mut bundles = get_vliw_schedule(&program, &dep_table);
        assert_eq!(bundles[0].iter(ExecutionUnit::Alu)[0].dest(), None);
        assert_eq!(bundles[0].iter(ExecutionUnit::Alu)[1].dest(), Some(1));
        assert_eq!(bundles[1].iter(ExecutionUnit::Alu)[0].dest(), Some(2));
        assert_eq!(bundles[1].iter(ExecutionUnit::Alu)[1].dest(), Some(3));
        assert_eq!(bundles[2].iter(ExecutionUnit::Alu)[0].dest(), Some(4));
        assert_eq!(bundles[2].iter(ExecutionUnit::Mem)[0].dest(), Some(5));
        assert_eq!(bundles[3].iter(ExecutionUnit::Mul)[0].dest(), Some(6));
        assert_eq!(bundles[4].iter(ExecutionUnit::Mul)[0].dest(), Some(7));
    }

    #[test]
    fn vliw_schedule_works_large_loop_ops() {
        let mut program = get_large_loop();
        let mut dep_table = get_dependency_table(&program);
        let mut bundles = get_vliw_schedule(&program, &dep_table);
        assert_eq!(bundles[2].iter(ExecutionUnit::Alu)[0].operands(), vec![1]);
        assert_eq!(bundles[2].iter(ExecutionUnit::Mem)[0].operands(), vec![1]);
        assert_eq!(
            bundles[3].iter(ExecutionUnit::Mul)[0].operands(),
            vec![5, 3]
        );
        assert_eq!(
            bundles[4].iter(ExecutionUnit::Mul)[0].operands(),
            vec![2, 5]
        );
        assert_eq!(bundles[6].iter(ExecutionUnit::Alu)[0].operands(), vec![4]);
        assert_eq!(bundles[7].iter(ExecutionUnit::Alu)[0].operands(), vec![7]);
        assert_eq!(
            bundles[8].iter(ExecutionUnit::Mem)[0].operands(),
            vec![7, 4]
        );
    }

    #[test]
    fn vliw_schedule_works_simple_loop_dest() {
        let mut program = get_simple_loop();
        let mut dep_table = get_dependency_table(&program);
        let mut bundles = get_vliw_schedule(&program, &dep_table);
        assert_eq!(bundles[0].iter(ExecutionUnit::Alu)[1].dest(), Some(1));
        assert_eq!(bundles[1].iter(ExecutionUnit::Mul)[0].dest(), Some(2));
        assert_eq!(bundles[4].iter(ExecutionUnit::Alu)[0].dest(), Some(3));
    }
}
