// Git repository available on GitHub at https://github.com/thedarkcolour/mips-assembler

use std::ascii::AsciiExt;
use std::fs::File;
use std::io::{Read, Write};
use bimap::BiMap;
use clap::{Parser, ValueEnum};

// Offset I type instructions
const LW_OPCODE: u32 = 0b100011;
const SW_OPCODE: u32 = 0b101011;

// shamt R type instructions
const SLL_OPCODE: u32 = 0b000000;
const SLLV_OPCODE: u32 = 0b000100;
const SRL_OPCODE: u32 = 0b000010;
const SRLV_OPCODE: u32 = 0b000110;
const SRA_OPCODE: u32 = 0b000011;
const SRAV_OPCODE: u32 = 0b000111;

#[derive(Parser)]
struct Args {
    #[arg(short, long)]
    input_file: String,
    #[arg(short, long)]
    mode: Option<AssemblerMode>,
}

#[derive(Eq, PartialEq, Clone, ValueEnum)]
enum AssemblerMode {
    // Assemble .bin and .mhc files from Binary
    Assemble,
    // Disassemble .bin files
    Bin,
    // Disassemble .mhc files
    Mhc,
}

fn main() {
    let j_codes = create_j_codes();
    let i_codes = create_i_codes();
    let r_codes = create_r_codes();
    let registers = create_register_codes();

    let args = Args::parse();
    let input_path = &args.input_file;
    let mode = args.mode.unwrap_or(AssemblerMode::Assemble);

    if mode == AssemblerMode::Assemble {
        let binary_path = input_path.to_owned() + ".bin";
        let mhc_path = input_path.to_owned() + ".mhc";

        assemble_file(&j_codes, &i_codes, &r_codes, &registers, &binary_path, input_path, &mhc_path);
    } else {
        let mut input_file = File::create(input_path).expect("No such file");
        let mut instructions: Vec<u32> = Vec::new();
        let mut bytes: Vec<u8>;

        // Different reading modes
        if mode == AssemblerMode::Bin {
            let mut input_file = std::io::BufReader::new(input_file);
            let mut s = String::new();

            bytes = input_file.read_to_string(&mut s)
                .expect("Failed to read")
                .to_le_bytes()
                .to_vec();
        } else {
            // copied from std::fs::read
            let size = input_file.metadata().map(|m| m.len() as usize).ok();
            bytes = Vec::with_capacity(size.unwrap_or(0));
            input_file.read_to_end(&mut bytes).unwrap();
        }

        instructions.reserve(bytes.len() / 4);
        for chunk in bytes.chunks(4) {
            // Rust wants things in sized slices apparently
            let mut chunk_4 = [0u8; 4];
            chunk_4.copy_from_slice(chunk);
            instructions.push(u32::from_le_bytes(chunk_4));
        }

        for instruction in instructions {
            let opcode = instruction >> 26;

            let result = if opcode == 0 {
                disassemble_r(instruction, &registers, *r_codes.get_by_right(&(instruction & 0b111111)).unwrap())
            } else {
                if let Some(j_instruction) = j_codes.get_by_right(&opcode) {
                    disassemble_j(instruction, j_instruction)
                } else if let Some(i_instruction) = i_codes.get_by_right(&opcode) {
                    disassemble_i(instruction, &registers, *i_instruction)
                } else {
                    panic!("Invalid opcode");
                }
            };

            println!("{}", result);
        }
    }
}

fn assemble_file(j_codes: &BiMap<&str, u32>, i_codes: &BiMap<&str, u32>, r_codes: &BiMap<&str, u32>, registers: &BiMap<&str, u32>, binary_path: &str, asm_path: &str, mhc_path: &str) {
    let result = std::fs::read_to_string(asm_path).expect("No such file");
    let binary_file = File::create(binary_path).expect("Failed to create binary file");
    let mhc_file = File::create(mhc_path).expect("MHC ");
    // Buffered writers flush when they go out of scope
    let mut binary_file = std::io::BufWriter::new(binary_file);
    let mut mhc_file = std::io::BufWriter::new(mhc_file);

    for asm_line in result.lines() {
        // Actual machine code
        let mhc_line = assemble_line(j_codes, i_codes, r_codes, registers, asm_line);
        println!("{:032b}", mhc_line);
        mhc_file.write_all(&mhc_line.to_le_bytes()).expect("Failed to write line");
        // Human-readable 0s and 1s (characters)
        let bin_line = format!("{:032b}", mhc_line);
        binary_file.write(bin_line.as_bytes()).expect("Failed to write line");
    }
}

