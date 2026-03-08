use alloc::string::String;
use alloc::vec::Vec;

const ELF_MAGIC: [u8; 4] = [0x7f, b'E', b'L', b'F'];
const ELF64_EHDR_SIZE: usize = 64;
const ELF64_PHDR_SIZE: usize = 56;
const ELF_CLASS_64: u8 = 2;
const ELF_DATA_LITTLE: u8 = 1;
const ELF_VERSION_CURRENT: u8 = 1;
const MAX_PROGRAM_HEADERS: usize = 256;

#[derive(Copy, Clone, Eq, PartialEq)]
pub enum ImageKind {
    Elf64,
    ShebangScript,
    Text,
    Unknown,
}

#[derive(Copy, Clone, Eq, PartialEq)]
pub enum ProgramHeaderType {
    Null,
    Load,
    Dynamic,
    Interpreter,
    Note,
    ProgramHeaderTable,
    ThreadLocalStorage,
    GnuStack,
    GnuRelro,
    Unknown(u32),
}

impl ProgramHeaderType {
    fn from_raw(raw: u32) -> Self {
        match raw {
            0 => Self::Null,
            1 => Self::Load,
            2 => Self::Dynamic,
            3 => Self::Interpreter,
            4 => Self::Note,
            6 => Self::ProgramHeaderTable,
            7 => Self::ThreadLocalStorage,
            0x6474_e551 => Self::GnuStack,
            0x6474_e552 => Self::GnuRelro,
            other => Self::Unknown(other),
        }
    }
}

#[derive(Copy, Clone)]
pub struct ProgramHeader {
    pub segment_type: ProgramHeaderType,
    pub flags: u32,
    pub offset: u64,
    pub virtual_address: u64,
    pub file_size: u64,
    pub memory_size: u64,
    pub alignment: u64,
}

impl ProgramHeader {
    pub fn is_loadable(self) -> bool {
        self.segment_type == ProgramHeaderType::Load
    }
}

pub struct ElfImage {
    pub image_type: u16,
    pub machine: u16,
    pub entry_point: u64,
    pub interpreter: Option<String>,
    pub program_headers: Vec<ProgramHeader>,
}

pub struct ShebangImage {
    pub interpreter: String,
    pub argument: Option<String>,
}

pub enum ExecutableImage {
    Elf64(ElfImage),
    Shebang(ShebangImage),
    Text,
    Unknown,
}

#[derive(Copy, Clone)]
pub enum ParseError {
    Truncated,
    UnsupportedClass,
    UnsupportedEndianness,
    UnsupportedVersion,
    InvalidHeader,
    TooManyProgramHeaders,
    InvalidProgramHeaderTable,
    ProgramHeaderOutOfBounds,
    SegmentOutOfBounds,
}

impl ParseError {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Truncated => "executable image is truncated",
            Self::UnsupportedClass => "elf class is unsupported",
            Self::UnsupportedEndianness => "elf endianness is unsupported",
            Self::UnsupportedVersion => "elf version is unsupported",
            Self::InvalidHeader => "elf header is invalid",
            Self::TooManyProgramHeaders => "elf has too many program headers",
            Self::InvalidProgramHeaderTable => "elf program header table is invalid",
            Self::ProgramHeaderOutOfBounds => "elf program header extends beyond image bounds",
            Self::SegmentOutOfBounds => "elf segment extends beyond image bounds",
        }
    }
}

pub fn inspect(image: &[u8]) -> Result<ExecutableImage, ParseError> {
    if image.starts_with(&ELF_MAGIC) {
        let elf = parse_elf64(image)?;
        return Ok(ExecutableImage::Elf64(elf));
    }

    if let Some(shebang) = parse_shebang(image) {
        return Ok(ExecutableImage::Shebang(shebang));
    }

    if looks_like_text(image) {
        return Ok(ExecutableImage::Text);
    }

    Ok(ExecutableImage::Unknown)
}

pub fn detect_kind(image: &[u8]) -> Result<ImageKind, ParseError> {
    Ok(match inspect(image)? {
        ExecutableImage::Elf64(_) => ImageKind::Elf64,
        ExecutableImage::Shebang(_) => ImageKind::ShebangScript,
        ExecutableImage::Text => ImageKind::Text,
        ExecutableImage::Unknown => ImageKind::Unknown,
    })
}

