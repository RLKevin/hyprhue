pub fn calculate_colors(data: &[u8]) -> (u8, u8, u8) {
    let mut r_total: u64 = 0;
    let mut g_total: u64 = 0;
    let mut b_total: u64 = 0;
    let mut count: u64 = 0;

    // Iterate with a stride of 4 (B, G, R, A)
    // We skip pixels for performance (every 10th pixel = 40 bytes)
    for chunk in data.chunks_exact(4).step_by(100) {
        // Wayland usually gives B G R A (Little Endian ARGB8888)
        let b = chunk[0] as u64;
        let g = chunk[1] as u64;
        let r = chunk[2] as u64;
        
        r_total += r;
        g_total += g;
        b_total += b;
        count += 1;
    }

    if count == 0 {
        return (0, 0, 0);
    }

    (
        (r_total / count) as u8,
        (g_total / count) as u8,
        (b_total / count) as u8,
    )
}
