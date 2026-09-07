const VERSION: usize = 5;
const SIZE: usize = 17 + VERSION * 4;
const DATA_CODEWORDS: usize = 108;
const ECC_CODEWORDS: usize = 26;
const MAX_PAYLOAD_BYTES: usize = 106;
const QUIET_ZONE: usize = 4;

pub fn render_terminal_qr(payload: &str) -> Result<String, String> {
    let matrix = encode(payload.as_bytes())?;
    let mut output = String::new();

    for y in 0..(SIZE + QUIET_ZONE * 2) {
        output.push_str("\x1b[47m");
        for x in 0..(SIZE + QUIET_ZONE * 2) {
            let dark = if x < QUIET_ZONE
                || y < QUIET_ZONE
                || x >= SIZE + QUIET_ZONE
                || y >= SIZE + QUIET_ZONE
            {
                false
            } else {
                matrix[y - QUIET_ZONE][x - QUIET_ZONE]
            };

            if dark {
                output.push_str("\x1b[40m  \x1b[47m");
            } else {
                output.push_str("  ");
            }
        }
        output.push_str("\x1b[0m\n");
    }

    Ok(output)
}

fn encode(payload: &[u8]) -> Result<Vec<Vec<bool>>, String> {
    if payload.len() > MAX_PAYLOAD_BYTES {
        return Err(format!(
            "pairing QR payload is {} bytes; QR v5-L supports at most {MAX_PAYLOAD_BYTES} bytes in this local encoder",
            payload.len()
        ));
    }

    let data = make_data_codewords(payload)?;
    let divisor = reed_solomon_divisor(ECC_CODEWORDS);
    let ecc = reed_solomon_remainder(&data, &divisor);
    let mut codewords = data;
    codewords.extend_from_slice(&ecc);

    let mut modules = vec![vec![false; SIZE]; SIZE];
    let mut function = vec![vec![false; SIZE]; SIZE];
    draw_function_patterns(&mut modules, &mut function);
    place_codewords(&codewords, &mut modules, &function)?;
    apply_mask_zero(&mut modules, &function);
    Ok(modules)
}

fn make_data_codewords(payload: &[u8]) -> Result<Vec<u8>, String> {
    let capacity_bits = DATA_CODEWORDS * 8;
    let mut bits = Vec::<bool>::with_capacity(capacity_bits);
    append_bits(&mut bits, 0b0100, 4); // byte mode
    append_bits(&mut bits, payload.len() as u32, 8); // versions 1..=9
    for &byte in payload {
        append_bits(&mut bits, u32::from(byte), 8);
    }

    if bits.len() > capacity_bits {
        return Err("pairing QR payload exceeds the fixed QR v5-L data capacity".to_owned());
    }

    let terminator = (capacity_bits - bits.len()).min(4);
    bits.extend(std::iter::repeat_n(false, terminator));
    while bits.len() % 8 != 0 {
        bits.push(false);
    }

    let mut data = bits
        .chunks_exact(8)
        .map(|chunk| {
            chunk
                .iter()
                .fold(0_u8, |value, bit| (value << 1) | u8::from(*bit))
        })
        .collect::<Vec<_>>();

    let mut pad = true;
    while data.len() < DATA_CODEWORDS {
        data.push(if pad { 0xEC } else { 0x11 });
        pad = !pad;
    }

    if data.len() != DATA_CODEWORDS {
        return Err("internal QR data-codeword length mismatch".to_owned());
    }
    Ok(data)
}

fn append_bits(target: &mut Vec<bool>, value: u32, count: usize) {
    for shift in (0..count).rev() {
        target.push(((value >> shift) & 1) != 0);
    }
}

fn reed_solomon_divisor(degree: usize) -> Vec<u8> {
    let mut result = vec![0_u8; degree];
    result[degree - 1] = 1;
    let mut root = 1_u8;

    for _ in 0..degree {
        for index in 0..degree {
            result[index] = gf_multiply(result[index], root);
            if index + 1 < degree {
                result[index] ^= result[index + 1];
            }
        }
        root = gf_multiply(root, 0x02);
    }
    result
}

fn reed_solomon_remainder(data: &[u8], divisor: &[u8]) -> Vec<u8> {
    let mut result = vec![0_u8; divisor.len()];
    for &byte in data {
        let factor = byte ^ result[0];
        result.rotate_left(1);
        if let Some(last) = result.last_mut() {
            *last = 0;
        }
        for (value, &coefficient) in result.iter_mut().zip(divisor) {
            *value ^= gf_multiply(coefficient, factor);
        }
    }
    result
}

fn gf_multiply(left: u8, right: u8) -> u8 {
    let mut x = u16::from(left);
    let mut y = right;
    let mut product = 0_u16;

    while y != 0 {
        if y & 1 != 0 {
            product ^= x;
        }
        y >>= 1;
        x <<= 1;
        if x & 0x100 != 0 {
            x ^= 0x11D;
        }
    }
    product as u8
}

fn draw_function_patterns(modules: &mut [Vec<bool>], function: &mut [Vec<bool>]) {
    for index in 0..SIZE {
        set_function(modules, function, 6, index, index % 2 == 0);
        set_function(modules, function, index, 6, index % 2 == 0);
    }

    draw_finder(modules, function, 3, 3);
    draw_finder(modules, function, SIZE - 4, 3);
    draw_finder(modules, function, 3, SIZE - 4);

    // Version 5 alignment centers are [6, 30]. The combinations touching
    // finder patterns are skipped, leaving only the bottom-right pattern.
    draw_alignment(modules, function, 30, 30);
    draw_format_bits(modules, function, 0);
}

