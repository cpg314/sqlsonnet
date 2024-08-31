//! Representation of SQL queries.

#![allow(unstable_name_collisions)]
pub use expr::*;

use itertools::Itertools;
use serde::{de::Error, Deserialize, Serialize};

/// A set of [`Query`].
#[derive(Serialize, Debug, Default, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Queries(Vec<Query>);

// Supports deserializing Queries from Query while keeping good error reporting.
struct Visitor(Queries);
impl<'de> serde::de::Visitor<'de> for Visitor {
    type Value = Queries;

    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(formatter, "Expected one or multiple queries")
    }
    fn visit_map<A>(mut self, mut map: A) -> Result<Self::Value, A::Error>
    where
        A: serde::de::MapAccess<'de>,
    {
        loop {
            let Some(key) = map.next_key::<String>()? else {
                break;
            };
            if key == "select" {
                let value = map.next_value::<select::Query>()?;
                self.0 .0.push(Query::Select(value));
            } else {
                return Err(A::Error::custom(format!("Unsupported query type {}", key)));
            }
        }
        Ok(self.0)
    }
    fn visit_seq<A>(mut self, mut seq: A) -> Result<Self::Value, A::Error>
    where
        A: serde::de::SeqAccess<'de>,
    {
        while let Some(x) = seq.next_element::<Query>()? {
            self.0 .0.push(x);
        }
        Ok(self.0)
    }
}
impl<'de> Deserialize<'de> for Queries {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let visitor = Visitor(Default::default());
        deserializer.deserialize_any(visitor)
    }
}

impl Queries {
    pub fn len(&self) -> usize {
        self.0.len()
    }
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}
impl From<Vec<Query>> for Queries {
    fn from(source: Vec<Query>) -> Self {
        Self(source)
    }
}

impl IntoIterator for Queries {
    type Item = Query;

    type IntoIter = std::vec::IntoIter<Query>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}
impl<'a> IntoIterator for &'a Queries {
    type Item = &'a Query;

    type IntoIter = std::slice::Iter<'a, Query>;

    fn into_iter(self) -> Self::IntoIter {
        let s = &self.0;
        s.iter()
    }
}

/// SQL expressions
pub mod expr {
    use super::*;

    #[derive(Eq, PartialEq, Debug, Deserialize, Serialize)]
    pub struct Prefix(pub String);

    #[derive(Eq, PartialEq, Debug, Deserialize, Serialize)]
    pub struct Operator(pub String);
    impl Operator {
        pub fn linebreak(&self) -> bool {
            ["and", "or"].contains(&self.0.to_lowercase().as_str())
        }
    }
    #[derive(Deserialize, Serialize, Debug)]
    pub struct FloatEq(f64);
    impl std::cmp::Eq for FloatEq {}
    impl std::fmt::Display for FloatEq {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "{}", self.0)
        }
    }
    impl std::cmp::PartialEq for FloatEq {
        fn eq(&self, other: &Self) -> bool {
            self.0.to_le_bytes() == other.0.to_le_bytes()
        }
    }

    #[derive(Deserialize, Serialize, Debug, PartialEq, Eq)]
    #[serde(deny_unknown_fields, untagged)]
    pub enum Expr {
        Raw(String),
        RawBool(bool),
        RawInteger(i64),
        RawFloat(FloatEq),
        Prefix(Prefix, Box<Expr>),
        // [expr, op, [expr]]
        Operator(Box<Expr>, Operator, Box<Expr>),
        // [expr, [[op, expr], ...]]
        OperatorSeq(Box<Expr>, Vec<(Operator, Box<Expr>)>),
        Subquery(Box<Query>),
        FunctionCall { r#fn: String, params: ExprList },
        Aliased { expr: Box<Expr>, alias: String },
    }
    impl From<&str> for Expr {
        fn from(source: &str) -> Self {
            Self::Raw(
                source
                    .replace('\n', " ")
                    .split(' ')
                    .filter(|s| !s.is_empty())
                    .join(" "),
            )
        }
    }

    impl Expr {
        pub fn is_raw(&self) -> bool {
            matches!(self, Self::Raw(_) | Self::RawBool(_) | Self::RawInteger(_))
        }
        pub fn operator(self, op: Operator, right: Expr) -> Self {
            Self::Operator(Box::new(self), op, Box::new(right))
        }
    }
    impl Default for Expr {
        fn default() -> Self {
            Self::Raw(Default::default())
        }
    }

    #[derive(Deserialize, Serialize, Debug, Default, PartialEq, Eq)]
    #[serde(deny_unknown_fields)]
    pub struct ExprList(pub Vec<Expr>);
    impl ExprList {
        pub fn is_empty(&self) -> bool {
            self.0.is_empty()
        }
    }
}

