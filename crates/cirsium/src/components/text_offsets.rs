use std::ops::Range;

pub(super) fn offset_from_utf16(text: &str, offset: usize) -> usize {
    let mut utf8_offset = 0;
    let mut utf16_count = 0;

    for ch in text.chars() {
        if utf16_count >= offset {
            break;
        }
        utf16_count += ch.len_utf16();
        utf8_offset += ch.len_utf8();
    }

    utf8_offset
}

pub(super) fn offset_to_utf16(text: &str, offset: usize) -> usize {
    let mut utf16_offset = 0;
    let mut utf8_count = 0;

    for ch in text.chars() {
        if utf8_count >= offset {
            break;
        }
        utf8_count += ch.len_utf8();
        utf16_offset += ch.len_utf16();
    }

    utf16_offset
}

pub(super) fn range_to_utf16(text: &str, range: &Range<usize>) -> Range<usize> {
    self::offset_to_utf16(text, range.start)..self::offset_to_utf16(text, range.end)
}

pub(super) fn range_from_utf16(text: &str, range: &Range<usize>) -> Range<usize> {
    self::offset_from_utf16(text, range.start)..self::offset_from_utf16(text, range.end)
}

pub(super) fn previous_boundary(text: &str, offset: usize) -> usize {
    text[..offset]
        .char_indices()
        .next_back()
        .map(|(ix, _)| ix)
        .unwrap_or(0)
}

pub(super) fn next_boundary(text: &str, offset: usize) -> usize {
    text[offset..]
        .char_indices()
        .nth(1)
        .map(|(ix, _)| offset + ix)
        .unwrap_or(text.len())
}
