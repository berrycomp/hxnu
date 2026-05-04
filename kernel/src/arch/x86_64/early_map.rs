use core::arch::asm;
use core::ptr::{read_volatile, write_volatile};

use crate::mm;

pub const FLAG_WRITE_THROUGH: u64 = 1 << 3;
pub const FLAG_CACHE_DISABLE: u64 = 1 << 4;

const PAGE_PRESENT: u64 = 1 << 0;
const PAGE_WRITABLE: u64 = 1 << 1;
pub const PAGE_USER: u64 = 1 << 2;
const PAGE_HUGE: u64 = 1 << 7;
const PAGE_ADDRESS_MASK: u64 = 0x000f_ffff_ffff_f000;
const PAGE_SIZE: u64 = 4096;

#[derive(Copy, Clone, Debug)]
pub enum MapError {
    AddressOverflow,
    PageTableAllocationFailed,
    MappingConflict,
}

pub fn ensure_region_mapped(
    hhdm_offset: u64,
    physical_address: u64,
    length: usize,
    extra_flags: u64,
) -> Result<u64, MapError> {
    let virtual_address = hhdm_offset
        .checked_add(physical_address)
        .ok_or(MapError::AddressOverflow)?;
    let last_physical_address = physical_address
        .checked_add(length.max(1) as u64 - 1)
        .ok_or(MapError::AddressOverflow)?;

    let start_page = physical_address & !0xfff;
    let end_page = last_physical_address & !0xfff;

    let mut page = start_page;
    loop {
        ensure_page_mapping(hhdm_offset, virtual_address_for_hhdm(hhdm_offset, page)?, page, extra_flags)?;
        if page == end_page {
            break;
        }
        page = page.checked_add(PAGE_SIZE).ok_or(MapError::AddressOverflow)?;
    }

    Ok(virtual_address)
}

pub fn map_virtual_page(
    hhdm_offset: u64,
    virtual_address: u64,
    physical_address: u64,
    extra_flags: u64,
) -> Result<(), MapError> {
    ensure_page_mapping(
        hhdm_offset,
        virtual_address & !0xfff,
        physical_address & !0xfff,
        extra_flags,
    )
}

pub fn unmap_virtual_page(hhdm_offset: u64, virtual_address: u64) -> Result<Option<u64>, MapError> {
    let page_address = virtual_address & !0xfff;
    let pml4 = hhdm_offset
        .checked_add(read_cr3() & PAGE_ADDRESS_MASK)
        .ok_or(MapError::AddressOverflow)? as *mut u64;
    let pml4_index = page_table_index(page_address, 39);
    let pdpt_index = page_table_index(page_address, 30);
    let pd_index = page_table_index(page_address, 21);
    let pt_index = page_table_index(page_address, 12);

    let Some(pdpt) = existing_next_table(pml4, pml4_index, hhdm_offset)? else {
        return Ok(None);
    };
    let Some(pd) = existing_next_table(pdpt, pdpt_index, hhdm_offset)? else {
        return Ok(None);
    };
    let Some(pt) = existing_next_table(pd, pd_index, hhdm_offset)? else {
        return Ok(None);
    };

    let pte = unsafe { pt.add(pt_index) };
    let entry = unsafe { read_volatile(pte) };
    if entry & PAGE_PRESENT == 0 {
        return Ok(None);
    }

    unsafe {
        write_volatile(pte, 0);
    }
    invalidate_page(page_address);
    Ok(Some(entry & PAGE_ADDRESS_MASK))
}