/// An SQL query
#[derive(Deserialize, Serialize, PartialEq, Eq, Debug)]
#[serde(deny_unknown_fields, rename_all = "lowercase")]
pub enum Query {
    Select(select::Query),
}

/// `FROM` statements
pub mod from {
    use super::*;
    #[derive(Deserialize, Serialize, PartialEq, Eq, Debug)]
    #[serde(deny_unknown_fields, untagged)]
    pub enum From {
        Table(String),
        AliasedTable {
            table: String,
            #[serde(rename = "as")]
            alias: String,
        },
        Subquery {
            #[serde(flatten)]
            query: Box<select::Query>,
            #[serde(rename = "as")]
            alias: Option<String>,
        },
    }
    impl From {
        pub fn with_alias(self, alias: Option<String>) -> Self {
            let Some(alias) = alias else {
                return self;
            };
            match self {
                Self::Table(table) => Self::AliasedTable { table, alias },
                Self::AliasedTable { table, .. } => Self::AliasedTable { table, alias },
                Self::Subquery { query, .. } => Self::Subquery {
                    query,
                    alias: Some(alias),
                },
            }
        }
    }
}

/// `JOIN` statements
pub mod join {
    use super::*;
    #[derive(Deserialize, Serialize, PartialEq, Eq, Debug)]
    #[serde(deny_unknown_fields)]
    pub struct Join {
        pub from: from::From,
        #[serde(flatten)]
        pub on: On,
    }
    #[serde_with::serde_as]
    #[derive(Deserialize, Serialize, PartialEq, Eq, Debug)]
    #[serde(deny_unknown_fields)]
    pub enum On {
        #[serde(rename = "on")]
        On(ExprList),
        #[serde(rename = "using")]
        Using(Vec<String>),
    }
    impl On {
        pub fn is_empty(&self) -> bool {
            match self {
                On::On(exprs) => exprs.is_empty(),
                On::Using(cols) => cols.is_empty(),
            }
        }
    }
}

/// `ORDER BY` statements
pub mod order_by {
    use super::*;

    #[derive(Deserialize, Serialize, Debug, PartialEq, Eq)]
    #[serde(deny_unknown_fields, untagged)]
    pub enum Expr {
        Asc(super::Expr),
        Ordering { expr: super::Expr, order: Ordering },
    }
    impl Expr {
        pub fn new(expr: super::Expr, order: Ordering) -> Self {
            if order == Ordering::Asc {
                Self::Asc(expr)
            } else {
                Self::Ordering { expr, order }
            }
        }
    }
    #[derive(Deserialize, Serialize, Debug, Default, PartialEq, Eq)]
    pub enum Ordering {
        #[default]
        #[serde(rename = "asc")]
        Asc,
        #[serde(rename = "desc")]
        Desc,
    }
}

/// `SELECT` queries
pub mod select {
    use super::*;
    #[derive(Deserialize, Serialize, Debug, Default, PartialEq, Eq)]
    #[serde(deny_unknown_fields)]
    pub struct Query {
        #[serde(skip_serializing_if = "Option::is_none")]
        pub fields: Option<ExprList>,
        pub from: Option<from::From>,
        #[serde(rename = "where")]
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub where_: Option<Expr>,
        #[serde(rename = "groupBy")]
        #[serde(default, skip_serializing_if = "ExprList::is_empty")]
        pub group_by: ExprList,
        #[serde(default)]
        #[serde(skip_serializing_if = "Vec::is_empty")]
        pub joins: Vec<join::Join>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub having: Option<Expr>,
        #[serde(default, rename = "orderBy", skip_serializing_if = "Vec::is_empty")]
        pub order_by: Vec<order_by::Expr>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub limit: Option<usize>,
        #[serde(rename = "limitBy", skip_serializing_if = "Option::is_none")]
        // TODO: Error if `limit` is not set
        // TODO: Support in from_sql
        pub limit_by: Option<ExprList>,
        #[serde(default, skip_serializing_if = "ExprList::is_empty")]
        pub settings: ExprList,
    }
}
