mod dependency;
mod instruction;
mod parser;
mod pip_reg_alloc;
mod pip_scheduler;
mod scheduler;

use std::env;

use std::fs::File;
use std::io::{self, BufRead, Write};

use crate::instruction::*;
use crate::parser::*;
use crate::pip_reg_alloc::*;
use crate::pip_scheduler::*;

use crate::dependency::*;
use crate::scheduler::*;

fn read_strings_from_file(inputfile: &str) -> io::Result<Vec<String>> {
    let file = File::open(inputfile)?;
    let mut strings = Vec::new();

    for line in io::BufReader::new(file).lines() {
        let line = line?;

        for capture in line.split(&['"', '[', ']'][..]).filter_map(|s| {
            if s.trim().is_empty() {
                None
            } else {
                Some(s)
            }
        }) {
            if capture.len() > 1 {
                strings.push(capture.trim().to_string());
            }
        }
    }

    Ok(strings)
}

fn write_schedule(path: String, bundles: Vec<VLIW>) -> io::Result<()> {
    let mut file = File::create(path)?;

    writeln!(file, "[")?;

    for (i, bundle) in bundles.iter().enumerate() {
        if i > 0 {
            write!(file, ",\n")?;
        }
        write!(file, "\t{}", bundle.to_string())?;
    }

    writeln!(file, "\n]")?;

    Ok(())
}

fn write_pipelined_schedule(
    path: String,
    bundles: Vec<[PredicatedInstruction; 5]>,
) -> io::Result<()> {
    let mut file = File::create(path)?;

    writeln!(file, "[")?;

    for (i, bundle) in bundles.iter().enumerate() {
        if i > 0 {
            write!(file, ",\n")?;
        }
        let bundle_strs: Vec<String> = bundle.iter().map(|item| item.to_string()).collect();
        write!(file, "\t[")?;
        for (j, item_str) in bundle_strs.iter().enumerate() {
            if j > 0 {
                write!(file, ", ")?;
            }
            write!(file, "\"{}\"", item_str)?;
        }
        write!(file, "]")?;
    }

    writeln!(file, "\n]")?;

    Ok(())
}

fn main() -> io::Result<()> {
    let args: Vec<String> = env::args().collect();
    if args.len() != 4 || (args[3] != "--pip" && args[3] != "--no_pip") {
        eprintln!("Usage: {} <input> <output> --[no_]pip", args[0]);
        return Ok(());
    }

    let mut program: Vec<Instruction> = Vec::<Instruction>::new();

    let inputfile = &args[1];
    match read_strings_from_file(inputfile) {
        Ok(strings) => {
            for (pc, string) in strings.iter().enumerate() {
                match parse_instruction(string) {
                    Ok(instr) => program.push(instr),
                    Err(e) => println!("{}: {}", pc, e),
                }
            }
        }
        Err(e) => {
            eprintln!("Error reading file: {}", e);
        }
    }

    let mut has_loop = false;
    for instr in program.iter() {
        match instr {
            Instruction::Loop(_) => has_loop = true,
            _ => {}
        }
    }

    let dependency_table = get_dependency_table(&program);

    if (args[3] == "--pip" && !has_loop) || args[3] != "--pip" {
        let bundles = get_vliw_schedule(&program, &dependency_table);
        return Ok(write_schedule(args[2].to_string(), bundles)?);
    } else {
        let (
            mut final_schedule,
            placement_map,
            max_bundle_number_bb0,
            max_bundle_number_bb1,
            max_bundle_number_bb2,
            ii,
        ) = loop_pip_schedule(program.clone());
        register_realloc(
            &program,
            placement_map,
            &mut final_schedule,
            max_bundle_number_bb0,
            max_bundle_number_bb1,
            max_bundle_number_bb2,
            ii,
        );

        let terminal_schedule = prepare_loop(
            &final_schedule,
            ii,
            max_bundle_number_bb0,
            max_bundle_number_bb1,
        );

        return Ok(write_pipelined_schedule(
            args[2].to_string(),
            terminal_schedule,
        )?);
    }
}
