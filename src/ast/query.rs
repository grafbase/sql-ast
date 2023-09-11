use crate::ast::{Delete, Insert, Select, Update};

/// A database query
#[derive(Debug, Clone, PartialEq)]
pub enum Query<'a> {
    Select(Box<Select<'a>>),
    Insert(Box<Insert<'a>>),
    Update(Box<Update<'a>>),
    Delete(Box<Delete<'a>>),
}

impl<'a> Query<'a> {
    pub fn is_select(&self) -> bool {
        matches!(self, Query::Select(_))
    }

    pub fn is_insert(&self) -> bool {
        matches!(self, Query::Insert(_))
    }

    pub fn is_update(&self) -> bool {
        matches!(self, Query::Update(_))
    }

    pub fn is_delete(&self) -> bool {
        matches!(self, Query::Delete(_))
    }
}
