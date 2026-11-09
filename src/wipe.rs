//! Text that can hold secrets, kept in `Vec<char>`s without leaving copies behind: every buffer it
//! has been in is overwritten before it's given back, the unused part of it too (where a deleted
//! character stays). Best effort: it covers these buffers, not what the terminal's events or the
//! caller do with the text.

use zeroize::Zeroize;

/// Overwrite the whole buffer, its unused capacity included, and empty it.
pub(crate) fn wipe(text: &mut Vec<char>) {
    text.zeroize();
}

/// A copy of `old` with room for `more`; `old` is left wiped.
fn grown(old: &mut Vec<char>, more: usize) -> Vec<char> {
    let capacity = (old.len() + more).max(old.capacity() * 2).max(32);
    let mut new = Vec::with_capacity(capacity);
    new.extend_from_slice(old);
    wipe(old);
    new
}

/// Room for `more` characters. A `Vec` growing by itself would free the old buffer as it is.
pub(crate) fn make_room(text: &mut Vec<char>, more: usize) {
    if text.capacity() - text.len() < more {
        let mut old = grown(text, more);
        std::mem::swap(text, &mut old);
    }
}

/// `text` as characters, in one exact allocation.
pub(crate) fn chars(text: &str) -> Vec<char> {
    let mut out = Vec::with_capacity(text.chars().count());
    out.extend(text.chars());
    out
}

/// `text` as a string, in one exact allocation.
pub(crate) fn string(text: &[char]) -> String {
    let mut out = String::with_capacity(text.iter().map(|c| c.len_utf8()).sum());
    out.extend(text.iter());
    out
}

#[cfg(test)]
pub(crate) fn all_zero(text: &mut Vec<char>) -> bool {
    assert!(text.is_empty());
    assert!(text.capacity() > 0);
    text.spare_capacity_mut()
        .iter()
        .all(|c| unsafe { c.as_ptr().cast::<u32>().read() } == 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_wipe_reaches_a_deleted_character() {
        let mut text = vec!['a', 'b', 'c'];
        text.pop();
        wipe(&mut text);
        assert!(all_zero(&mut text));
    }

    #[test]
    fn growing_keeps_the_text_and_wipes_the_old_buffer() {
        let mut old = vec!['a', 'b', 'c'];
        let new = grown(&mut old, 1);
        assert_eq!(new, ['a', 'b', 'c']);
        assert!(new.capacity() > 3);
        assert!(all_zero(&mut old));
    }

    #[test]
    fn room_is_made_only_when_needed() {
        let mut text = Vec::with_capacity(4);
        text.push('a');
        let before = text.as_ptr();
        make_room(&mut text, 3);
        assert_eq!(text.as_ptr(), before);
        make_room(&mut text, 4);
        assert_ne!(text.as_ptr(), before);
        assert_eq!(text, ['a']);
    }

    #[test]
    fn conversions_take_one_exact_allocation() {
        let text = chars("é€x");
        assert_eq!(text.capacity(), 3);
        let s = string(&text);
        assert_eq!(s, "é€x");
        assert_eq!(s.capacity(), s.len());
    }
}
