#[allow(clippy::cast_possible_truncation)]
#[allow(clippy::cast_possible_wrap)]
#[allow(clippy::cast_sign_loss)]
#[allow(clippy::cast_precision_loss)]
#[must_use]
pub fn format_bytes(bytes: u64) -> String {
    const PREFIXES: [&str; 9] = ["", "Ki", "Mi", "Gi", "Ti", "Pi", "Ei", "Zi", "Yi"];

    if bytes < 1024 {
        return format!("{bytes} B");
    }

    let bytes_f64 = bytes as f64;

    // calculate log1024(bytes) and round down
    let power_of_1024 = (bytes_f64.log2() / 10.0).floor() as i32;

    let number = bytes_f64 / 1024f64.powi(power_of_1024);
    let formatted = format!("{number:0.2}");
    let formatted = formatted.trim_end_matches('0').trim_end_matches('.'); // Remove trailing zeros

    let prefix = PREFIXES[power_of_1024 as usize];

    format!("{formatted} {prefix}B")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    pub fn test_format_bytes() {
        assert_eq!(format_bytes(0), "0 B");
        assert_eq!(format_bytes(1), "1 B");
        assert_eq!(format_bytes(1024), "1 KiB");
        assert_eq!(format_bytes(1024 * 1024), "1 MiB");
        assert_eq!(format_bytes(1024 * 1024 * 1024), "1 GiB");
        assert_eq!(format_bytes(1024 * 1024 * 1024 * 1024), "1 TiB");

        assert_eq!(format_bytes(5 * 1024 * 1024), "5 MiB");

        assert_eq!(format_bytes(1024 + 256), "1.25 KiB");
        assert_eq!(format_bytes(1024 + 100), "1.1 KiB");
        assert_eq!(format_bytes(1024 + 1000), "1.98 KiB");

        assert_eq!(format_bytes(u64::MAX), "16 EiB");
    }
}
