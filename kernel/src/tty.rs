use core::cell::UnsafeCell;

use crate::fb;
use crate::serial;

pub use crate::fb::{ConsoleGlyph, ConsoleStyle};

const OUTPUT_SERIAL: u8 = 1 << 0;
const OUTPUT_FRAMEBUFFER: u8 = 1 << 1;

pub const VIRTUAL_CONSOLE_COUNT: usize = 4;

const DEFAULT_COLUMNS: usize = 80;
const DEFAULT_ROWS: usize = 25;
const MAX_COLUMNS: usize = 128;
const MAX_ROWS: usize = 64;
const MAX_CELLS: usize = MAX_COLUMNS * MAX_ROWS;

#[derive(Copy, Clone)]
pub struct TtyBootstrap {
    pub console_id: u32,
    pub output_count: u8,
    pub framebuffer_output: bool,
    pub virtual_console_count: u32,
    pub columns: usize,
    pub rows: usize,
}

#[derive(Copy, Clone)]
pub struct TtyStats {
    pub console_id: u32,
    pub output_count: u8,
    pub bytes_written: u64,
    pub lines_written: u64,
    pub virtual_console_count: u32,
    pub columns: usize,
    pub rows: usize,
}

#[derive(Copy, Clone)]
pub enum TtyError {
    InvalidConsole,
}

impl TtyError {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::InvalidConsole => "tty console index is out of range",
        }
    }
}

struct GlobalTtyConsole(UnsafeCell<TtyConsole>);

unsafe impl Sync for GlobalTtyConsole {}

impl GlobalTtyConsole {
    const fn new() -> Self {
        Self(UnsafeCell::new(TtyConsole::new()))
    }

    fn get(&self) -> *mut TtyConsole {
        self.0.get()
    }
}

static TTY_CONSOLE: GlobalTtyConsole = GlobalTtyConsole::new();

#[derive(Copy, Clone)]
struct VirtualConsole {
    columns: usize,
    rows: usize,
    cursor_column: usize,
    cursor_row: usize,
    bytes_written: u64,
    lines_written: u64,
    cells: [ConsoleGlyph; MAX_CELLS],
}

impl VirtualConsole {
    const fn new() -> Self {
        Self {
            columns: DEFAULT_COLUMNS,
            rows: DEFAULT_ROWS,
            cursor_column: 0,
            cursor_row: 0,
            bytes_written: 0,
            lines_written: 0,
            cells: [ConsoleGlyph::empty(); MAX_CELLS],
        }
    }

    fn configure(&mut self, columns: usize, rows: usize) {
        self.columns = columns.min(MAX_COLUMNS).max(1);
        self.rows = rows.min(MAX_ROWS).max(1);
        self.clear();
        self.bytes_written = 0;
        self.lines_written = 0;
    }

    fn clear(&mut self) {
        self.cursor_column = 0;
        self.cursor_row = 0;
        self.cells.fill(ConsoleGlyph::empty());
    }

    fn write_style(&mut self, style: ConsoleStyle, text: &str) {
        self.bytes_written = self.bytes_written.saturating_add(text.len() as u64);
        self.lines_written = self
            .lines_written
            .saturating_add(text.bytes().filter(|byte| *byte == b'\n').count() as u64);

        for byte in text.bytes() {
            match byte {
                b'\r' => {}
                b'\n' => self.new_line(),
                b'\t' => {
                    for _ in 0..4 {
                        self.write_byte(style, b' ');
                    }
                }
                byte => self.write_byte(style, normalize_glyph_byte(byte)),
            }
        }
    }

    fn write_byte(&mut self, style: ConsoleStyle, byte: u8) {
        if self.cursor_column >= self.columns {
            self.new_line();
        }

        let index = self.cursor_row * self.columns + self.cursor_column;
        if index < self.cells.len() {
            self.cells[index] = ConsoleGlyph { byte, style };
        }
        self.cursor_column += 1;
    }

    fn new_line(&mut self) {
        self.cursor_column = 0;
        if self.cursor_row + 1 >= self.rows {
            self.scroll_up();
        } else {
            self.cursor_row += 1;
        }
    }

    fn scroll_up(&mut self) {
        if self.rows <= 1 {
            self.clear_row(0);
            return;
        }

        for row in 1..self.rows {
            let dst = (row - 1) * self.columns;
            let src = row * self.columns;
            for column in 0..self.columns {
                self.cells[dst + column] = self.cells[src + column];
            }
        }
        self.clear_row(self.rows - 1);
        self.cursor_row = self.rows - 1;
    }

    fn clear_row(&mut self, row: usize) {
        let start = row * self.columns;
        let end = start + self.columns;
        for cell in &mut self.cells[start..end] {
            *cell = ConsoleGlyph::empty();
        }
    }
}

struct TtyConsole {
    initialized: bool,
    active_console_id: u32,
    outputs: u8,
    bytes_written: u64,
    lines_written: u64,
    columns: usize,
    rows: usize,
    consoles: [VirtualConsole; VIRTUAL_CONSOLE_COUNT],
}

impl TtyConsole {
    const fn new() -> Self {
        Self {
            initialized: false,
            active_console_id: 0,
            outputs: OUTPUT_SERIAL,
            bytes_written: 0,
            lines_written: 0,
            columns: DEFAULT_COLUMNS,
            rows: DEFAULT_ROWS,
            consoles: [VirtualConsole::new(); VIRTUAL_CONSOLE_COUNT],
        }
    }

