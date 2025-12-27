//! Hit circle rendering with combo numbers

use bevy::prelude::*;

pub struct CirclesPlugin;

impl Plugin for CirclesPlugin {
    fn build(&self, _app: &mut App) {
        // Circles rendering is now done in the unified render_all_objects system
        // This plugin is kept for organization but registers nothing
    }
}

/// 7-segment display style digit rendering
/// Segments are:  0
///              1   2
///                3
///              4   5
///                6
/// Each digit is represented as a bitmask of which segments to draw
const DIGIT_SEGMENTS: [u8; 10] = [
    0b1110111, // 0: all except middle
    0b0100100, // 1: right side only
    0b1011101, // 2: top, top-right, middle, bottom-left, bottom
    0b1101101, // 3: top, right side, middle, bottom
    0b0101110, // 4: top-left, middle, right side
    0b1101011, // 5: top, top-left, middle, bottom-right, bottom
    0b1111011, // 6: all except top-right
    0b0100101, // 7: top, right side
    0b1111111, // 8: all segments
    0b1101111, // 9: all except bottom-left
];

/// Draw a number using gizmo lines (supports multi-digit numbers)
pub fn draw_number_gizmo(gizmos: &mut Gizmos, pos: Vec2, num: u32, size: f32, alpha: f32) {
    // Convert number to digits
    let digits: Vec<u32> = if num == 0 {
        vec![0]
    } else {
        let mut n = num;
        let mut d = Vec::new();
        while n > 0 {
            d.push(n % 10);
            n /= 10;
        }
        d.reverse();
        d
    };

    let digit_count = digits.len();
    let digit_width = size * 0.6;
    let total_width = digit_width * digit_count as f32;
    let start_x = -total_width / 2.0 + digit_width / 2.0;

    for (i, &digit) in digits.iter().enumerate() {
        let digit_pos = pos + Vec2::new(start_x + i as f32 * digit_width, 0.0);
        draw_single_digit(gizmos, digit_pos, digit, size, alpha);
    }
}

/// Draw a single digit 0-9 using 7-segment display style
fn draw_single_digit(gizmos: &mut Gizmos, pos: Vec2, digit: u32, size: f32, alpha: f32) {
    if digit > 9 {
        return;
    }
    
    let color = Color::srgba(1.0, 1.0, 1.0, alpha);
    let h = size * 0.5;  // half height
    let w = size * 0.25; // half width
    
    let segments = DIGIT_SEGMENTS[digit as usize];
    
    // Segment 0: top horizontal
    if segments & 0b0000001 != 0 {
        gizmos.line_2d(pos + Vec2::new(-w, h), pos + Vec2::new(w, h), color);
    }
    // Segment 1: top-left vertical
    if segments & 0b0000010 != 0 {
        gizmos.line_2d(pos + Vec2::new(-w, h), pos + Vec2::new(-w, 0.0), color);
    }
    // Segment 2: top-right vertical
    if segments & 0b0000100 != 0 {
        gizmos.line_2d(pos + Vec2::new(w, h), pos + Vec2::new(w, 0.0), color);
    }
    // Segment 3: middle horizontal
    if segments & 0b0001000 != 0 {
        gizmos.line_2d(pos + Vec2::new(-w, 0.0), pos + Vec2::new(w, 0.0), color);
    }
    // Segment 4: bottom-left vertical
    if segments & 0b0010000 != 0 {
        gizmos.line_2d(pos + Vec2::new(-w, 0.0), pos + Vec2::new(-w, -h), color);
    }
    // Segment 5: bottom-right vertical
    if segments & 0b0100000 != 0 {
        gizmos.line_2d(pos + Vec2::new(w, 0.0), pos + Vec2::new(w, -h), color);
    }
    // Segment 6: bottom horizontal
    if segments & 0b1000000 != 0 {
        gizmos.line_2d(pos + Vec2::new(-w, -h), pos + Vec2::new(w, -h), color);
    }
}
