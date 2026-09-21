/// 0-based page numbers to render in pagination controls, `None` marking
/// an ellipsis gap. Shows every page up to 7, otherwise the first page, a
/// window around the current one, and the last page.
#[must_use]
pub fn page_window(current: usize, pages: usize) -> Vec<Option<usize>> {
    if pages <= 7 {
        return (0..pages).map(Some).collect();
    }
    let mut window = Vec::with_capacity(9);
    window.push(Some(0));
    let lo = current.saturating_sub(2).max(1);
    let hi = current.saturating_add(2).min(pages - 2);
    if lo > 1 {
        window.push(None);
    }
    window.extend((lo..=hi).map(Some));
    if hi < pages - 2 {
        window.push(None);
    }
    window.push(Some(pages - 1));
    window
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    pub fn test_page_window() {
        let numbered = |pages: &[Option<usize>]| pages.to_vec();
        assert_eq!(page_window(0, 0), numbered(&[]));
        assert_eq!(page_window(0, 1), numbered(&[Some(0)]));
        assert_eq!(
            page_window(3, 7),
            numbered(&[
                Some(0),
                Some(1),
                Some(2),
                Some(3),
                Some(4),
                Some(5),
                Some(6)
            ])
        );
        assert_eq!(
            page_window(0, 8),
            numbered(&[Some(0), Some(1), Some(2), None, Some(7)])
        );
        assert_eq!(
            page_window(4, 10),
            numbered(&[
                Some(0),
                None,
                Some(2),
                Some(3),
                Some(4),
                Some(5),
                Some(6),
                None,
                Some(9)
            ])
        );
        assert_eq!(
            page_window(9, 10),
            numbered(&[Some(0), None, Some(7), Some(8), Some(9)])
        );
        assert_eq!(
            page_window(6, 8),
            numbered(&[Some(0), None, Some(4), Some(5), Some(6), Some(7)])
        );
    }
}
