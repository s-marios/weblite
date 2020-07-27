pub fn to_usize(hex: &str) -> Option<usize> {
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
    Some(
        hex.chars()
            .map(|x| x.to_digit(16).unwrap())
            .fold(0usize, |acc, digit| (acc << 4) + digit as usize),
    )
}

#[cfg(debug)]
mod tests {
    use super::to_usize;

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

    fn is_511() {
        let hex = "0xfFfF";
        assert_eq!(to_usize(hex).unwrap(), 511usize);
    }
}