fn assemble_line(j_codes: &BiMap<&str, u32>, i_codes: &BiMap<&str, u32>, r_codes: &BiMap<&str, u32>, registers: &BiMap<&str, u32>, asm_line: &str) -> u32 {
    let asm_line = if let Some(split) = asm_line.split_once("#") {
        split.0
    } else {
        asm_line
    }.trim();

    // split on spaces or commas, filter empty substrings, put into array
    let parts: Vec<&str> = asm_line
        .split(|c| c == ',' || c == ' ')
        .filter(|str| !str.is_empty())
        .collect();
    let instruction = parts[0].to_ascii_lowercase();
    let instruction = instruction.as_str();
    println!("{:?}", parts);

    if let Some(i_opcode) = i_codes.get_by_left(instruction) {
        assemble_i(*i_opcode, registers, parts)
    } else if let Some(r_opcode) = r_codes.get_by_left(instruction) {
        assemble_r(*r_opcode, registers, parts)
    } else if let Some(j_opcode) = j_codes.get_by_left(instruction) {
        assemble_j(*j_opcode)
    } else {
        println!("Failed to parse line {:?}", parts);
        0
    }
}

fn assemble_i(opcode: u32, registers: &BiMap<&str, u32>, parts: Vec<&str>) -> u32 {
    let immediate: u32;
    let t_register: &u32;
    let s_register: &u32;

    if opcode == LW_OPCODE || opcode == SW_OPCODE {
        let last_part_parts: Vec<&str> = parts[2]
            .split(|c| c == '(' || c == ')')
            .filter(|str| !str.is_empty())
            .collect();

        t_register = registers.get_by_left(parts[1]).unwrap();
        immediate = last_part_parts[0].parse::<u32>().expect("Invalid immediate value for lw/sw instruction") & 0xffff;
        s_register = registers.get_by_left(last_part_parts[1]).unwrap();
    } else {
        immediate = parts[3].parse::<u32>().expect("Invalid immediate value for instruction") & 0xffff;
        t_register = registers.get_by_left(parts[1]).unwrap();
        s_register = registers.get_by_left(parts[2]).unwrap();
    }

    immediate | (s_register << 16) | (t_register << 21) | (opcode << 26)
}

fn disassemble_i(instruction: u32, registers: &BiMap<&str, u32>, instruction_name: &str) -> String {
    let t_register = registers.get_by_right(&((instruction >> 21) & 0b11111)).unwrap();
    let s_register = registers.get_by_right(&((instruction >> 16) & 0b11111)).unwrap();
    let immediate = instruction & 0xffff;

    if instruction_name.eq("lw") || instruction_name.eq("sw") {
        format!("{} {}, {}({})\n", instruction_name, t_register, immediate, s_register)
    } else {
        format!("{} {}, {}, {}\n", instruction_name, t_register, s_register, immediate)
    }
}

fn assemble_r(func_code: u32, registers: &BiMap<&str, u32>, parts: Vec<&str>) -> u32 {
    let shift_opcode = func_code == SLL_OPCODE || func_code == SRL_OPCODE || func_code == SRA_OPCODE;
    let shift_amount = if shift_opcode {
        parts[3].parse::<u32>().expect(&format!("Invalid shift amount: {}", parts[3]))
    } else {
        0
    };
    let d_register = registers.get_by_left(parts[1]).unwrap();
    let t_register = registers.get_by_left(parts[2]).unwrap();
    let s_register = if shift_opcode { &0 } else { registers.get_by_left(parts[3]).unwrap() };

    // no need to specify opcode as it is always zero for R type instructions
    func_code | (shift_amount << 6) | (d_register << 11) | (t_register << 16) | (s_register << 21)
}

