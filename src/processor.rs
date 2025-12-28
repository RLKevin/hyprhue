pub fn calculate_colors(data: &[u8], width: u32, height: u32, stride: u32) -> ((u8, u8, u8), (u8, u8, u8)) {
    let mut left_r_acc: u64 = 0;
    let mut left_g_acc: u64 = 0;
    let mut left_b_acc: u64 = 0;
    let mut left_weight: u64 = 0;

    let mut right_r_acc: u64 = 0;
    let mut right_g_acc: u64 = 0;
    let mut right_b_acc: u64 = 0;
    let mut right_weight: u64 = 0;

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

            // Calculate saturation (max - min)
            let max = r.max(g).max(b);
            let min = r.min(g).min(b);
            let saturation = max - min;

            // Weight by brightness AND saturation to prioritize colorful pixels
            // Squaring the saturation gives it a lot more influence
            let weight = (r + g + b) + (saturation * saturation);
            
            if weight == 0 { continue; }

            if x < left_boundary {
                left_r_acc += r * r * weight;
                left_g_acc += g * g * weight;
                left_b_acc += b * b * weight;
                left_weight += weight;
            } else if x >= right_boundary {
                right_r_acc += r * r * weight;
                right_g_acc += g * g * weight;
                right_b_acc += b * b * weight;
                right_weight += weight;
            }
        }
    }

    let left_color = if left_weight > 0 {
        (
            (left_r_acc as f64 / left_weight as f64).sqrt() as u8,
            (left_g_acc as f64 / left_weight as f64).sqrt() as u8,
            (left_b_acc as f64 / left_weight as f64).sqrt() as u8,
        )
    } else {
        (0, 0, 0)
    };

    let right_color = if right_weight > 0 {
        (
            (right_r_acc as f64 / right_weight as f64).sqrt() as u8,
            (right_g_acc as f64 / right_weight as f64).sqrt() as u8,
            (right_b_acc as f64 / right_weight as f64).sqrt() as u8,
        )
    } else {
        (0, 0, 0)
    };

    (left_color, right_color)
}
