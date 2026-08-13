//! Textkonsole auf dem Bootloader-Framebuffer.
//!
//! Kein VGA-Textmodus: auf UEFI-Maschinen gibt es den nicht mehr. Der
//! Framebuffer ist der einzige Ausgabeweg, der ueberall funktioniert.
//! Die Konsole meldet sich als [`TextSink`] an, danach schreibt `println!`
//! gleichzeitig auf serielle Konsole und Bildschirm.

use spin::Mutex;

use super::font::{glyph, GLYPH_H, GLYPH_W};
use crate::boot::limine::FramebufferInfo;
use crate::kcore::print::TextSink;

struct FbState {
    info: FramebufferInfo,
    cols: usize,
    rows: usize,
    cx: usize,
    cy: usize,
    fg: u32,
    bg: u32,
}

// Sicherheit: `FbState` enthaelt einen Rohzeiger auf den Framebuffer. Dieser
// Speicher gehoert exklusiv der Konsole; der Zugriff wird durch den Mutex
// serialisiert.
unsafe impl Send for FbState {}

impl FbState {
    fn new(info: FramebufferInfo) -> Self {
        let cols = info.width / GLYPH_W;
        let rows = info.height / GLYPH_H;
        let fg = pack(&info, 0xC8, 0xD0, 0xD8);
        let bg = 0;
        let mut s = FbState { info, cols, rows, cx: 0, cy: 0, fg, bg };
        s.clear();
        s
    }

    fn clear(&mut self) {
        let total = self.info.pitch * self.info.height;
        unsafe { core::ptr::write_bytes(self.info.addr, 0, total) };
        self.cx = 0;
        self.cy = 0;
    }

    #[inline]
    fn put_pixel(&mut self, x: usize, y: usize, color: u32) {
        if x >= self.info.width || y >= self.info.height {
            return;
        }
        let bytes = (self.info.bpp as usize) / 8;
        let off = y * self.info.pitch + x * bytes;
        unsafe {
            let p = self.info.addr.add(off);
            match bytes {
                4 => core::ptr::write_volatile(p as *mut u32, color),
                3 => {
                    core::ptr::write_volatile(p, color as u8);
                    core::ptr::write_volatile(p.add(1), (color >> 8) as u8);
                    core::ptr::write_volatile(p.add(2), (color >> 16) as u8);
                }
                2 => core::ptr::write_volatile(p as *mut u16, color as u16),
                _ => {}
            }
        }
    }

    fn scroll(&mut self) {
        let row_bytes = self.info.pitch * GLYPH_H;
        let total = self.info.pitch * self.info.height;
        unsafe {
            core::ptr::copy(self.info.addr.add(row_bytes), self.info.addr, total - row_bytes);
            core::ptr::write_bytes(self.info.addr.add(total - row_bytes), 0, row_bytes);
        }
        if self.cy > 0 {
            self.cy -= 1;
        }
    }

    fn newline(&mut self) {
        self.cx = 0;
        self.cy += 1;
        if self.cy >= self.rows {
            self.scroll();
        }
    }

    fn draw(&mut self, ch: u8) {
        let g = glyph(ch);
        let px = self.cx * GLYPH_W;
        let py = self.cy * GLYPH_H;
        let (fg, bg) = (self.fg, self.bg);
        for (row, bits) in g.iter().enumerate() {
            for col in 0..GLYPH_W {
                let on = (bits >> (7 - col)) & 1 == 1;
                self.put_pixel(px + col, py + row, if on { fg } else { bg });
            }
        }
        self.cx += 1;
        if self.cx >= self.cols {
            self.newline();
        }
    }

    fn write(&mut self, s: &str) {
        for b in s.bytes() {
            match b {
                b'\n' => self.newline(),
                b'\r' => self.cx = 0,
                b'\t' => {
                    let next = (self.cx + 8) & !7;
                    while self.cx < next.min(self.cols) {
                        self.draw(b' ');
                    }
                }
                0x20..=0x7e => self.draw(b),
                _ => self.draw(b'?'),
            }
        }
    }
}

fn pack(info: &FramebufferInfo, r: u8, g: u8, b: u8) -> u32 {
    ((r as u32) << info.red_shift) | ((g as u32) << info.green_shift) | ((b as u32) << info.blue_shift)
}

/// Die Framebuffer-Konsole.
pub struct FbConsole {
    state: Mutex<Option<FbState>>,
}

impl TextSink for FbConsole {
    fn write_str(&self, s: &str) {
        if let Some(st) = self.state.lock().as_mut() {
            st.write(s);
        }
    }
}

/// Die eine Konsoleninstanz.
pub static CONSOLE: FbConsole = FbConsole { state: Mutex::new(None) };

/// Startet die Konsole und meldet sie bei `println!` an.
///
/// # Safety
/// `info` muss einen gueltigen, abgebildeten Framebuffer beschreiben.
pub unsafe fn init(info: FramebufferInfo) -> (usize, usize) {
    let st = FbState::new(info);
    let dims = (st.cols, st.rows);
    *CONSOLE.state.lock() = Some(st);
    crate::kcore::print::set_sink(&CONSOLE);
    dims
}