fn disassemble_r(instruction: u32, registers: &BiMap<&str, u32>, instruction_name: &str) -> String {
    let d_register = registers.get_by_right(&((instruction >> 11) & 0b11111)).unwrap();
    let t_register = registers.get_by_right(&((instruction >> 16) & 0b11111)).unwrap();
    let s_register = registers.get_by_right(&((instruction >> 21) & 0b11111)).unwrap();

    format!("{} {}, {}, {}", instruction_name, d_register, s_register, t_register)
}

// Not sure how to handle labels. It wasn't in the assembler Dabish gave us.
fn assemble_j(opcode: u32) -> u32 {
    opcode << 26
}

fn disassemble_j(instruction: u32, instruction_name: &&str) -> String {
    // Sorry. Dabish didn't include labels in his assembler, so we had nothing to go off of
    format!("{} unimplemented", instruction_name)
}

// https://www.d.umn.edu/~gshute/mips/jtype.html
fn create_j_codes<'a>() -> BiMap<&'a str, u32> {
    BiMap::from_iter([
        ("j", 0b000010),
        ("jal", 0b000011)
    ])
}

// https://www.d.umn.edu/~gshute/mips/itype.html
fn create_i_codes<'a>() -> BiMap<&'a str, u32> {
    BiMap::from_iter([
        ("addi", 0b001000),
        ("addiu", 0b001001),
        ("andi", 0b001100),
        ("beq", 0b000100),
        ("bne", 0b000101),
        ("lw", LW_OPCODE),
        ("ori", 0b001101),
        ("sw", SW_OPCODE),
    ])
}

// Func codes. Opcode of R-type is always zero
// https://www.d.umn.edu/~gshute/mips/rtype.html
fn create_r_codes<'a>() -> BiMap<&'a str, u32> {
    BiMap::from_iter([
        ("add", 0b100000),
        ("addu", 0b100001),
        ("and", 0b100100),
        ("div", 0b011010),
        ("jr", 0b001000),
        ("nor", 0b100111),
        ("or", 0b100101),
        ("sll", SLL_OPCODE),
        ("sllv", SLLV_OPCODE),
        ("slt", 0b101010),
        ("sltu", 0b101011),
        ("sra", SRA_OPCODE),
        ("srav", SRAV_OPCODE),
        ("srl", SRL_OPCODE),
        ("srlv", SRLV_OPCODE),
        ("sub", 0b100010),
        ("subu", 0b100011),
        ("xor", 0b100110)
    ])
}

// All registers (couldn't do aliases with BiMap, but if i used two maps it would complicate the code)
fn create_register_codes<'a>() -> BiMap<&'a str, u32> {
    BiMap::from_iter([
        ("$zero", 0b00000),
        ("$at", 0b00001),
        ("$v0", 0b00010),
        ("$v1", 0b00011),
        ("$a0", 0b00100),
        ("$a1", 0b00101),
        ("$a2", 0b00110),
        ("$a3", 0b00111),
        ("$t0", 0b01000),
        ("$t1", 0b01001),
        ("$t2", 0b01010),
        ("$t3", 0b01011),
        ("$t4", 0b01100),
        ("$t5", 0b01101),
        ("$t6", 0b01110),
        ("$t7", 0b01111),
        ("$s0", 0b10000),
        ("$s1", 0b10001),
        ("$s2", 0b10010),
        ("$s3", 0b10011),
        ("$s4", 0b10100),
        ("$s5", 0b10101),
        ("$s6", 0b10110),
        ("$s7", 0b10111),
        ("$t8", 0b11000),
        ("$t9", 0b11001),
        ("$k0", 0b11010),
        ("$k1", 0b11011),
        ("$gp", 0b11100),
        ("$sp", 0b11101),
        ("$fp", 0b11110),
        ("$ra", 0b11111),
    ])
}
