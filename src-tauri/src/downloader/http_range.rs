use eyre::{WrapErr, ensure};

pub fn validate_content_range(start: u64, end: u64, header: &str) -> eyre::Result<u64> {
    let expected_length = end
        .checked_sub(start)
        .and_then(|length| length.checked_add(1))
        .ok_or_else(|| eyre::eyre!("无效的分片范围: {start}-{end}"))?;
    let range = header
        .strip_prefix("bytes ")
        .and_then(|value| value.split_once('/'))
        .ok_or_else(|| eyre::eyre!("无效的 Content-Range: {header}"))?;
    ensure!(
        range.0 == format!("{start}-{end}"),
        "Content-Range 与请求范围不符: {header}，预期 {start}-{end}"
    );
    if range.1 != "*" {
        let total = range.1.parse::<u64>().wrap_err("无效的媒体总长度")?;
        ensure!(total > end, "Content-Range 超出媒体总长度: {header}");
    }
    Ok(expected_length)
}

pub fn validate_chunk_length(expected: u64, actual: usize) -> eyre::Result<()> {
    ensure!(
        u64::try_from(actual).ok() == Some(expected),
        "分片长度不符: 预期 {expected} 字节，实际 {actual} 字节"
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_exact_inclusive_range_and_unknown_total() {
        assert_eq!(validate_content_range(2, 4, "bytes 2-4/5").unwrap(), 3);
        assert_eq!(validate_content_range(0, 0, "bytes 0-0/*").unwrap(), 1);
        assert!(validate_chunk_length(3, 3).is_ok());
    }

    #[test]
    fn rejects_wrong_range_units_and_total() {
        for header in ["bytes 0-2/5", "items 2-4/5", "bytes 2-4/4", "bytes 2-4/no"] {
            assert!(validate_content_range(2, 4, header).is_err(), "{header}");
        }
        assert!(validate_content_range(4, 2, "bytes 4-2/5").is_err());
        assert!(validate_content_range(0, u64::MAX, "bytes 0-0/*").is_err());
    }

    #[test]
    fn rejects_truncated_and_oversized_bodies() {
        assert!(validate_chunk_length(3, 2).is_err());
        assert!(validate_chunk_length(3, 4).is_err());
    }
}
