//! A line diff with no dependency: the longest common subsequence of the
//! lines of two texts, after their common head and tail are set aside. It
//! gives each unchanged old line its new number. `remap` moves citations by
//! it.

/// Above this many cells, the middle is not aligned and counts as changed. The
/// cells are the old lines times the new lines, after the common head and tail.
/// More cells need a table of more than 64 MB.
const MAX_CELLS: usize = 16 * 1024 * 1024;

/// For each line of `old` (index 0 is line 1), its line number in `new`, or
/// `None` when the edit removed or changed it.
pub(crate) fn line_map(old: &[&str], new: &[&str]) -> Vec<Option<usize>> {
    let (old_len, new_len) = (old.len(), new.len());
    let mut map = vec![None; old_len];
    let mut head = 0;
    while head < old_len && head < new_len && old[head] == new[head] {
        map[head] = Some(head + 1);
        head += 1;
    }
    let mut tail = 0;
    while tail < old_len - head && tail < new_len - head && old[old_len - 1 - tail] == new[new_len - 1 - tail] {
        map[old_len - 1 - tail] = Some(new_len - tail);
        tail += 1;
    }
    let left = &old[head..old_len - tail];
    let right = &new[head..new_len - tail];
    if left.is_empty() || right.is_empty() || left.len().saturating_mul(right.len()) > MAX_CELLS {
        slide_up(&mut map, new);
        return map;
    }
    // lcs[i * width + j] is the length of a longest common subsequence of
    // left[i..] and right[j..]. A length is at most MAX_CELLS, so u32 holds it.
    let width = right.len() + 1;
    let mut lcs = vec![0u32; (left.len() + 1) * width];
    for i in (0..left.len()).rev() {
        for j in (0..right.len()).rev() {
            lcs[i * width + j] = if left[i] == right[j] {
                lcs[(i + 1) * width + j + 1] + 1
            } else {
                lcs[(i + 1) * width + j].max(lcs[i * width + j + 1])
            };
        }
    }
    // Two equal lines can always be matched: some longest subsequence does.
    let (mut i, mut j) = (0, 0);
    while i < left.len() && j < right.len() {
        if left[i] == right[j] {
            map[head + i] = Some(head + j + 1);
            i += 1;
            j += 1;
        } else if lcs[(i + 1) * width + j] >= lcs[i * width + j + 1] {
            i += 1;
        } else {
            j += 1;
        }
    }
    slide_up(&mut map, new);
    map
}

/// Each unchanged line at the earliest place where the same text allows it. The
/// common tail is matched from the end. So new functions after the closing `}`
/// of a cited function would take that `}` as theirs. When the first added line
/// has the same text as the line after the addition, the addition can come after
/// that line. Then the citation keeps its own lines.
fn slide_up(map: &mut [Option<usize>], new: &[&str]) {
    let mut before = 0;
    for slot in map {
        if let Some(at) = *slot {
            if at > before + 1 && new[before] == new[at - 1] {
                *slot = Some(before + 1);
            }
            before = slot.unwrap_or(at);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn map(old: &str, new: &str) -> Vec<Option<usize>> {
        let o: Vec<&str> = old.lines().collect();
        let n: Vec<&str> = new.lines().collect();
        line_map(&o, &n)
    }

    #[test]
    fn an_unchanged_text_maps_to_itself() {
        assert_eq!(map("a\nb\nc\n", "a\nb\nc\n"), vec![Some(1), Some(2), Some(3)]);
        assert_eq!(map("", ""), Vec::<Option<usize>>::new());
    }

    #[test]
    fn a_line_added_above_moves_each_line_below() {
        assert_eq!(map("a\nb\nc\n", "new\na\nb\nc\n"), vec![Some(2), Some(3), Some(4)]);
        assert_eq!(map("a\nb\nc\n", "a\nnew\nnew\nb\nc\n"), vec![Some(1), Some(4), Some(5)]);
    }

    #[test]
    fn a_changed_or_removed_line_has_no_new_number() {
        assert_eq!(map("a\nb\nc\n", "a\nB\nc\n"), vec![Some(1), None, Some(3)]);
        assert_eq!(map("a\nb\nc\nd\n", "a\nd\n"), vec![Some(1), None, None, Some(2)]);
    }

    /// New code after a closing line, with a change above it: the old closing
    /// line stays with its own part, not with the new code.
    #[test]
    fn an_addition_after_a_closing_line_comes_after_it() {
        let old = "top\nfn a {\nx\n}\n}\n";
        let new = "TOP\nfn a {\nx\n}\nfn b {\ny\n}\n}\n";
        assert_eq!(map(old, new), vec![None, Some(2), Some(3), Some(4), Some(8)]);
    }

    #[test]
    fn the_middle_is_aligned_around_a_change() {
        let old = "h\nx\n1\n2\n3\ny\nt\n";
        let new = "h\nX\n0\n1\n2\n3\nY\nt\n";
        assert_eq!(map(old, new), vec![Some(1), None, Some(4), Some(5), Some(6), None, Some(8)]);
    }
}