fn draw_finder(
    modules: &mut [Vec<bool>],
    function: &mut [Vec<bool>],
    center_x: usize,
    center_y: usize,
) {
    let cx = center_x as isize;
    let cy = center_y as isize;
    for dy in -4_isize..=4 {
        for dx in -4_isize..=4 {
            let x = cx + dx;
            let y = cy + dy;
            if x < 0 || y < 0 || x >= SIZE as isize || y >= SIZE as isize {
                continue;
            }
            let distance = dx.abs().max(dy.abs());
            let dark = distance != 2 && distance != 4;
            set_function(modules, function, x as usize, y as usize, dark);
        }
    }
}

fn draw_alignment(
    modules: &mut [Vec<bool>],
    function: &mut [Vec<bool>],
    center_x: usize,
    center_y: usize,
) {
    for dy in -2_isize..=2 {
        for dx in -2_isize..=2 {
            let distance = dx.abs().max(dy.abs());
            set_function(
                modules,
                function,
                (center_x as isize + dx) as usize,
                (center_y as isize + dy) as usize,
                distance != 1,
            );
        }
    }
}

fn draw_format_bits(
    modules: &mut [Vec<bool>],
    function: &mut [Vec<bool>],
    mask: u8,
) {
    // Error correction level L uses format value 01.
    let data = (1_u16 << 3) | u16::from(mask);
    let mut remainder = data;
    for _ in 0..10 {
        remainder = (remainder << 1) ^ ((remainder >> 9) * 0x537);
    }
    let bits = ((data << 10) | remainder) ^ 0x5412;

    for index in 0..=5 {
        set_function(modules, function, 8, index, bit(bits, index));
    }
    set_function(modules, function, 8, 7, bit(bits, 6));
    set_function(modules, function, 8, 8, bit(bits, 7));
    set_function(modules, function, 7, 8, bit(bits, 8));
    for index in 9..=14 {
        set_function(modules, function, 14 - index, 8, bit(bits, index));
    }

    for index in 0..=7 {
        set_function(
            modules,
            function,
            SIZE - 1 - index,
            8,
            bit(bits, index),
        );
    }
    for index in 8..=14 {
        set_function(
            modules,
            function,
            8,
            SIZE - 15 + index,
            bit(bits, index),
        );
    }

    // Fixed dark module.
    set_function(modules, function, 8, SIZE - 8, true);
}

fn bit(value: u16, index: usize) -> bool {
    ((value >> index) & 1) != 0
}

fn set_function(
    modules: &mut [Vec<bool>],
    function: &mut [Vec<bool>],
    x: usize,
    y: usize,
    dark: bool,
) {
    modules[y][x] = dark;
    function[y][x] = true;
}

fn place_codewords(
    codewords: &[u8],
    modules: &mut [Vec<bool>],
    function: &[Vec<bool>],
) -> Result<(), String> {
    let total_bits = codewords.len() * 8;
    let mut bit_index = 0_usize;
    let mut right = SIZE as isize - 1;

    while right >= 1 {
        if right == 6 {
            right -= 1;
        }
        let upward = ((right + 1) & 2) == 0;

        for vertical in 0..SIZE {
            let y = if upward {
                SIZE - 1 - vertical
            } else {
                vertical
            };
            for offset in 0..2 {
                let x = right as usize - offset;
                if function[y][x] || bit_index >= total_bits {
                    continue;
                }
                modules[y][x] = ((codewords[bit_index >> 3]
                    >> (7 - (bit_index & 7)))
                    & 1)
                    != 0;
                bit_index += 1;
            }
        }
        right -= 2;
    }

    if bit_index != total_bits {
        return Err(format!(
            "internal QR placement mismatch: placed {bit_index} of {total_bits} bits"
        ));
    }
    Ok(())
}

fn apply_mask_zero(modules: &mut [Vec<bool>], function: &[Vec<bool>]) {
    for y in 0..SIZE {
        for x in 0..SIZE {
            if !function[y][x] && (x + y) % 2 == 0 {
                modules[y][x] = !modules[y][x];
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixed_encoder_accepts_version_five_l_capacity() {
        let payload = "a".repeat(MAX_PAYLOAD_BYTES);
        let matrix = encode(payload.as_bytes()).expect("payload should fit");
        assert_eq!(matrix.len(), SIZE);
        assert!(matrix.iter().all(|row| row.len() == SIZE));
    }

    #[test]
    fn fixed_encoder_rejects_oversized_payload() {
        let payload = "a".repeat(MAX_PAYLOAD_BYTES + 1);
        assert!(encode(payload.as_bytes()).is_err());
    }

    #[test]
    fn data_and_ecc_codeword_count_matches_version_five_l() {
        let data = make_data_codewords(b"assd://p?h=192.168.1.20&p=8765&t=0123456789abcdef")
            .expect("payload should fit");
        let divisor = reed_solomon_divisor(ECC_CODEWORDS);
        let ecc = reed_solomon_remainder(&data, &divisor);
        assert_eq!(data.len(), DATA_CODEWORDS);
        assert_eq!(ecc.len(), ECC_CODEWORDS);
    }
}
