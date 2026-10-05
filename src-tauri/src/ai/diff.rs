//! Line diff for previewing proposed changes. Lines prefixed with `+` are added, `-` removed,
//! and a space marks unchanged context. Uses LCS, falling back to a full replacement for
//! very large inputs so preview cost stays bounded.

pub const MAX_DIFF_CHARS: usize = 20_000;
const MAX_CELLS: usize = 4_000_000;

pub fn line_diff(old: &str, new: &str) -> String {
    let a: Vec<&str> = old.lines().collect();
    let b: Vec<&str> = new.lines().collect();
    let mut out: Vec<String> = Vec::new();

    if a.len().saturating_mul(b.len()) > MAX_CELLS {
        out.extend(a.iter().map(|l| format!("-{l}")));
        out.extend(b.iter().map(|l| format!("+{l}")));
    } else {
        let (n, m) = (a.len(), b.len());
        let mut lcs = vec![vec![0u32; m + 1]; n + 1];
        for i in (0..n).rev() {
            for j in (0..m).rev() {
                lcs[i][j] = if a[i] == b[j] {
                    lcs[i + 1][j + 1] + 1
                } else {
                    lcs[i + 1][j].max(lcs[i][j + 1])
                };
            }
        }
        let (mut i, mut j) = (0, 0);
        while i < n && j < m {
            if a[i] == b[j] {
                out.push(format!(" {}", a[i]));
                i += 1;
                j += 1;
            } else if lcs[i + 1][j] >= lcs[i][j + 1] {
                out.push(format!("-{}", a[i]));
                i += 1;
            } else {
                out.push(format!("+{}", b[j]));
                j += 1;
            }
        }
        out.extend(a[i..].iter().map(|l| format!("-{l}")));
        out.extend(b[j..].iter().map(|l| format!("+{l}")));
    }

    let mut text = String::new();
    for line in out {
        if text.len() + line.len() + 1 > MAX_DIFF_CHARS {
            text.push_str("… diff truncated for display\n");
            break;
        }
        text.push_str(&line);
        text.push('\n');
    }
    text
}

/// Counts added and removed lines in a diff produced by `line_diff`.
pub fn change_counts(diff: &str) -> (usize, usize) {
    diff.lines().fold((0, 0), |(added, removed), line| {
        if line.starts_with('+') {
            (added + 1, removed)
        } else if line.starts_with('-') {
            (added, removed + 1)
        } else {
            (added, removed)
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn marks_only_changed_lines() {
        let diff = line_diff("one\ntwo\nthree", "one\n2\nthree");
        assert_eq!(diff, " one\n-two\n+2\n three\n");
        assert_eq!(change_counts(&diff), (1, 1));
    }

    #[test]
    fn new_document_is_all_additions() {
        assert_eq!(change_counts(&line_diff("", "a\nb")), (2, 0));
    }

    #[test]
    fn identical_text_has_no_changes() {
        assert_eq!(change_counts(&line_diff("same\ntext", "same\ntext")), (0, 0));
    }
}
