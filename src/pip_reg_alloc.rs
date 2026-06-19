use crate::dependency::*;
use crate::instruction::*;
use crate::pip_scheduler::*;
use std::collections::HashSet;

fn realloc_operands(
    source_instruction: Instruction,
    old_dest_reg: u8,
    remap_value: u8,
) -> Instruction {
    let mut new_instr = source_instruction.clone();

    if source_instruction
        .operands()
        .iter()
        .any(|&op| op == old_dest_reg)
    {
        new_instr = match source_instruction {
            Instruction::Add(dest, opa, opb) => Instruction::Add(
                dest,
                if opa == old_dest_reg {
                    remap_value
                } else {
                    opa
                },
                if opb == old_dest_reg {
                    remap_value
                } else {
                    opb
                },
            ),
            Instruction::Addi(dest, _, imm) => Instruction::Addi(dest, remap_value, imm),
            Instruction::Sub(dest, opa, opb) => Instruction::Sub(
                dest,
                if opa == old_dest_reg {
                    remap_value
                } else {
                    opa
                },
                if opb == old_dest_reg {
                    remap_value
                } else {
                    opb
                },
            ),
            Instruction::Mulu(dest, opa, opb) => Instruction::Mulu(
                dest,
                if opa == old_dest_reg {
                    remap_value
                } else {
                    opa
                },
                if opb == old_dest_reg {
                    remap_value
                } else {
                    opb
                },
            ),
            Instruction::Ld(dest, imm, _) => Instruction::Ld(dest, imm, remap_value),
            Instruction::St(source, imm, addr) => Instruction::St(
                if source == old_dest_reg {
                    remap_value
                } else {
                    source
                },
                imm,
                if addr == old_dest_reg {
                    remap_value
                } else {
                    addr
                },
            ),
            Instruction::Mov(dest, _) => Instruction::Mov(dest, remap_value),
            _ => source_instruction,
        };
    }
    new_instr
}

