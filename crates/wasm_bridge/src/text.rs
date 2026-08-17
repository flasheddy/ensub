pub(crate) fn utf16_offset(text: &str, byte_offset: usize) -> Option<usize> {
    text.get(..byte_offset)
        .map(|prefix| prefix.encode_utf16().count())
}