    fn initialize(&mut self, framebuffer_output: bool) -> TtyBootstrap {
        self.initialized = true;
        self.active_console_id = 0;
        self.outputs = OUTPUT_SERIAL;
        if framebuffer_output {
            self.outputs |= OUTPUT_FRAMEBUFFER;
        }

        let (columns, rows) = if framebuffer_output {
            fb::console_dimensions().unwrap_or((DEFAULT_COLUMNS, DEFAULT_ROWS))
        } else {
            (DEFAULT_COLUMNS, DEFAULT_ROWS)
        };
        self.columns = columns.min(MAX_COLUMNS).max(1);
        self.rows = rows.min(MAX_ROWS).max(1);
        self.bytes_written = 0;
        self.lines_written = 0;

        for console in &mut self.consoles {
            console.configure(self.columns, self.rows);
        }

        if self.outputs & OUTPUT_FRAMEBUFFER != 0 {
            self.render_active_console();
        }

        TtyBootstrap {
            console_id: self.active_console_id,
            output_count: output_count(self.outputs),
            framebuffer_output,
            virtual_console_count: VIRTUAL_CONSOLE_COUNT as u32,
            columns: self.columns,
            rows: self.rows,
        }
    }

    fn write_str(&mut self, text: &str) {
        self.write_style(ConsoleStyle::Default, text);
    }

    fn write_style(&mut self, style: ConsoleStyle, text: &str) {
        let outputs = if self.initialized { self.outputs } else { OUTPUT_SERIAL };

        if outputs & OUTPUT_SERIAL != 0 {
            serial::write_str(text);
        }

        let active = self.active_console_id as usize;
        self.consoles[active].write_style(style, text);
        if outputs & OUTPUT_FRAMEBUFFER != 0 {
            match style {
                ConsoleStyle::Default => fb::write_str(text),
                style => fb::write_style(style, text),
            }
        }

        self.bytes_written = self.bytes_written.saturating_add(text.len() as u64);
        self.lines_written = self
            .lines_written
            .saturating_add(text.bytes().filter(|byte| *byte == b'\n').count() as u64);
    }

    fn write_to_console(
        &mut self,
        console_id: u32,
        style: ConsoleStyle,
        text: &str,
    ) -> Result<(), TtyError> {
        let index = console_id as usize;
        let console = self
            .consoles
            .get_mut(index)
            .ok_or(TtyError::InvalidConsole)?;
        console.write_style(style, text);

        if index == self.active_console_id as usize && self.outputs & OUTPUT_FRAMEBUFFER != 0 {
            self.render_active_console();
        }

        Ok(())
    }

    fn switch_active_console(&mut self, console_id: u32) -> Result<(), TtyError> {
        let index = console_id as usize;
        if self.consoles.get(index).is_none() {
            return Err(TtyError::InvalidConsole);
        }

        self.active_console_id = console_id;
        if self.outputs & OUTPUT_FRAMEBUFFER != 0 {
            self.render_active_console();
        }
        Ok(())
    }

    fn render_active_console(&self) {
        let active = self.active_console_id as usize;
        let console = &self.consoles[active];
        fb::render_console(
            &console.cells[..console.columns * console.rows],
            console.columns,
            console.rows,
            console.cursor_column,
            console.cursor_row,
        );
    }

    fn stats(&self) -> TtyStats {
        let outputs = if self.initialized { self.outputs } else { OUTPUT_SERIAL };
        TtyStats {
            console_id: self.active_console_id,
            output_count: output_count(outputs),
            bytes_written: self.bytes_written,
            lines_written: self.lines_written,
            virtual_console_count: VIRTUAL_CONSOLE_COUNT as u32,
            columns: self.columns,
            rows: self.rows,
        }
    }
}

pub fn initialize(framebuffer_output: bool) -> TtyBootstrap {
    unsafe { (*TTY_CONSOLE.get()).initialize(framebuffer_output) }
}

pub fn write_str(text: &str) {
    unsafe {
        (*TTY_CONSOLE.get()).write_str(text);
    }
}

pub fn write_style(style: ConsoleStyle, text: &str) {
    unsafe {
        (*TTY_CONSOLE.get()).write_style(style, text);
    }
}

pub fn write_to_console(
    console_id: u32,
    style: ConsoleStyle,
    text: &str,
) -> Result<(), TtyError> {
    unsafe { (*TTY_CONSOLE.get()).write_to_console(console_id, style, text) }
}

pub fn switch_active_console(console_id: u32) -> Result<(), TtyError> {
    unsafe { (*TTY_CONSOLE.get()).switch_active_console(console_id) }
}

pub fn stats() -> TtyStats {
    unsafe { (*TTY_CONSOLE.get()).stats() }
}

const fn output_count(outputs: u8) -> u8 {
    (outputs & OUTPUT_SERIAL != 0) as u8 + (outputs & OUTPUT_FRAMEBUFFER != 0) as u8
}

const fn normalize_glyph_byte(byte: u8) -> u8 {
    if byte >= 0x20 && byte <= 0x7e {
        byte
    } else {
        b'?'
    }
}
