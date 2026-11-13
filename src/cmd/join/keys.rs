use std::str;

use csv;
use select::Selection;

pub(super) type ByteString = Vec<u8>;

pub(super) fn get_row_key(
    sel: &Selection,
    row: &csv::ByteRecord,
    casei: bool,
) -> Vec<ByteString> {
    sel.select(row).map(|v| transform(&v, casei)).collect()
}

pub(super) fn transform(bs: &[u8], casei: bool) -> ByteString {
    match str::from_utf8(bs) {
        Err(_) => bs.to_vec(),
        Ok(s) => {
            if !casei {
                s.trim().as_bytes().to_vec()
            } else {
                let norm: String =
                    s.trim().chars()
                     .map(|c| c.to_lowercase().next().unwrap()).collect();
                norm.into_bytes()
            }
        }
    }
}
