use crate::dependency::*;
use crate::instruction::*;

use std::cmp::max;

pub fn find_loop_bounds(program: &Vec<Instruction>) -> (usize, usize) {
    let mut loop_start = program.len();
    let mut loop_end = program.len();

    //Locate loop
    for (index, instr) in program.iter().enumerate() {
        if let Instruction::Loop(imm) | Instruction::LoopPip(imm) = instr {
            loop_start = *imm as usize;
            loop_end = index;
        }
    }
    (loop_start, loop_end)
}
fn compute_initiation_interval(program: &Vec<Instruction>) -> usize {
    let mut counts: [usize; 4] = [0; 4];

    let (loop_start, loop_end) = find_loop_bounds(program);

    for instr in &program.as_slice()[loop_start..=loop_end] {
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

fn internal_pip_scheduler(
    program: &Vec<Instruction>,
    dependency_table: &Vec<DependencyList>,
) -> (Vec<usize>, usize, usize, usize, usize) {
    let (loop_start, loop_end) = find_loop_bounds(program);
    //Data structure that holds for each PC of the program, on which bundle they're being assigned
    //At the beginning schedule only contains size for BB0
    let mut schedule: Vec<usize> = vec![0; loop_start];
    //Data structure that holds for each exec unit the operations being fit into it
    let mut used: Vec<Vec<usize>> = vec![Vec::new(); ExecutionUnit::iter().len()];
    //The idea being that I can fill a data structure that contains PER UNIT the schedule
    //format would be schedule[bundle_number] = <Alu0_PC, Alu1_PC, Store_PC, Mul_PC, Branch_PC>
    let mut max_bundle_number_bb0: usize = 0;

    //Schedule BB0
    let bb0 = &program.as_slice()[..=loop_start - 1];
    //For each instruction in the code
    for (pc, instr) in bb0.iter().enumerate() {
        //Get the Unit on which this instruction will go
        let unit_id = instr.unit() as usize;
        //For each of the writers writing on one of the sources registers of the current
        //instruction (for local dependencies, we can just push back the scheduling)
        for write_pc in dependency_table[pc].iter(Dependency::Local) {
            schedule[pc] = max(
                schedule[pc],
                schedule[*write_pc] + bb0[*write_pc].unit().latency() as usize,
            );
        }
        //While, for a given execution unit, we have already scheduled more instructions into that
        //unit than the decided bundle from dependencies, and there are no more rooms for a given
        //type of exec units at a bundle time, we push back the scheduling by 1
        while used[unit_id].len() > schedule[pc]
            && used[unit_id][schedule[pc]] >= instr.unit().nb_available()
        {
            schedule[pc] = schedule[pc] + 1;
        }
        max_bundle_number_bb0 = if schedule[pc] > max_bundle_number_bb0 {
            schedule[pc]
        } else {
            max_bundle_number_bb0
        };
        //If we have decided we are gonna schedule farther than an exec unit has been filled so
        //far, we match the current bundle number by filling with nops.
        while used[unit_id].len() <= schedule[pc] {
            used[instr.unit() as usize].push(0);
        }
        //We assign one more instruction to the given bundle number to a given unit
        used[unit_id][schedule[pc]] = used[unit_id][schedule[pc]] + 1;
    }

    if loop_start == program.len() {
        return (schedule, max_bundle_number_bb0, loop_start, loop_start, 1);
    }
    for (pc, bundle) in schedule.iter().enumerate() {
        max_bundle_number_bb0 = max(
            max_bundle_number_bb0,
            bundle + program[pc].unit().latency() as usize - 1,
        );
    }

    //Allocate space for the new basic block in the schedule and initialize it.
    let mut max_bundle_number_bb1: usize = if loop_start > 0 {
        max_bundle_number_bb0 + 1
    } else {
        0
    };
    schedule.append(&mut vec![
        max_bundle_number_bb1 - 1;
        loop_end - loop_start + 1
    ]);

    let clean_schedule = schedule.clone();
    let clean_used = used.clone();

    let mut ii: usize = compute_initiation_interval(program) - 1;
    let bb1 = &program.as_slice()[loop_start..=loop_end];
    //Will hold reservation information
    //While not valid schedule
    loop {
        let mut valid_schedule = true;
        ii += 1;
        schedule = clean_schedule.clone();
        used = clean_used.clone();
        let mut reserved_table: Vec<[usize; 4]> = vec![[0; 4]; ii];
        max_bundle_number_bb1 = max_bundle_number_bb0 + 1;
        'outer: for (pc, instr) in bb1.iter().enumerate() {
            //Get the Unit on which this instruction will go
            let unit_id = instr.unit() as usize;
            //For each of the writers writing on one of the sources registers of the current
            let mut supposed_bundle_pc: usize = max_bundle_number_bb0 + 1;
            // Brute-force scheduling based on all dependencies.
            for dep_type in Dependency::iter() {
                for write_pc in dependency_table[pc + loop_start].iter(dep_type) {
                    supposed_bundle_pc = max(
                        supposed_bundle_pc,
                        schedule[*write_pc] + program[*write_pc].unit().latency() as usize,
                    );
                }
                //While, for a given execution unit, we have already scheduled more instructions into that
                //unit than the decided bundle from dependencies, and there are no more rooms for a given
                //type of exec units at a bundle time, we push back the scheduling by 1
                while used[unit_id].len() > supposed_bundle_pc
                    && used[unit_id][supposed_bundle_pc] >= instr.unit().nb_available()
                {
                    supposed_bundle_pc += 1;
                }
            }
            let mut scheduled = false;

            //We check if there room for reserved slots (this forces a new loop as we have a
            //separate data structure)
            for i in 0..ii {
                let index = (supposed_bundle_pc - (max_bundle_number_bb0 + 1)) % ii + i;
                if reserved_table[index][unit_id] >= instr.unit().nb_available() {
                } else {
                    reserved_table[index][unit_id] += 1;
                    scheduled = true;
                    if let Instruction::Loop(_) | Instruction::LoopPip(_) = instr {
                        supposed_bundle_pc += ii - 1;
                        schedule[pc + loop_start] = supposed_bundle_pc + i;
                    } else {
                        schedule[pc + loop_start] = supposed_bundle_pc + i;
                    }
                    break;
                }
            }
            //We didn't manage to schedule based on the reservation information
            if !scheduled {
                valid_schedule = false;
                break 'outer;
            }

            for write_pc in dependency_table[pc + loop_start].iter(Dependency::InterLoop) {
                if schedule[*write_pc] + program[*write_pc].unit().latency() as usize
                    > schedule[pc + loop_start] + ii
                {
                    valid_schedule = false;
                    break 'outer;
                }
            }

            max_bundle_number_bb1 = if supposed_bundle_pc > max_bundle_number_bb1 {
                schedule[pc + loop_start]
            } else {
                max_bundle_number_bb1
            };
            //If we have decided we are gonna schedule farther than an exec unit has been filled so
            //far, we match the current bundle number by filling with nops.
            while used[unit_id].len() <= schedule[pc + loop_start] {
                used[instr.unit() as usize].push(0);
            }
            //We assign one more instruction to the given bundle number to a given unit
            used[unit_id][schedule[pc + loop_start]] = used[unit_id][schedule[pc + loop_start]] + 1;
        }
        if valid_schedule {
            break;
        }
        //End loop
    }

    max_bundle_number_bb1 = if loop_start == loop_end {
        max_bundle_number_bb0
    } else {
        max_bundle_number_bb1
    };

    //If no BB2
    if loop_end + 1 == program.len() {
        return (
            schedule,
            max_bundle_number_bb0,
            max_bundle_number_bb1,
            max_bundle_number_bb1,
            ii,
        );
    }
    let offset = (ii - (max_bundle_number_bb1 - max_bundle_number_bb0) % ii) % ii;
    max_bundle_number_bb1 += offset;
    let bb2 = &program.as_slice()[loop_end + 1..];
    let mut max_bundle_number_bb2 = max_bundle_number_bb1 + 1;
    //For each instruction in the code
    schedule.append(&mut vec![
        max_bundle_number_bb2;
        program.len() - schedule.len()
    ]);
    for (pc, instr) in bb2.iter().enumerate() {
        let effective_pc: usize = pc + loop_end + 1;
        //Get the Unit on which this instruction will go
        let unit_id = instr.unit() as usize;
        //For each of the writers writing on one of the sources registers of the current
        //instruction (for local dependencies, we can just push back the scheduling)
        for dep_type in Dependency::iter() {
            for write_pc in dependency_table[effective_pc].iter(dep_type) {
                schedule[effective_pc] = max(
                    schedule[effective_pc],
                    schedule[*write_pc] + program[*write_pc].unit().latency() as usize,
                );
            }
        }
        //While, for a given execution unit, we have already scheduled more instructions into that
        //unit than the decided bundle from dependencies, and there are no more rooms for a given
        //type of exec units at a bundle time, we push back the scheduling by 1
        while used[unit_id].len() > schedule[effective_pc]
            && used[unit_id][schedule[effective_pc]] >= instr.unit().nb_available()
        {
            schedule[effective_pc] = schedule[effective_pc] + 1;
        }
        max_bundle_number_bb2 = if schedule[effective_pc] > max_bundle_number_bb2 {
            schedule[effective_pc]
        } else {
            max_bundle_number_bb2
        };
        //If we have decided we are gonna schedule farther than an exec unit has been filled so
        //far, we match the current bundle number by filling with nops.
        while used[unit_id].len() <= schedule[effective_pc] {
            used[instr.unit() as usize].push(0);
        }
        //We assign one more instruction to the given bundle number to a given unit
        used[unit_id][schedule[effective_pc]] = used[unit_id][schedule[effective_pc]] + 1;
    }
    (
        schedule,
        max_bundle_number_bb0,
        max_bundle_number_bb1,
        max_bundle_number_bb2,
        ii,
    )
}

fn format_scheduler(
    program: Vec<Instruction>,
    schedule: Vec<usize>,
    max_bundle_number: usize,
) -> (Vec<[Instruction; 5]>, Vec<(usize, usize)>) {
    let mut final_schedule: Vec<[Instruction; 5]> =
        vec![[Instruction::Nop; 5]; max_bundle_number + 1];
    let mut placement_map: Vec<(usize, usize)> = vec![(0, 0); program.len()];
    for (orig_pc, bundle_pc) in schedule.iter().enumerate() {
        let unit = program[orig_pc].unit();
        //Match explicited for code clarity purposes (and double ALU handling)
        match unit {
            ExecutionUnit::Alu => {
                //Determine ALU allocation. Because we operate on original PCs, we are ensured that
                //lowest PCs get matched to ALU 0 always (trying to preserve scheduling ordering)
                let alu_index = match final_schedule[*bundle_pc][0] {
                    Instruction::Nop => 0,
                    _ => 1,
                };
                final_schedule[*bundle_pc][alu_index] = program[orig_pc];
                placement_map[orig_pc] = (*bundle_pc, alu_index);
            }
            //Indices are offset by 1 because the enum only defines one ALU
            ExecutionUnit::Mul => {
                final_schedule[*bundle_pc][(unit as usize) + 1] = program[orig_pc];
                placement_map[orig_pc] = (*bundle_pc, unit as usize + 1)
            }
            ExecutionUnit::Mem => {
                final_schedule[*bundle_pc][(unit as usize) + 1] = program[orig_pc];
                placement_map[orig_pc] = (*bundle_pc, unit as usize + 1)
            }
            ExecutionUnit::Branch => {
                if let Instruction::Loop(imm) = program[orig_pc] {
                    final_schedule[*bundle_pc][(unit as usize) + 1] = Instruction::LoopPip(imm);
                }
                placement_map[orig_pc] = (*bundle_pc, unit as usize + 1)
            }
        }
    }
    (final_schedule, placement_map)
}

pub fn loop_pip_schedule(
    program: Vec<Instruction>,
) -> (
    Vec<[Instruction; 5]>,
    Vec<(usize, usize)>,
    usize,
    usize,
    usize,
    usize,
) {
    let dependency_table = get_dependency_table(&program);
    let (linear_schedule, max_bundle_number_bb0, max_bundle_number_bb1, max_bundle_number_bb2, ii) =
        internal_pip_scheduler(&program, &dependency_table);

    let (final_schedule, placement_map) =
        format_scheduler(program, linear_schedule, max_bundle_number_bb2);
    (
        final_schedule,
        placement_map,
        max_bundle_number_bb0,
        max_bundle_number_bb1,
        max_bundle_number_bb2,
        ii,
    )
}

#[cfg(test)]
mod tests {
    use crate::instruction::*;
    use crate::pip_scheduler::*;

    #[test]
    fn compute_initiation_interval_works() {
        let mut program: Vec<Instruction> = Vec::new();
        program.push(Instruction::MovL(SpecialRegister::LoopCount, 100));
        program.push(Instruction::MovI(2, 0x1000));
        program.push(Instruction::MovI(3, 1));
        program.push(Instruction::MovI(4, 25));
        program.push(Instruction::Ld(5, 0, 2));
        program.push(Instruction::Mulu(6, 5, 4));
        program.push(Instruction::Mulu(3, 3, 5));
        program.push(Instruction::St(6, 0, 2));
        program.push(Instruction::Addi(2, 2, 1));
        program.push(Instruction::LoopPip(4));
        program.push(Instruction::St(3, 0, 2));
        assert_eq!(compute_initiation_interval(&program), 2);
    }
}
