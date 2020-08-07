fn check(hex: &str) -> Option<&str> {
    let mut hex = &hex[..];
    if hex.starts_with("0x") || hex.starts_with("0X") {
        hex = &hex[2..];
    }
    if hex.len() < 2 || hex.len() % 2 != 0 {
        return None;
    }
    if !hex.chars().all(|x| x.is_ascii_hexdigit()) {
        return None;
    }
    Some(hex)
}

pub fn to_usize(hex: &str) -> Option<usize> {
    Some(
        check(hex)?
            .chars()
            .map(|x| x.to_digit(16).unwrap())
            .fold(0usize, |acc, digit| (acc << 4) + digit as usize),
    )
}

pub fn to_bytes(hex: &str) -> Option<Vec<u8>> {
    Some(
        check(hex)?
            .chars()
            .map(|x| x.to_digit(16).unwrap() as u8)
            .collect::<Vec<u8>>()
            .chunks_exact(2)
            .map(|chunk| ((chunk[0] << 4) + chunk[1]) as u8)
            .collect::<Vec<u8>>(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_zero() {
        let hex = "0x00";
        assert_eq!(to_usize(hex).unwrap(), 0);
    }

    #[test]
    fn is_255() {
        let hex = "0xFF";
        assert_eq!(to_usize(hex).unwrap(), 255usize);
    }

    #[test]
    fn is_511() {
        let hex = "0x01fF";
        assert_eq!(to_usize(hex).unwrap(), 511usize);
    }

    #[test]
    fn is_511_to_bytes() {
        let res = to_bytes("0x01fF").unwrap();
        assert_eq!(res[0], 1u8);
        assert_eq!(res[1], 255u8);
    }

    #[test]
    fn ffff_is_two_bytes_255() {
        let hex = "FFFF";
        let res = to_bytes(hex).unwrap();
        assert_eq!(res[0], 255);
        assert_eq!(res[1], 255);
    }
}
