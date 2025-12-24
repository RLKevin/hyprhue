pub fn calculate_colors(data: &[u8], width: u32, height: u32, stride: u32) -> ((u8, u8, u8), (u8, u8, u8)) {
    let mut left_r: u64 = 0;
    let mut left_g: u64 = 0;
    let mut left_b: u64 = 0;
    let mut left_count: u64 = 0;

    let mut right_r: u64 = 0;
    let mut right_g: u64 = 0;
    let mut right_b: u64 = 0;
    let mut right_count: u64 = 0;

    let left_boundary = (width as f32 * 0.25) as u32;
    let right_boundary = (width as f32 * 0.75) as u32;

    // Iterate over rows with a step for performance
    for y in (0..height).step_by(10) {
        let row_start = (y * stride) as usize;
        
        // Iterate over pixels in the row with a step
        for x in (0..width).step_by(10) {
            let pixel_offset = row_start + (x * 4) as usize;
            
            // Safety check
            if pixel_offset + 4 > data.len() {
                continue;
            }

            // Wayland usually gives B G R A (Little Endian ARGB8888)
            let b = data[pixel_offset] as u64;
            let g = data[pixel_offset + 1] as u64;
            let r = data[pixel_offset + 2] as u64;

            if x < left_boundary {
                left_r += r;
                left_g += g;
                left_b += b;
                left_count += 1;
            } else if x >= right_boundary {
                right_r += r;
                right_g += g;
                right_b += b;
                right_count += 1;
            }
        }
    }

    let left_color = if left_count > 0 {
        (
            (left_r / left_count) as u8,
            (left_g / left_count) as u8,
            (left_b / left_count) as u8,
        )
    } else {
        (0, 0, 0)
    };

    let right_color = if right_count > 0 {
        (
            (right_r / right_count) as u8,
            (right_g / right_count) as u8,
            (right_b / right_count) as u8,
        )
    } else {
        (0, 0, 0)
    };

    (left_color, right_color)
}
