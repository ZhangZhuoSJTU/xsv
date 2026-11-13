use std::collections::hash_map::{HashMap, Entry};
use std::fmt;
use std::io;
use std::iter::repeat;

use byteorder::{WriteBytesExt, BigEndian};
use csv;

use CliResult;
use index::Indexed;
use select::Selection;
use super::keys::{transform, ByteString};

pub(super) struct ValueIndex<R> {
    // This maps tuples of values to corresponding rows.
    pub(super) values: HashMap<Vec<ByteString>, Vec<usize>>,
    pub(super) idx: Indexed<R, io::Cursor<Vec<u8>>>,
    pub(super) num_rows: usize,
}

impl<R: io::Read + io::Seek> ValueIndex<R> {
    pub(super) fn new(
        mut rdr: csv::Reader<R>,
        sel: &Selection,
        casei: bool,
        nulls: bool,
    ) -> CliResult<ValueIndex<R>> {
        let mut val_idx = HashMap::with_capacity(10000);
        let mut row_idx = io::Cursor::new(Vec::with_capacity(8 * 10000));
        let (mut rowi, mut count) = (0usize, 0usize);

        // This logic is kind of tricky. Basically, we want to include
        // the header row in the line index (because that's what csv::index
        // does), but we don't want to include header values in the ValueIndex.
        if !rdr.has_headers() {
            // ... so if there are no headers, we seek to the beginning and
            // index everything.
            let mut pos = csv::Position::new();
            pos.set_byte(0);
            rdr.seek(pos)?;
        } else {
            // ... and if there are headers, we make sure that we've parsed
            // them, and write the offset of the header row to the index.
            rdr.byte_headers()?;
            row_idx.write_u64::<BigEndian>(0)?;
            count += 1;
        }

        let mut row = csv::ByteRecord::new();
        while rdr.read_byte_record(&mut row)? {
            // This is a bit hokey. We're doing this manually instead of using
            // the `csv-index` crate directly so that we can create both
            // indexes in one pass.
            row_idx.write_u64::<BigEndian>(row.position().unwrap().byte())?;

            let fields: Vec<_> = sel
                .select(&row)
                .map(|v| transform(v, casei))
                .collect();
            if nulls || !fields.iter().any(|f| f.is_empty()) {
                match val_idx.entry(fields) {
                    Entry::Vacant(v) => {
                        let mut rows = Vec::with_capacity(4);
                        rows.push(rowi);
                        v.insert(rows);
                    }
                    Entry::Occupied(mut v) => {
                        v.get_mut().push(rowi);
                    }
                }
            }
            rowi += 1;
            count += 1;
        }

        row_idx.write_u64::<BigEndian>(count as u64)?;
        let idx = Indexed::open(rdr, io::Cursor::new(row_idx.into_inner()))?;
        Ok(ValueIndex {
            values: val_idx,
            idx: idx,
            num_rows: rowi,
        })
    }
}

pub(super) struct RowMatches {
    written: Vec<bool>,
}

impl RowMatches {
    pub(super) fn new<R>(index: &ValueIndex<R>) -> RowMatches {
        let len = index.values.values().map(|rows| rows.len()).sum();
        RowMatches {
            written: repeat(false).take(len).collect(),
        }
    }

    pub(super) fn mark(&mut self, row_index: usize) {
        self.written[row_index] = true;
    }

    pub(super) fn contains(&self, row_index: usize) -> bool {
        self.written.get(row_index).map_or(false, |&written| written)
    }
}

impl<R> fmt::Debug for ValueIndex<R> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        // Sort the values by order of first appearance.
        let mut kvs = self.values.iter().collect::<Vec<_>>();
        kvs.sort_by(|&(_, v1), &(_, v2)| v1[0].cmp(&v2[0]));
        for (keys, rows) in kvs.into_iter() {
            // This is just for debugging, so assume Unicode for now.
            let keys = keys.iter()
                           .map(|k| String::from_utf8(k.to_vec()).unwrap())
                           .collect::<Vec<_>>();
            writeln!(f, "({}) => {:?}", keys.join(", "), rows)?
        }
        Ok(())
    }
}
