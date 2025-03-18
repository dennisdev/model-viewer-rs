use bytes::{Buf, BufMut};

static CP1252_ASCII_EXTENSION_CHARS: [char; 32] = [
    '€', '\u{0000}', '‚', 'ƒ', '„', '…', '†', '‡', 'ˆ', '‰', 'Š', '‹', 'Œ', '\u{0000}', 'Ž',
    '\u{0000}', '\u{0000}', '‘', '’', '“', '”', '•', '–', '—', '˜', '™', 'š', '›', 'œ', '\u{0000}',
    'ž', 'Ÿ',
];

fn u8_to_cp1252_ascii(c: u8) -> char {
    match c {
        128..160 => match CP1252_ASCII_EXTENSION_CHARS[(c - 128) as usize] {
            '\u{0000}' => '?',
            c => c,
        },
        _ => c as char,
    }
}

pub trait Packet: Buf + Sized {
    #[inline]
    fn skip(&mut self, n: usize) {
        self.advance(n);
    }

    #[inline]
    fn remaining(&self) -> usize {
        Buf::remaining(self)
    }

    #[inline]
    fn g1(&mut self) -> u8 {
        self.get_u8()
    }

    #[inline]
    fn g1s(&mut self) -> i8 {
        self.get_i8()
    }

    #[inline]
    fn g2(&mut self) -> u16 {
        self.get_u16()
    }

    #[inline]
    fn g2s(&mut self) -> i16 {
        self.get_i16()
    }

    #[inline]
    fn g3(&mut self) -> u32 {
        (self.get_u8() as u32) << 16 | self.get_u16() as u32
    }

    #[inline]
    fn g4(&mut self) -> u32 {
        self.get_u32()
    }

    #[inline]
    fn g4s(&mut self) -> i32 {
        self.get_i32()
    }

    #[inline]
    fn g8(&mut self) -> u64 {
        self.get_u64()
    }

    #[inline]
    fn g8s(&mut self) -> i64 {
        self.get_i64()
    }

    #[inline]
    fn get_smart_1_or_2(&mut self) -> i32 {
        if self.chunk()[0] < 128 {
            self.g1() as i32
        } else {
            (self.g2() as i32) - 32768
        }
    }

    #[inline]
    fn get_smart_1_or_2s(&mut self) -> i32 {
        if self.chunk()[0] < 128 {
            (self.g1() as i32) - 64
        } else {
            (self.g2() as i32) - 49152
        }
    }

    #[inline]
    fn get_smart_1_or_2_null(&mut self) -> i32 {
        self.get_smart_1_or_2() - 1
    }

    #[inline]
    fn get_smart_2_or_4(&mut self) -> u32 {
        if self.chunk()[0] & 0x80 == 0x80 {
            self.g4() & i32::MAX as u32
        } else {
            self.g2() as u32
        }
    }

    #[inline]
    fn get_array(&mut self, dst: &mut [u8]) {
        self.copy_to_slice(dst);
    }

    fn get_str_cp1252_to_utf8(&mut self) -> String {
        let mut chars: Vec<char> = Vec::new();
        loop {
            let c = self.g1();
            if c == 0 {
                break;
            }
            chars.push(u8_to_cp1252_ascii(c));
        }
        chars.into_iter().collect()
    }
}

impl<T: Buf + Sized> Packet for T {}

pub trait PacketMut: BufMut + Sized {
    #[inline]
    fn p1(&mut self, n: u8) {
        self.put_u8(n);
    }

    #[inline]
    fn p1s(&mut self, n: i8) {
        self.put_i8(n);
    }

    #[inline]
    fn p2(&mut self, n: u16) {
        self.put_u16(n);
    }

    #[inline]
    fn p2s(&mut self, n: i16) {
        self.put_i16(n);
    }

    #[inline]
    fn p3(&mut self, n: u32) {
        self.put_u8((n >> 16) as u8);
        self.put_u16(n as u16);
    }

    #[inline]
    fn p4(&mut self, n: u32) {
        self.put_u32(n);
    }

    #[inline]
    fn p4s(&mut self, n: i32) {
        self.put_i32(n);
    }
}

impl<T: BufMut + Sized> PacketMut for T {}