fn parse_elf64(image: &[u8]) -> Result<ElfImage, ParseError> {
    if image.len() < ELF64_EHDR_SIZE {
        return Err(ParseError::Truncated);
    }
    if image[4] != ELF_CLASS_64 {
        return Err(ParseError::UnsupportedClass);
    }
    if image[5] != ELF_DATA_LITTLE {
        return Err(ParseError::UnsupportedEndianness);
    }
    if image[6] != ELF_VERSION_CURRENT {
        return Err(ParseError::UnsupportedVersion);
    }

    let header_size = read_u16_le(image, 52)?;
    if header_size as usize != ELF64_EHDR_SIZE {
        return Err(ParseError::InvalidHeader);
    }

    let image_type = read_u16_le(image, 16)?;
    let machine = read_u16_le(image, 18)?;
    let version = read_u32_le(image, 20)?;
    if version != ELF_VERSION_CURRENT as u32 {
        return Err(ParseError::UnsupportedVersion);
    }

    let entry_point = read_u64_le(image, 24)?;
    let program_header_offset = read_u64_le(image, 32)? as usize;
    let program_header_entry_size = read_u16_le(image, 54)? as usize;
    let program_header_count = read_u16_le(image, 56)? as usize;

    if program_header_count > MAX_PROGRAM_HEADERS {
        return Err(ParseError::TooManyProgramHeaders);
    }
    if program_header_count > 0 {
        if program_header_entry_size < ELF64_PHDR_SIZE {
            return Err(ParseError::InvalidProgramHeaderTable);
        }
        let table_size = program_header_entry_size
            .checked_mul(program_header_count)
            .ok_or(ParseError::InvalidProgramHeaderTable)?;
        let table_end = program_header_offset
            .checked_add(table_size)
            .ok_or(ParseError::InvalidProgramHeaderTable)?;
        if table_end > image.len() {
            return Err(ParseError::ProgramHeaderOutOfBounds);
        }
    }

    let mut program_headers = Vec::with_capacity(program_header_count);
    let mut interpreter = None;
    for index in 0..program_header_count {
        let entry_offset = program_header_offset
            .checked_add(
                index
                    .checked_mul(program_header_entry_size)
                    .ok_or(ParseError::InvalidProgramHeaderTable)?,
            )
            .ok_or(ParseError::InvalidProgramHeaderTable)?;
        let entry_end = entry_offset
            .checked_add(ELF64_PHDR_SIZE)
            .ok_or(ParseError::InvalidProgramHeaderTable)?;
        if entry_end > image.len() {
            return Err(ParseError::ProgramHeaderOutOfBounds);
        }

        let segment_type = ProgramHeaderType::from_raw(read_u32_le(image, entry_offset)?);
        let flags = read_u32_le(image, entry_offset + 4)?;
        let offset = read_u64_le(image, entry_offset + 8)?;
        let virtual_address = read_u64_le(image, entry_offset + 16)?;
        let file_size = read_u64_le(image, entry_offset + 32)?;
        let memory_size = read_u64_le(image, entry_offset + 40)?;
        let alignment = read_u64_le(image, entry_offset + 48)?;

        if file_size > 0 {
            let segment_start = offset as usize;
            let segment_end = segment_start
                .checked_add(file_size as usize)
                .ok_or(ParseError::SegmentOutOfBounds)?;
            if segment_end > image.len() {
                return Err(ParseError::SegmentOutOfBounds);
            }
        }

        let header = ProgramHeader {
            segment_type,
            flags,
            offset,
            virtual_address,
            file_size,
            memory_size,
            alignment,
        };
        if interpreter.is_none() && header.segment_type == ProgramHeaderType::Interpreter && header.file_size > 0 {
            let segment_start = header.offset as usize;
            let segment_end = segment_start + header.file_size as usize;
            let segment = &image[segment_start..segment_end];
            let length = segment
                .iter()
                .position(|byte| *byte == 0)
                .unwrap_or(segment.len());
            if length > 0 {
                interpreter = Some(String::from_utf8_lossy(&segment[..length]).into_owned());
            }
        }

        program_headers.push(header);
    }

    Ok(ElfImage {
        image_type,
        machine,
        entry_point,
        interpreter,
        program_headers,
    })
}

fn parse_shebang(image: &[u8]) -> Option<ShebangImage> {
    if !image.starts_with(b"#!") {
        return None;
    }

    let line_end = image
        .iter()
        .position(|byte| *byte == b'\n')
        .unwrap_or(image.len());
    let line = &image[2..line_end];

    let mut tokens = line
        .split(|byte| byte.is_ascii_whitespace())
        .filter(|token| !token.is_empty());
    let interpreter = String::from_utf8_lossy(tokens.next()?).into_owned();
    let argument = tokens
        .next()
        .map(|value| String::from_utf8_lossy(value).into_owned());

    Some(ShebangImage {
        interpreter,
        argument,
    })
}

fn looks_like_text(image: &[u8]) -> bool {
    if image.is_empty() {
        return false;
    }

    image.iter().all(|byte| {
        matches!(
            byte,
            b'\n' | b'\r' | b'\t' | b' '..=b'~'
        )
    })
}

fn read_u16_le(bytes: &[u8], offset: usize) -> Result<u16, ParseError> {
    let end = offset.checked_add(2).ok_or(ParseError::Truncated)?;
    let slice = bytes.get(offset..end).ok_or(ParseError::Truncated)?;
    Ok(u16::from_le_bytes([slice[0], slice[1]]))
}

fn read_u32_le(bytes: &[u8], offset: usize) -> Result<u32, ParseError> {
    let end = offset.checked_add(4).ok_or(ParseError::Truncated)?;
    let slice = bytes.get(offset..end).ok_or(ParseError::Truncated)?;
    Ok(u32::from_le_bytes([slice[0], slice[1], slice[2], slice[3]]))
}

fn read_u64_le(bytes: &[u8], offset: usize) -> Result<u64, ParseError> {
    let end = offset.checked_add(8).ok_or(ParseError::Truncated)?;
    let slice = bytes.get(offset..end).ok_or(ParseError::Truncated)?;
    Ok(u64::from_le_bytes([
        slice[0], slice[1], slice[2], slice[3], slice[4], slice[5], slice[6], slice[7],
    ]))
}