pub fn register_realloc(
    program: &Vec<Instruction>,
    placement_map: Vec<(usize, usize)>,
    schedule: &mut Vec<[Instruction; 5]>,
    max_bundle_number_bb0: usize,
    max_bundle_number_bb1: usize,
    max_bundle_number_bb2: usize,
    ii: usize,
) {
    let mut remapped_reg_bb1 = 32;
    let clean_schedule: Vec<[Instruction; 5]> = (*schedule).clone();
    let mut used_registers: Vec<u8> = Vec::new();
    let mut allocated_registers: Vec<u8> = Vec::new();
    let dependency_table = get_dependency_table(&program);
    let nb_stages: u8 = ((max_bundle_number_bb1 - max_bundle_number_bb0) / ii) as u8;
    //Stage 1 of alloc_r
    let first_bb1_bundle_number = max_bundle_number_bb0 + 1;

    for (bundle_pc, bundle) in schedule.clone().as_slice()
        [first_bb1_bundle_number..=max_bundle_number_bb1]
        .iter()
        .enumerate()
    {
        let mut new_bundle: [Instruction; 5] = bundle.clone();
        for (slot_index, instr) in bundle.iter().enumerate() {
            let mut new_instr: Instruction = instr.clone();
            //Only remap if it produces new values
            if let Some(dest) = instr.dest() {
                new_instr = match instr {
                    Instruction::Add(_, opa, opb) => Instruction::Add(remapped_reg_bb1, *opa, *opb),
                    Instruction::Addi(_, opa, imm) => {
                        Instruction::Addi(remapped_reg_bb1, *opa, *imm)
                    }
                    Instruction::Sub(_, opa, opb) => Instruction::Sub(remapped_reg_bb1, *opa, *opb),
                    Instruction::Mulu(_, opa, opb) => {
                        Instruction::Mulu(remapped_reg_bb1, *opa, *opb)
                    }
                    Instruction::Ld(_, imm, addr) => Instruction::Ld(remapped_reg_bb1, *imm, *addr),
                    Instruction::Mov(_, src) => Instruction::Mov(remapped_reg_bb1, *src),
                    Instruction::MovI(_, imm) => Instruction::MovI(remapped_reg_bb1, *imm),
                    // Instruction::MovP(_, pred) => MovP(remapped_reg_bb1, pred),
                    _ => *instr,
                };
                used_registers.push(dest);
                allocated_registers.push(remapped_reg_bb1);
                remapped_reg_bb1 += nb_stages + 1;
            }
            new_bundle[slot_index] = new_instr.clone()
            //End of iterating through a bundle
        }
        schedule[bundle_pc + first_bb1_bundle_number] = new_bundle.clone();

        //End of schedule iteration
    }
    //
    //Remap destination registers for BB0
    let mut remapped_loop_inv_reg = 1;
    for (bundle_pc, bundle) in schedule.clone().as_slice()[..=max_bundle_number_bb0]
        .iter()
        .enumerate()
    {
        let mut new_bundle: [Instruction; 5] = bundle.clone();
        for (slot_index, instr) in bundle.iter().enumerate() {
            let mut new_instr: Instruction = instr.clone();
            //Only remap if it produces new values
            if let Some(dest) = instr.dest() {
                if let Some(orig_ins_pc) = placement_map
                    .iter()
                    .position(|&x| x == (bundle_pc, slot_index))
                {
                    if get_depended_upon(program, &dependency_table, orig_ins_pc).is_empty() {
                        new_instr = match instr {
                            Instruction::Add(_, opa, opb) => {
                                Instruction::Add(remapped_loop_inv_reg, *opa, *opb)
                            }
                            Instruction::Addi(_, opa, imm) => {
                                Instruction::Addi(remapped_loop_inv_reg, *opa, *imm)
                            }
                            Instruction::Sub(_, opa, opb) => {
                                Instruction::Sub(remapped_loop_inv_reg, *opa, *opb)
                            }
                            Instruction::Mulu(_, opa, opb) => {
                                Instruction::Mulu(remapped_loop_inv_reg, *opa, *opb)
                            }
                            Instruction::Ld(_, imm, addr) => {
                                Instruction::Ld(remapped_loop_inv_reg, *imm, *addr)
                            }
                            Instruction::Mov(_, src) => {
                                Instruction::Mov(remapped_loop_inv_reg, *src)
                            }
                            Instruction::MovI(_, imm) => {
                                Instruction::MovI(remapped_loop_inv_reg, *imm)
                            }
                            // Instruction::MovP(_, pred) => MovP(remapped_reg_bb1, pred),
                            _ => *instr,
                        };
                        used_registers.push(dest);
                        allocated_registers.push(remapped_loop_inv_reg);
                        remapped_loop_inv_reg += 1;
                    }
                }
            }
            new_bundle[slot_index] = new_instr.clone()
            //End of iterating through a bundle
        }
        schedule[bundle_pc] = new_bundle.clone();
        //End of schedule iteration
    }

    //if BB2 exists
    if !(max_bundle_number_bb1 == max_bundle_number_bb2) {
        //Remap destination registers for BB2
        for (bundle_pc, bundle) in schedule.clone().as_slice()[max_bundle_number_bb1 + 1..]
            .iter()
            .enumerate()
        {
            let mut new_bundle: [Instruction; 5] = bundle.clone();
            for (slot_index, instr) in bundle.iter().enumerate() {
                let mut new_instr: Instruction = instr.clone();
                //Only remap if it produces new values
                if let Some(dest) = instr.dest() {
                    if let Some(orig_ins_pc) = placement_map
                        .iter()
                        .position(|&x| x == (bundle_pc + max_bundle_number_bb1 + 1, slot_index))
                    {
                        if get_depended_upon(program, &dependency_table, orig_ins_pc).is_empty() {
                            new_instr = match instr {
                                Instruction::Add(_, opa, opb) => {
                                    Instruction::Add(remapped_loop_inv_reg, *opa, *opb)
                                }
                                Instruction::Addi(_, opa, imm) => {
                                    Instruction::Addi(remapped_loop_inv_reg, *opa, *imm)
                                }
                                Instruction::Sub(_, opa, opb) => {
                                    Instruction::Sub(remapped_loop_inv_reg, *opa, *opb)
                                }
                                Instruction::Mulu(_, opa, opb) => {
                                    Instruction::Mulu(remapped_loop_inv_reg, *opa, *opb)
                                }
                                Instruction::Ld(_, imm, addr) => {
                                    Instruction::Ld(remapped_loop_inv_reg, *imm, *addr)
                                }
                                Instruction::Mov(_, src) => {
                                    Instruction::Mov(remapped_loop_inv_reg, *src)
                                }
                                Instruction::MovI(_, imm) => {
                                    Instruction::MovI(remapped_loop_inv_reg, *imm)
                                }
                                // Instruction::MovP(_, pred) => MovP(remapped_reg_bb1, pred),
                                _ => *instr,
                            };
                            used_registers.push(dest);
                            allocated_registers.push(remapped_loop_inv_reg);
                            remapped_loop_inv_reg += 1;
                        }
                    }
                }
                new_bundle[slot_index] = new_instr.clone()
                //End of iterating through a bundle
            }
            schedule[bundle_pc + max_bundle_number_bb1 + 1] = new_bundle.clone();
            //End of schedule iteration
        }
    }

    //Phase 2 of alloc_r (loop_invariant)
    for (pc, _) in program.iter().enumerate() {
        let mut inv_deps = HashSet::new();
        for write_pc in dependency_table[pc].iter(Dependency::LoopInvariant) {
            inv_deps.insert(*write_pc);
        }

        for write_pc in inv_deps.iter() {
            let mut new_bundle: [Instruction; 5] = schedule[placement_map[*write_pc].0].clone();
            for (slot_index, bundle_ins) in new_bundle.clone().iter().enumerate() {
                let mut new_instr: Instruction = bundle_ins.clone();
                if *bundle_ins == program[*write_pc] {
                    //Only remap if it produces new values
                    if let Some(dest) = bundle_ins.dest() {
                        if !(used_registers.contains(&dest)) {
                            //Remap on the destination side
                            new_instr = match bundle_ins {
                                Instruction::Add(_, opa, opb) => {
                                    Instruction::Add(remapped_loop_inv_reg, *opa, *opb)
                                }
                                Instruction::Addi(_, opa, imm) => {
                                    Instruction::Addi(remapped_loop_inv_reg, *opa, *imm)
                                }
                                Instruction::Sub(_, opa, opb) => {
                                    Instruction::Sub(remapped_loop_inv_reg, *opa, *opb)
                                }
                                Instruction::Mulu(_, opa, opb) => {
                                    Instruction::Mulu(remapped_loop_inv_reg, *opa, *opb)
                                }
                                Instruction::Ld(_, imm, addr) => {
                                    Instruction::Ld(remapped_loop_inv_reg, *imm, *addr)
                                }
                                Instruction::Mov(_, src) => {
                                    Instruction::Mov(remapped_loop_inv_reg, *src)
                                }
                                Instruction::MovI(_, imm) => {
                                    Instruction::MovI(remapped_loop_inv_reg, *imm)
                                }
                                // Instruction::MovP(_, pred) => MovP(remapped_reg_bb1, pred),
                                _ => *bundle_ins,
                            };
                            used_registers.push(dest);
                            allocated_registers.push(remapped_loop_inv_reg);
                            remapped_loop_inv_reg += 1;
                        }
                    }
                }
                new_bundle[slot_index] = new_instr;
            }
            schedule[placement_map[*write_pc].0] = new_bundle;
        }
    }

    //Phase 3 of alloc_r
    //Check over all bundles
    for (bundle_pc, bundle) in schedule.clone().as_slice()
        [first_bb1_bundle_number..=max_bundle_number_bb1]
        .iter()
        .enumerate()
    {
        //Check over all instructions in that bundle
        for (slot_index, _) in bundle.iter().enumerate() {
            //Find the original PC corresponding to that slotted instruction
            if let Some(orig_ins_pc) = placement_map
                .iter()
                .position(|&x| x == (bundle_pc + first_bb1_bundle_number, slot_index))
            {
                //Loop Invariant dependencies
                for write_pc in dependency_table[orig_ins_pc].iter(Dependency::LoopInvariant) {
                    let remap_value =
                        schedule[placement_map[*write_pc].0][placement_map[*write_pc].1].dest();
                    let remapped = realloc_operands(
                        schedule[bundle_pc + first_bb1_bundle_number][slot_index],
                        program[*write_pc].dest().expect("Op has no dest"),
                        remap_value.expect("No remap value was found"),
                    );

                    schedule[bundle_pc + first_bb1_bundle_number][slot_index] = remapped.clone();
                }

                for write_pc in dependency_table[orig_ins_pc].iter(Dependency::Local) {
                    let remap_value = schedule[placement_map[*write_pc].0]
                        [placement_map[*write_pc].1]
                        .dest()
                        .unwrap()
                        + (((bundle_pc) / ii)
                            - ((placement_map[*write_pc].0 - first_bb1_bundle_number) / ii))
                            as u8;

                    let remapped = realloc_operands(
                        schedule[bundle_pc + first_bb1_bundle_number][slot_index],
                        program[*write_pc].dest().expect("Op has no dest"),
                        remap_value,
                    );
                    allocated_registers.push(remap_value);
                    schedule[bundle_pc + first_bb1_bundle_number][slot_index] = remapped.clone();
                }

                for write_pc in dependency_table[orig_ins_pc].iter(Dependency::InterLoop) {
                    if placement_map[*write_pc].0 < first_bb1_bundle_number {
                        continue;
                    }
                    let remap_value = schedule[placement_map[*write_pc].0]
                        [placement_map[*write_pc].1]
                        .dest()
                        .unwrap()
                        + (((bundle_pc + first_bb1_bundle_number - max_bundle_number_bb0) / ii)
                            - ((placement_map[*write_pc].0 - max_bundle_number_bb0) / ii))
                            as u8
                        + 1;
                    let remapped = realloc_operands(
                        schedule[bundle_pc + first_bb1_bundle_number][slot_index],
                        program[*write_pc].dest().expect("Op has no dest"),
                        remap_value,
                    );

                    allocated_registers.push(remap_value);
                    schedule[bundle_pc + first_bb1_bundle_number][slot_index] = remapped.clone();
                }
            }
        }
    }

    //Phase 4 of alloc_r
    let (loop_start, loop_end) = find_loop_bounds(program);

    //Case 1: If BB0 produces, and BB1 consumes from BB0 as well as interloop from another ins
    //in BB1
    for (bb0_pc, _) in program.as_slice()[..loop_start].iter().enumerate() {
        for (bb1_pc, _) in program.as_slice()[loop_start..=loop_end].iter().enumerate() {
            //If depends on BB0
            if dependency_table[bb1_pc + loop_start]
                .iter(Dependency::InterLoop)
                .contains(&bb0_pc)
            {
                for (bb1_pc_bis, _) in program.as_slice()[loop_start..=loop_end].iter().enumerate()
                {
                    if dependency_table[bb1_pc + loop_start]
                        .iter(Dependency::InterLoop)
                        .contains(&(bb1_pc_bis + loop_start))
                    {
                        let remap_value = schedule[placement_map[bb1_pc_bis + loop_start].0]
                            [placement_map[bb1_pc_bis + loop_start].1]
                            .dest()
                            .unwrap()
                            - ((placement_map[bb1_pc_bis + loop_start].0 - first_bb1_bundle_number)
                                / ii) as u8
                            + 1;
                        if let Some(_) =
                            schedule[placement_map[bb0_pc].0][placement_map[bb0_pc].1].dest()
                        {
                            let mut new_instr =
                                schedule[placement_map[bb0_pc].0][placement_map[bb0_pc].1].clone();

                            new_instr = match new_instr {
                                Instruction::Add(_, opa, opb) => {
                                    Instruction::Add(remap_value, opa, opb)
                                }
                                Instruction::Addi(_, opa, imm) => {
                                    Instruction::Addi(remap_value, opa, imm)
                                }
                                Instruction::Sub(_, opa, opb) => {
                                    Instruction::Sub(remap_value, opa, opb)
                                }
                                Instruction::Mulu(_, opa, opb) => {
                                    Instruction::Mulu(remap_value, opa, opb)
                                }
                                Instruction::Ld(_, imm, addr) => {
                                    Instruction::Ld(remap_value, imm, addr)
                                }
                                Instruction::Mov(_, src) => Instruction::Mov(remap_value, src),
                                Instruction::MovI(_, imm) => Instruction::MovI(remap_value, imm),
                                // Instruction::MovP(_, pred) => MovP(remapped_reg_bb1, pred),
                                _ => new_instr,
                            };
                            allocated_registers.push(remap_value);
                            schedule[placement_map[bb0_pc].0][placement_map[bb0_pc].1] =
                                new_instr.clone();
                        }
                    }
                }
            }
        }
    }

    //Case 2 and 4
    for (pc, _) in program.as_slice()[..loop_start].iter().enumerate() {
        for write_pc in dependency_table[pc].iter(Dependency::Local) {
            let mut new_bundle: [Instruction; 5] = schedule[placement_map[*write_pc].0].clone();
            for (slot_index, bundle_ins) in new_bundle.clone().iter().enumerate() {
                let mut new_instr: Instruction = bundle_ins.clone();
                if *bundle_ins == schedule[placement_map[*write_pc].0][placement_map[*write_pc].1] {
                    //Only remap if it produces new values
                    if let Some(dest) = bundle_ins.dest() {
                        //Remap on the destination side
                        new_instr = match bundle_ins {
                            Instruction::Add(_, opa, opb) => {
                                Instruction::Add(remapped_loop_inv_reg, *opa, *opb)
                            }
                            Instruction::Addi(_, opa, imm) => {
                                Instruction::Addi(remapped_loop_inv_reg, *opa, *imm)
                            }
                            Instruction::Sub(_, opa, opb) => {
                                Instruction::Sub(remapped_loop_inv_reg, *opa, *opb)
                            }
                            Instruction::Mulu(_, opa, opb) => {
                                Instruction::Mulu(remapped_loop_inv_reg, *opa, *opb)
                            }
                            Instruction::Ld(_, imm, addr) => {
                                Instruction::Ld(remapped_loop_inv_reg, *imm, *addr)
                            }
                            Instruction::Mov(_, src) => {
                                Instruction::Mov(remapped_loop_inv_reg, *src)
                            }
                            Instruction::MovI(_, imm) => {
                                Instruction::MovI(remapped_loop_inv_reg, *imm)
                            }
                            // Instruction::MovP(_, pred) => MovP(remapped_reg_bb1, pred),
                            _ => *bundle_ins,
                        };
                        used_registers.push(dest);
                        allocated_registers.push(remapped_loop_inv_reg);
                        remapped_loop_inv_reg += 1;
                    }
                }
                new_bundle[slot_index] = new_instr;
            }
            schedule[placement_map[*write_pc].0] = new_bundle;
        }

        for write_pc in dependency_table[pc].iter(Dependency::Local) {
            let remap_value =
                schedule[placement_map[*write_pc].0][placement_map[*write_pc].1].dest();
            let remapped = realloc_operands(
                schedule[placement_map[pc].0][placement_map[pc].1],
                program[*write_pc].dest().expect("Op has no dest"),
                remap_value.expect("No remap value was found"),
            );

            schedule[placement_map[pc].0][placement_map[pc].1] = remapped.clone();
        }

        for write_pc in dependency_table[pc].iter(Dependency::LoopInvariant) {
            let remap_value =
                schedule[placement_map[*write_pc].0][placement_map[*write_pc].1].dest();
            let remapped = realloc_operands(
                schedule[placement_map[pc].0][placement_map[pc].1],
                program[*write_pc].dest().expect("Op has no dest"),
                remap_value.expect("No remap value was found"),
            );

            schedule[placement_map[pc].0][placement_map[pc].1] = remapped.clone();
        }
    }

    //BB2 local dependency
    for (pc, _) in program.as_slice()[loop_end + 1..].iter().enumerate() {
        for write_pc in dependency_table[pc + loop_end + 1].iter(Dependency::Local) {
            let mut new_bundle: [Instruction; 5] = schedule[placement_map[*write_pc].0].clone();
            for (slot_index, bundle_ins) in new_bundle.clone().iter().enumerate() {
                let mut new_instr: Instruction = bundle_ins.clone();
                if *bundle_ins == schedule[placement_map[*write_pc].0][placement_map[*write_pc].1] {
                    //Only remap if it produces new values
                    if let Some(dest) = bundle_ins.dest() {
                        //Remap on the destination side
                        new_instr = match bundle_ins {
                            Instruction::Add(_, opa, opb) => {
                                Instruction::Add(remapped_loop_inv_reg, *opa, *opb)
                            }
                            Instruction::Addi(_, opa, imm) => {
                                Instruction::Addi(remapped_loop_inv_reg, *opa, *imm)
                            }
                            Instruction::Sub(_, opa, opb) => {
                                Instruction::Sub(remapped_loop_inv_reg, *opa, *opb)
                            }
                            Instruction::Mulu(_, opa, opb) => {
                                Instruction::Mulu(remapped_loop_inv_reg, *opa, *opb)
                            }
                            Instruction::Ld(_, imm, addr) => {
                                Instruction::Ld(remapped_loop_inv_reg, *imm, *addr)
                            }
                            Instruction::Mov(_, src) => {
                                Instruction::Mov(remapped_loop_inv_reg, *src)
                            }
                            Instruction::MovI(_, imm) => {
                                Instruction::MovI(remapped_loop_inv_reg, *imm)
                            }
                            // Instruction::MovP(_, pred) => MovP(remapped_reg_bb1, pred),
                            _ => *bundle_ins,
                        };
                        used_registers.push(dest);
                        allocated_registers.push(remapped_loop_inv_reg);
                        remapped_loop_inv_reg += 1;
                    }
                }
                new_bundle[slot_index] = new_instr;
            }
            schedule[placement_map[*write_pc].0] = new_bundle;
        }

        for write_pc in dependency_table[pc].iter(Dependency::Local) {
            let remap_value =
                schedule[placement_map[*write_pc].0][placement_map[*write_pc].1].dest();
            let remapped = realloc_operands(
                schedule[placement_map[pc].0][placement_map[pc].1],
                program[*write_pc].dest().expect("Op has no dest"),
                remap_value.expect("No remap value was found"),
            );

            schedule[placement_map[pc].0][placement_map[pc].1] = remapped.clone();
        }

        for write_pc in dependency_table[pc].iter(Dependency::LoopInvariant) {
            let remap_value =
                schedule[placement_map[*write_pc].0][placement_map[*write_pc].1].dest();
            let remapped = realloc_operands(
                schedule[placement_map[pc + loop_end + 1].0][placement_map[pc + loop_end].1],
                program[*write_pc].dest().expect("Op has no dest"),
                remap_value.expect("No remap value was found"),
            );

            schedule[placement_map[pc + loop_end + 1].0][placement_map[pc + loop_end + 1].1] =
                remapped.clone();
        }
    }

    //Case 3: Post dep in BB2
    for (bb2_pc, _) in program.as_slice()[loop_end..].iter().enumerate() {
        for write_pc in dependency_table[bb2_pc + loop_end].iter(Dependency::PostLoop) {
            let remap_value = schedule[placement_map[*write_pc].0][placement_map[*write_pc].1]
                .dest()
                .unwrap()
                + (nb_stages - 1)
                - ((placement_map[*write_pc].0 - first_bb1_bundle_number) / ii) as u8;
            let remapped = realloc_operands(
                schedule[placement_map[bb2_pc + loop_end].0][placement_map[bb2_pc + loop_end].1],
                program[*write_pc].dest().expect("Op has no dest"),
                remap_value,
            );

            allocated_registers.push(remap_value);
            schedule[placement_map[bb2_pc + loop_end].0][placement_map[bb2_pc + loop_end].1] =
                remapped.clone();
        }
    }

    //Check if we are reading from a register no one has used before
    for (bundle_pc, bundle) in clean_schedule.clone().iter().enumerate() {
        for (slot_index, bundle_ins) in bundle.iter().enumerate() {
            let mut unused_register;
            let ops = bundle_ins.operands();
            let mut remapped = schedule[bundle_pc][slot_index].clone();
            for operand in ops.iter() {
                if !used_registers.contains(operand) {
                    let mut iterator = 1;
                    unused_register = loop {
                        if !(allocated_registers.contains(&iterator)) {
                            break iterator;
                        }
                        iterator += 1;
                        if iterator > 95 {
                            break 0;
                        }
                    };
                    allocated_registers.push(unused_register);
                    remapped = realloc_operands(remapped, *operand, unused_register);
                }
            }
            schedule[bundle_pc][slot_index] = remapped.clone();
        }
    }
}