fn ensure_page_mapping(
    hhdm_offset: u64,
    virtual_address: u64,
    physical_address: u64,
    extra_flags: u64,
) -> Result<(), MapError> {
    let pml4 = hhdm_offset
        .checked_add(read_cr3() & PAGE_ADDRESS_MASK)
        .ok_or(MapError::AddressOverflow)? as *mut u64;
    let pml4_index = page_table_index(virtual_address, 39);
    let pdpt_index = page_table_index(virtual_address, 30);
    let pd_index = page_table_index(virtual_address, 21);
    let pt_index = page_table_index(virtual_address, 12);

    let pdpt = match next_table(pml4, pml4_index, hhdm_offset)? {
        NextTable::Table(table) => table,
        NextTable::HugePage => return Ok(()),
    };
    let pd = match next_table(pdpt, pdpt_index, hhdm_offset)? {
        NextTable::Table(table) => table,
        NextTable::HugePage => return Ok(()),
    };
    let pt = match next_table(pd, pd_index, hhdm_offset)? {
        NextTable::Table(table) => table,
        NextTable::HugePage => return Ok(()),
    };

    let pte = unsafe { pt.add(pt_index) };
    let entry = unsafe { read_volatile(pte) };
    if entry & PAGE_PRESENT == 0 {
        unsafe {
            write_volatile(
                pte,
                (physical_address & PAGE_ADDRESS_MASK)
                    | PAGE_PRESENT
                    | PAGE_WRITABLE
                    | extra_flags,
            );
        }
        invalidate_page(virtual_address);
    } else if entry & PAGE_ADDRESS_MASK != physical_address & PAGE_ADDRESS_MASK {
        return Err(MapError::MappingConflict);
    }

    Ok(())
}

fn virtual_address_for_hhdm(hhdm_offset: u64, physical_address: u64) -> Result<u64, MapError> {
    hhdm_offset
        .checked_add(physical_address)
        .ok_or(MapError::AddressOverflow)
}

fn next_table(table: *mut u64, index: usize, hhdm_offset: u64) -> Result<NextTable, MapError> {
    let entry_ptr = unsafe { table.add(index) };
    let entry = unsafe { read_volatile(entry_ptr) };
    if entry & PAGE_PRESENT == 0 {
        let frame = mm::frame::allocate_frame().ok_or(MapError::PageTableAllocationFailed)?;
        let table_virtual = hhdm_offset
            .checked_add(frame.start_address())
            .ok_or(MapError::AddressOverflow)? as *mut u64;
        zero_table(table_virtual);
        unsafe {
            write_volatile(entry_ptr, frame.start_address() | PAGE_PRESENT | PAGE_WRITABLE);
        }
        return Ok(NextTable::Table(table_virtual));
    }
    if entry & PAGE_HUGE != 0 {
        return Ok(NextTable::HugePage);
    }

    Ok(NextTable::Table(
        hhdm_offset
            .checked_add(entry & PAGE_ADDRESS_MASK)
            .ok_or(MapError::AddressOverflow)? as *mut u64,
    ))
}

fn existing_next_table(
    table: *mut u64,
    index: usize,
    hhdm_offset: u64,
) -> Result<Option<*mut u64>, MapError> {
    let entry_ptr = unsafe { table.add(index) };
    let entry = unsafe { read_volatile(entry_ptr) };
    if entry & PAGE_PRESENT == 0 || entry & PAGE_HUGE != 0 {
        return Ok(None);
    }

    Ok(Some(
        hhdm_offset
            .checked_add(entry & PAGE_ADDRESS_MASK)
            .ok_or(MapError::AddressOverflow)? as *mut u64,
    ))
}

fn zero_table(table: *mut u64) {
    for index in 0..512 {
        unsafe {
            write_volatile(table.add(index), 0);
        }
    }
}

fn read_cr3() -> u64 {
    let value: u64;
    unsafe {
        asm!("mov {}, cr3", out(reg) value, options(nomem, nostack, preserves_flags));
    }
    value
}

fn invalidate_page(address: u64) {
    unsafe {
        asm!("invlpg [{}]", in(reg) address, options(nostack, preserves_flags));
    }
}

const fn page_table_index(address: u64, shift: u32) -> usize {
    ((address >> shift) & 0x1ff) as usize
}

enum NextTable {
    Table(*mut u64),
    HugePage,
}
