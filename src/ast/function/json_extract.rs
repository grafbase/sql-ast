use super::Function;
use crate::ast::{Expression, FunctionType};
use std::borrow::Cow;

#[derive(Debug, Clone, PartialEq)]
pub struct JsonExtract<'a> {
    pub(crate) column: Box<Expression<'a>>,
    pub(crate) path: JsonPath<'a>,
    pub(crate) extract_as_string: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JsonPath<'a> {
    #[cfg(feature = "mysql")]
    String(Cow<'a, str>),
    #[cfg(feature = "postgresql")]
    Array(Vec<Cow<'a, str>>),
}

impl<'a> JsonPath<'a> {
    #[cfg(feature = "mysql")]
    pub fn string<S>(string: S) -> JsonPath<'a>
    where
        S: Into<Cow<'a, str>>,
    {
        JsonPath::String(string.into())
    }

    #[cfg(feature = "postgresql")]
    pub fn array<A, V>(array: A) -> JsonPath<'a>
    where
        V: Into<Cow<'a, str>>,
        A: Into<Vec<V>>,
    {
        JsonPath::Array(array.into().into_iter().map(|v| v.into()).collect())
    }
}

/// Extracts a subset of a JSON blob given a path.
/// Two types of paths can be used:
/// - `String` paths, referring to JSON paths. This is supported by MySQL only.
/// - `Array` paths, supported by Postgres only.
pub fn json_extract<'a, C, P>(column: C, path: P, extract_as_string: bool) -> Function<'a>
where
    C: Into<Expression<'a>>,
    P: Into<JsonPath<'a>>,
{
    let fun = JsonExtract {
        column: Box::new(column.into()),
        path: path.into(),
        extract_as_string,
    };

    fun.into()
}

impl<'a> From<JsonExtract<'a>> for Function<'a> {
    fn from(value: JsonExtract<'a>) -> Self {
        Self {
            typ_: FunctionType::JsonExtract(value),
            alias: None,
        }
    }
}