pub fn prepare_loop(
    schedule: &Vec<[Instruction; 5]>,
    ii: usize,
    max_bundle_number_bb0: usize,
    max_bundle_number_bb1: usize,
) -> Vec<[PredicatedInstruction; 5]> {
    let mut final_schedule: Vec<[PredicatedInstruction; 5]> = Vec::new();
    let mut compressed_loop: Vec<[PredicatedInstruction; 5]> =
        vec![[PredicatedInstruction::Nop; 5]; ii];
    for bundle in schedule.as_slice()[..=max_bundle_number_bb0].iter() {
        let mut pred_bundle: [PredicatedInstruction; 5] = [PredicatedInstruction::Nop; 5];
        for (slot_index, bundle_ins) in bundle.clone().iter().enumerate() {
            pred_bundle[slot_index] = PredicatedInstruction::from(bundle_ins, 0);
        }
        final_schedule.push(pred_bundle.clone());
    }

    let p32_ins: PredicatedInstruction = PredicatedInstruction::MovP(32, true);
    let ec_val = (max_bundle_number_bb1 - max_bundle_number_bb0) / ii - 1;
    let ec_ins: PredicatedInstruction =
        PredicatedInstruction::MovL(SpecialRegister::EpCount, ec_val as i16);
    //If there is room for at least one instruction in the current bundle
    let mut alu0_available = false;
    let mut alu1_available = false;
    if let PredicatedInstruction::Nop = final_schedule[max_bundle_number_bb0][0] {
        alu0_available = true;
        if let PredicatedInstruction::Nop = final_schedule[max_bundle_number_bb0][1] {
            alu1_available = true;
        }
    }

    if let PredicatedInstruction::Nop = final_schedule[max_bundle_number_bb0][1] {
        alu1_available = true;
    }
    let mut first_bb1_bundle_number = max_bundle_number_bb0 + 1;
    if !(alu0_available && alu1_available) {
        final_schedule.push([PredicatedInstruction::Nop; 5]);
        first_bb1_bundle_number += 1;
    }
    if alu0_available {
        final_schedule[max_bundle_number_bb0][0] = p32_ins;
    } else {
        final_schedule[max_bundle_number_bb0 + 1][0] = p32_ins;
    }

    if alu1_available {
        final_schedule[max_bundle_number_bb0][1] = ec_ins;
    } else {
        final_schedule[max_bundle_number_bb0 + 1][1] = ec_ins;
    }

    //For all bundles in loop
    for (bundle_pc, bundle) in schedule.as_slice()
        [max_bundle_number_bb0 + 1..=max_bundle_number_bb1]
        .iter()
        .enumerate()
    {
        let index = (bundle_pc) % ii;
        for (slot_index, bundle_ins) in bundle.clone().iter().enumerate() {
            if let PredicatedInstruction::Nop = compressed_loop[index][slot_index] {
                let current_stage = 32 + bundle_pc / ii;
                if let Instruction::Nop = bundle_ins {
                } else {
                    if let Instruction::LoopPip(_) = *bundle_ins {
                        compressed_loop[index][slot_index] =
                            PredicatedInstruction::LoopPip(first_bb1_bundle_number);
                    } else {
                        compressed_loop[index][slot_index] =
                            PredicatedInstruction::from(bundle_ins, current_stage as u8);
                    }
                }
            }
        }
    }

    final_schedule.extend(compressed_loop.clone());

    if max_bundle_number_bb1 == schedule.len() {
        return final_schedule;
    }
    for bundle in schedule.as_slice()[max_bundle_number_bb1 + 1..].iter() {
        let mut pred_bundle: [PredicatedInstruction; 5] = [PredicatedInstruction::Nop; 5];
        for (slot_index, bundle_ins) in bundle.clone().iter().enumerate() {
            pred_bundle[slot_index] = PredicatedInstruction::from(bundle_ins, 0);
        }
        final_schedule.push(pred_bundle.clone());
    }

    final_schedule
}
