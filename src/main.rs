use std::fs::File;
use std::io::Write;
use bimap::BiMap;
use clap::Parser;

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
    #[arg(short, long, action)]
    disassemble: bool,
}

fn main() {
    let j_codes = create_j_codes();
    let i_codes = create_i_codes();
    let r_codes = create_r_codes();
    let registers = create_register_codes();

    let args = Args::parse();

    if args.disassemble {
        println!("I am going to die");
        //disassemble_file();
    } else {
        let input_path = &args.input_file;
        let binary_path = input_path.to_owned() + ".bin";
        let mhc_path = input_path.to_owned() + ".mhc";
        assemble_file(&j_codes, &i_codes, &r_codes, &registers, &binary_path, input_path, &mhc_path);
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
    if opcode == LW_OPCODE || opcode == SW_OPCODE {
        let last_part_parts: Vec<&str> = parts[2]
            .split(|c| c == '(' || c == ')')
            .filter(|str| !str.is_empty())
            .collect();

        let t_register = registers.get_by_left(parts[1]).unwrap();
        let immediate = last_part_parts[0].parse::<u32>().expect("Invalid immediate value for lw/sw instruction") & 0xffff;
        let s_register = registers.get_by_left(last_part_parts[1]).unwrap();

        immediate | (s_register << 16) | (t_register << 21) | (opcode << 26)
    } else {
        let immediate = parts[3].parse::<u32>().expect("Invalid immediate value for instruction") & 0xffff;
        let t_register = registers.get_by_left(parts[1]).unwrap();
        let s_register = registers.get_by_left(parts[2]).unwrap();

        immediate | (s_register << 16) | (t_register << 21) | (opcode << 26)
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

// Not sure how to handle labels. It wasn't in the assembler Dabish gave us.
fn assemble_j(opcode: u32) -> u32 {
    opcode << 26
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

// All registers and their aliases (ex. $0 and $zero both map to 00000)
fn create_register_codes<'a>() -> BiMap<&'a str, u32> {
    BiMap::from_iter([
        ("$0", 0b00000),
        ("$zero", 0b00000),
        ("$1", 0b00001),
        ("$at", 0b00001),
        ("$2", 0b00010),
        ("$v0", 0b00010),
        ("$3", 0b00011),
        ("$v1", 0b00011),
        ("$4", 0b00100),
        ("$a0", 0b00100),
        ("$5", 0b00101),
        ("$a1", 0b00101),
        ("$6", 0b00110),
        ("$a2", 0b00110),
        ("$7", 0b00111),
        ("$a3", 0b00111),
        ("$8", 0b01000),
        ("$t0", 0b01000),
        ("$9", 0b01001),
        ("$t1", 0b01001),
        ("$10", 0b01010),
        ("$t2", 0b01010),
        ("$11", 0b01011),
        ("$t3", 0b01011),
        ("$12", 0b01100),
        ("$t4", 0b01100),
        ("$13", 0b01101),
        ("$t5", 0b01101),
        ("$14", 0b01110),
        ("$t6", 0b01110),
        ("$15", 0b01111),
        ("$t7", 0b01111),
        ("$16", 0b10000),
        ("$s0", 0b10000),
        ("$17", 0b10001),
        ("$s1", 0b10001),
        ("$18", 0b10010),
        ("$s2", 0b10010),
        ("$19", 0b10011),
        ("$s3", 0b10011),
        ("$20", 0b10100),
        ("$s4", 0b10100),
        ("$21", 0b10101),
        ("$s5", 0b10101),
        ("$22", 0b10110),
        ("$s6", 0b10110),
        ("$23", 0b10111),
        ("$s7", 0b10111),
        ("$24", 0b11000),
        ("$t8", 0b11000),
        ("$25", 0b11001),
        ("$t9", 0b11001),
        ("$26", 0b11010),
        ("$k0", 0b11010),
        ("$27", 0b11011),
        ("$k1", 0b11011),
        ("$28", 0b11100),
        ("$gp", 0b11100),
        ("$29", 0b11101),
        ("$sp", 0b11101),
        ("$30", 0b11110),
        ("$fp", 0b11110),
        ("$31", 0b11111),
        ("$ra", 0b11111),
    ])
}
