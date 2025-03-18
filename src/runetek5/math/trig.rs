use std::{f64::consts::PI, sync::LazyLock};

pub type JagDegrees = u16;

pub const JAG_DEGREES_RANGE: usize = 16384;

pub const JAG_TO_RADIANS: f64 = PI * 2.0 / JAG_DEGREES_RANGE as f64;

pub const DEGREES_TO_JAG: f64 = JAG_DEGREES_RANGE as f64 / 360.0;
pub const JAG_45_DEGREES: JagDegrees = (45.0 * DEGREES_TO_JAG) as JagDegrees;
pub const JAG_90_DEGREES: JagDegrees = (90.0 * DEGREES_TO_JAG) as JagDegrees;
pub const JAG_180_DEGREES: JagDegrees = (180.0 * DEGREES_TO_JAG) as JagDegrees;
pub const JAG_270_DEGREES: JagDegrees = (270.0 * DEGREES_TO_JAG) as JagDegrees;

pub static SINE: LazyLock<[i32; JAG_DEGREES_RANGE]> = LazyLock::new(|| calculate_jag_sin_table());
pub static COSINE: LazyLock<[i32; JAG_DEGREES_RANGE]> = LazyLock::new(|| calculate_jag_cos_table());

fn calculate_jag_sin_table() -> [i32; JAG_DEGREES_RANGE] {
    let mut table = [0; JAG_DEGREES_RANGE];
    for i in 0..JAG_DEGREES_RANGE {
        table[i] = (16384.0 * (i as f64 * JAG_TO_RADIANS).sin()) as i32;
    }
    table
}

fn calculate_jag_cos_table() -> [i32; JAG_DEGREES_RANGE] {
    let mut table = [0; JAG_DEGREES_RANGE];
    for i in 0..JAG_DEGREES_RANGE {
        table[i] = (16384.0 * (i as f64 * JAG_TO_RADIANS).cos()) as i32;
    }
    table
}
