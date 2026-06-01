use std::collections::HashMap;

pub const RAM_SIZE: usize = 65536;

#[derive(Debug, Clone, Copy)]
pub enum Op {
    Add(i16),
    Move(i16),

    Set(u8),

    Input,
    Output,

    JumpIfZero(usize),
    JumpIfNonZero(usize),

    Clear,

    Halt,
}

pub struct VM {
    pub ram: [u8; RAM_SIZE],
    pub ptr: usize,
    pub pc: usize,

    pub running: bool,

    pub program: Vec<Op>,

    pub input_buffer: Vec<u8>,
    pub output_buffer: Vec<u8>,
}

impl VM {
    pub fn new(program: Vec<Op>) -> Self {
        Self {
            ram: [0; RAM_SIZE],
            ptr: 0,
            pc: 0,

            running: true,

            program,

            input_buffer: Vec::new(),
            output_buffer: Vec::new(),
        }
    }

    #[inline(always)]
    pub fn current_cell(&mut self) -> &mut u8 {
        &mut self.ram[self.ptr]
    }

    pub fn reset(&mut self) {
        self.ram = [0; RAM_SIZE];
        self.ptr = 0;
        self.pc = 0;
        self.running = true;
        self.output_buffer.clear();
    }

    pub fn step(&mut self) {
        if !self.running {
            return;
        }

        if self.pc >= self.program.len() {
            self.running = false;
            return;
        }

        let op = self.program[self.pc];

        match op {
            Op::Add(v) => {
                let cell = self.current_cell();
                *cell = cell.wrapping_add(v as u8);
            }

            Op::Move(v) => {
                self.ptr = ((self.ptr as isize + v as isize)
                    .rem_euclid(RAM_SIZE as isize)) as usize;
            }

            Op::Set(v) => {
                *self.current_cell() = v;
            }

            Op::Input => {
                let value = if self.input_buffer.is_empty() {
                    0
                } else {
                    self.input_buffer.remove(0)
                };

                *self.current_cell() = value;
            }

            Op::Output => {
                self.output_buffer.push(*self.current_cell());
            }

            Op::JumpIfZero(target) => {
                if *self.current_cell() == 0 {
                    self.pc = target;
                    return;
                }
            }

            Op::JumpIfNonZero(target) => {
                if *self.current_cell() != 0 {
                    self.pc = target;
                    return;
                }
            }

            Op::Clear => {
                *self.current_cell() = 0;
            }

            Op::Halt => {
                self.running = false;
            }
        }

        self.pc += 1;
    }

    pub fn run(&mut self) {
        while self.running {
            self.step();
        }
    }
}

pub fn compile_bf(source: &str) -> Vec<Op> {
    let chars: Vec<char> = source.chars().collect();

    let mut program = Vec::new();
    let mut loop_stack = Vec::new();
    let mut jump_map: HashMap<usize, usize> = HashMap::new();

    let mut i = 0;

    while i < chars.len() {
        match chars[i] {
            '+' | '-' => {
                let mut value = 0i16;

                while i < chars.len() {
                    match chars[i] {
                        '+' => value += 1,
                        '-' => value -= 1,
                        _ => break,
                    }

                    i += 1;
                }

                if value != 0 {
                    program.push(Op::Add(value));
                }

                continue;
            }

            '>' | '<' => {
                let mut value = 0i16;

                while i < chars.len() {
                    match chars[i] {
                        '>' => value += 1,
                        '<' => value -= 1,
                        _ => break,
                    }

                    i += 1;
                }

                if value != 0 {
                    program.push(Op::Move(value));
                }

                continue;
            }

            '.' => {
                program.push(Op::Output);
            }

            ',' => {
                program.push(Op::Input);
            }

            '[' => {
                let pos = program.len();

                loop_stack.push(pos);

                program.push(Op::JumpIfZero(0));
            }

            ']' => {
                let start = loop_stack
                    .pop()
                    .expect("Unmatched closing bracket");

                let end = program.len();

                jump_map.insert(start, end);
                jump_map.insert(end, start);

                program.push(Op::JumpIfNonZero(start));
            }

            _ => {}
        }

        i += 1;
    }

    for (from, to) in jump_map {
        match &mut program[from] {
            Op::JumpIfZero(target) => *target = to,
            Op::JumpIfNonZero(target) => *target = to,
            _ => {}
        }
    }

    program.push(Op::Halt);

    program
}
