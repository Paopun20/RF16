use minifb::{Key, ScaleMode, Window, WindowOptions};
use rodio::{buffer::SamplesBuffer, OutputStream, Sink};
use std::env;
use std::f64::consts::PI;
use std::fs::File;
use std::io::{self, Read};

const WINDOW_SIZE: usize = 512;
const FB_SIZE: usize = 16;
const PIXEL_SCALE: usize = WINDOW_SIZE / FB_SIZE;

const SAMPLE_RATE: i32 = 48000;
const AMPLITUDE: f64 = 28000.0;

const MEMORY_SIZE: usize = 30_000;
const PROGRAM_CAP: usize = 16_777_216;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Palette {
    Rgb332,
    Grayscale,
}

fn pixel_color(byte: u8, palette: Palette) -> u32 {
    match palette {
        Palette::Rgb332 => {
            let r = ((byte & 0xE0) >> 5) as u32;
            let g = ((byte & 0x1C) >> 2) as u32;
            let b = (byte & 0x03) as u32;

            let r = (r * 255) / 7;
            let g = (g * 255) / 7;
            let b = (b * 255) / 3;

            (r << 16) | (g << 8) | b
        }
        Palette::Grayscale => {
            let v = byte as u32;
            (v << 16) | (v << 8) | v
        }
    }
}

fn make_note_buffer(pitch: u8) -> Vec<i16> {
    let freq = 440.0 * 2f64.powf((pitch as f64 - 69.0) / 12.0);

    let samples = SAMPLE_RATE as usize / 6;
    let attack = SAMPLE_RATE as usize / 50;
    let release_start = samples - attack;

    (0..samples)
        .map(|i| {
            let t = i as f64 / SAMPLE_RATE as f64;

            let envelope = if i < attack {
                i as f64 / attack as f64
            } else if i > release_start {
                (samples - i) as f64 / attack as f64
            } else {
                1.0
            };

            (AMPLITUDE * envelope * (2.0 * PI * freq * t).sin()) as i16
        })
        .collect()
}

fn play_note(sink: &Sink, pitch: u8) {
    let samples = make_note_buffer(pitch);

    let source = SamplesBuffer::new(1, SAMPLE_RATE as u32, samples);

    sink.append(source);
}

struct Interpreter {
    program: Vec<u16>,
    memory: [u8; MEMORY_SIZE],
    cursor: usize,
    address: usize,
}

impl Interpreter {
    fn new() -> Self {
        Self {
            program: Vec::with_capacity(PROGRAM_CAP),
            memory: [0u8; MEMORY_SIZE],
            cursor: 0,
            address: 0,
        }
    }

    fn program_size(&self) -> usize {
        self.program.len()
    }

    fn push(&mut self, opcode: u16, operand: u16) {
        self.program.push(opcode);
        self.program.push(operand);
    }

    fn run_frame(&mut self, window: &Window) -> bool {
        while self.cursor < self.program_size() {
            let opcode = self.program[self.cursor] as u8 as char;
            self.cursor += 1;

            match opcode {
                '>' => {
                    let n = self.program[self.cursor] as usize;
                    self.cursor += 1;

                    self.address = (self.address + n) % MEMORY_SIZE;
                }

                '<' => {
                    let n = self.program[self.cursor] as usize;
                    self.cursor += 1;

                    self.address = (self.address + MEMORY_SIZE - (n % MEMORY_SIZE)) % MEMORY_SIZE;
                }

                '+' => {
                    let n = self.program[self.cursor] as u8;
                    self.cursor += 1;

                    self.memory[self.address] = self.memory[self.address].wrapping_add(n);
                }

                '-' => {
                    let n = self.program[self.cursor] as u8;
                    self.cursor += 1;

                    self.memory[self.address] = self.memory[self.address].wrapping_sub(n);
                }

                '[' => {
                    if self.memory[self.address] == 0 {
                        self.cursor += self.program[self.cursor] as usize;
                    }

                    self.cursor += 1;
                }

                ']' => {
                    if self.memory[self.address] != 0 {
                        self.cursor -= self.program[self.cursor] as usize;
                    }

                    self.cursor += 1;
                }

                '.' => {
                    self.cursor += 1;
                    return true;
                }

                ',' => {
                    self.cursor += 1;

                    let mut key: u8 = 0;

                    if window.is_key_down(Key::Z) {
                        key |= 0x80;
                    }

                    if window.is_key_down(Key::X) {
                        key |= 0x40;
                    }

                    if window.is_key_down(Key::Enter) {
                        key |= 0x20;
                    }

                    if window.is_key_down(Key::Space) {
                        key |= 0x10;
                    }

                    if window.is_key_down(Key::Up) {
                        key |= 0x08;
                    }

                    if window.is_key_down(Key::Down) {
                        key |= 0x04;
                    }

                    if window.is_key_down(Key::Left) {
                        key |= 0x02;
                    }

                    if window.is_key_down(Key::Right) {
                        key |= 0x01;
                    }

                    self.memory[self.address] = key;
                }

                '?' => {
                    self.cursor += 1;

                    println!("memory[{}]: {}", self.address, self.memory[self.address]);
                }

                _ => {
                    eprintln!("unexpected char '{}'", opcode);
                    self.cursor += 1;
                }
            }
        }

        false
    }
}

