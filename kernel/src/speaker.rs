#![allow(dead_code)]
use core::arch::asm;

/// Sends a byte to the specified I/O port.
#[inline]
unsafe fn outb(port: u16, val: u8) {
    asm!("out dx, al", in("dx") port, in("al") val, options(nomem, nostack, preserves_flags));
}

/// Receives a byte from the specified I/O port.
#[inline]
unsafe fn inb(port: u16) -> u8 {
    let mut val: u8;
    asm!("in al, dx", out("al") val, in("dx") port, options(nomem, nostack, preserves_flags));
    val
}

/// Simple busy-wait delay for bare-metal panic state.
/// Cycles depend on CPU speed, but this is a rough stall.
fn busy_wait_approx(ms: u32) {
    for _ in 0..(ms * 50_000) {
        unsafe { asm!("pause", options(nomem, nostack, preserves_flags)); }
    }
}

/// Plays a specific frequency on the PC Speaker using PIT Channel 2.
pub fn play_tone(freq: u32) {
    if freq == 0 { return; }
    let divisor: u32 = 1193180 / freq;
    
    unsafe {
        // Set PIT to Channel 2, Square Wave Generator
        outb(0x43, 0xB6);
        // Send divisor low byte
        outb(0x42, (divisor & 0xFF) as u8);
        // Send divisor high byte
        outb(0x42, ((divisor >> 8) & 0xFF) as u8);
        
        // Enable PC Speaker (Bits 0 and 1 of Port 0x61)
        let mut tmp = inb(0x61);
        if tmp != (tmp | 3) {
            outb(0x61, tmp | 3);
        }
    }
}

/// Stops the PC Speaker.
pub fn stop_tone() {
    unsafe {
        let tmp = inb(0x61) & 0xFC;
        outb(0x61, tmp);
    }
}

/// The MD-80 "Altitude" Warning Homage (Continuous / Wii-Buzz Style)
/// Rapidly alternates between 250 Hz, 500 Hz, and 626 Hz (exact dissonant harmonic extracted via FFT) 
/// infinitely, creating a constant, disturbing aviation-style crash alarm.
pub fn play_md80_altitude_warning() -> ! {
    loop {
        // Tone 1: 250 Hz (Sub-harmonic)
        play_tone(250);
        busy_wait_approx(80); // ~80ms

        // Tone 2: 500 Hz (Dominant peak from FFT)
        play_tone(500);
        busy_wait_approx(80); // ~80ms
        
        // Tone 3: 626 Hz (Dissonant shift from FFT: 625.70 Hz)
        play_tone(626);
        busy_wait_approx(80); // ~80ms
        
        // Ses kesilmez (Sonsuz Wii Crash Buzz efekti)
    }
}
