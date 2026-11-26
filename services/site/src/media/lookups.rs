//! Where recently served images are stored, kept in memory. A media row never changes after the upload, so a kept row is never out of date; the least recently used are dropped past [`MAX_KEPT`].

use std::collections::{BTreeMap, HashMap};
use std::sync::{Mutex, PoisonError};

use uuid::Uuid;

/// Rows kept at most: a few hundred kilobytes.
const MAX_KEPT: usize = 4096;

/// What serving an image needs from its row.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MediaRow {
    pub content_type: String,
    pub variants: Vec<i32>,
    /// `s3` or `database`.
    pub storage: String,
}

#[derive(Default)]
struct Table {
    rows: HashMap<Uuid, (MediaRow, u64)>,
    /// Ids by when they were last used, oldest first.
    order: BTreeMap<u64, Uuid>,
    clock: u64,
}

#[derive(Default)]
pub struct Lookups {
    table: Mutex<Table>,
}

impl Lookups {
    pub fn get(&self, id: Uuid) -> Option<MediaRow> {
        let mut guard = self.table.lock().unwrap_or_else(PoisonError::into_inner);
        let table = &mut *guard;
        table.clock += 1;
        let now = table.clock;
        let (row, used) = table.rows.get_mut(&id)?;
        let row = row.clone();
        let before = std::mem::replace(used, now);
        table.order.remove(&before);
        table.order.insert(now, id);
        Some(row)
    }

    pub fn insert(&self, id: Uuid, row: MediaRow) {
        let mut guard = self.table.lock().unwrap_or_else(PoisonError::into_inner);
        let table = &mut *guard;
        table.clock += 1;
        let now = table.clock;
        if let Some((_, before)) = table.rows.insert(id, (row, now)) {
            table.order.remove(&before);
        }
        table.order.insert(now, id);
        while table.rows.len() > MAX_KEPT {
            let Some((_, oldest)) = table.order.pop_first() else {
                break;
            };
            table.rows.remove(&oldest);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(storage: &str) -> MediaRow {
        MediaRow {
            content_type: "image/jpeg".into(),
            variants: vec![480, 960],
            storage: storage.into(),
        }
    }

    #[test]
    fn keeps_the_most_recently_used_rows() {
        let lookups = Lookups::default();
        let first = Uuid::now_v7();
        lookups.insert(first, row("s3"));
        let ids: Vec<Uuid> = (0..MAX_KEPT).map(|_| Uuid::now_v7()).collect();
        for (index, id) in ids.iter().enumerate() {
            lookups.insert(*id, row("s3"));
            if index == 10 {
                // Using the first row keeps it while others arrive.
                assert_eq!(lookups.get(first), Some(row("s3")));
            }
        }
        assert_eq!(lookups.get(first), Some(row("s3")));
        // The least recently used went instead.
        assert_eq!(lookups.get(ids[0]), None);
        assert_eq!(lookups.get(ids[MAX_KEPT - 1]), Some(row("s3")));
        let table = lookups.table.lock().unwrap();
        assert_eq!(table.rows.len(), MAX_KEPT);
        assert_eq!(table.order.len(), MAX_KEPT);
    }
}