fn is_bf_char(c: char) -> bool {
    matches!(c, '>' | '<' | '+' | '-' | '[' | ']' | '.' | ',' | '?')
}

fn load_program(path: &str) -> io::Result<Interpreter> {
    let mut src = File::open(path)?;

    let mut interp = Interpreter::new();

    let mut raw = Vec::new();
    src.read_to_end(&mut raw)?;

    let chars: Vec<char> = raw.iter().map(|&b| b as char).collect();

    let mut i = 0;

    while i < chars.len() {
        let ch = raw[i] as char;
        i += 1;

        match ch {
            '.' | ',' | '?' | '[' => {
                interp.push(ch as u16, 0);
            }

            '>' | '<' | '+' | '-' => {
                interp.program.push(ch as u16);

                let operand_idx = interp.program.len();
                interp.program.push(1u16);

                loop {
                    while i < chars.len() && !is_bf_char(chars[i]) {
                        i += 1;
                    }

                    if i >= chars.len() || chars[i] != ch {
                        break;
                    }

                    interp.program[operand_idx] = interp.program[operand_idx].saturating_add(1);

                    i += 1;
                }
            }

            ']' => {
                let close_pos = interp.program.len();

                interp.program.push(']' as u16);
                interp.program.push(0u16);

                let mut depth: usize = 1;
                let mut j = close_pos;

                while j > 0 && depth > 0 {
                    j -= 2;

                    match interp.program[j] as u8 as char {
                        '[' => depth -= 1,
                        ']' => depth += 1,
                        _ => {}
                    }
                }

                if depth > 0 {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "unmatched closing bracket",
                    ));
                }

                let dist = (close_pos - j) as u16;

                interp.program[j + 1] = dist;
                interp.program[close_pos + 1] = dist;
            }

            _ => {}
        }
    }

    Ok(interp)
}

fn render_framebuffer(interp: &Interpreter, framebuffer: &mut [u32], palette: Palette) {
    for y in 0..FB_SIZE {
        for x in 0..FB_SIZE {
            let idx = y * FB_SIZE + x;

            let color = pixel_color(interp.memory[idx], palette);

            for sy in 0..PIXEL_SCALE {
                for sx in 0..PIXEL_SCALE {
                    let px = x * PIXEL_SCALE + sx;
                    let py = y * PIXEL_SCALE + sy;

                    framebuffer[py * WINDOW_SIZE + px] = color;
                }
            }
        }
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 || args.len() > 3 {
        eprintln!("Usage: {} <filename> [--grayscale]", args[0]);
        std::process::exit(1);
    }

    let palette = if args.iter().any(|a| a == "--grayscale") {
        Palette::Grayscale
    } else {
        Palette::Rgb332
    };

    let mut interp = load_program(&args[1])?;

    let mut window = Window::new(
        "BF16",
        WINDOW_SIZE,
        WINDOW_SIZE,
        WindowOptions {
            scale_mode: ScaleMode::AspectRatioStretch,
            resize: true,
            ..WindowOptions::default()
        },
    )?;

    window.set_target_fps(60);

    let mut framebuffer = vec![0u32; WINDOW_SIZE * WINDOW_SIZE];

    let (_stream, stream_handle) = OutputStream::try_default()?;
    let sink = Sink::try_new(&stream_handle)?;

    let mut current_note: u8 = 0;

    while window.is_open() && !window.is_key_down(Key::Escape) {
        render_framebuffer(&interp, &mut framebuffer, palette);

        if !interp.run_frame(&window) {
            break;
        }

        let note = interp.memory[interp.address];

        if note != current_note {
            current_note = note;

            if current_note != 0 {
                play_note(&sink, current_note);
            }
        }

        window.update_with_buffer(&framebuffer, WINDOW_SIZE, WINDOW_SIZE)?;
    }

    Ok(())
}
