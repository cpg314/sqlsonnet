#![allow(unstable_name_collisions)]
use itertools::Itertools;
use serde_with::OneOrMany;

use serde::{Deserialize, Serialize};

/// A set of SQL queries
#[serde_with::serde_as]
#[derive(Deserialize, Serialize, Debug, Default, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Queries(#[serde_as(as = "OneOrMany<_>")] Vec<Query>);
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

pub use expr::*;
pub mod expr {
    use super::*;
    #[derive(Eq, PartialEq, Debug, Deserialize, Serialize)]
    pub struct Operator(pub String);

    #[serde_with::serde_as]
    #[derive(Deserialize, Serialize, Debug, PartialEq, Eq)]
    #[serde(deny_unknown_fields, untagged)]
    pub enum Expr {
        Raw(String),
        // [expr, op, [expr]]
        Operator(Box<Expr>, Operator, Box<Expr>),
        // [expr, [[op, expr], ...]]
        OperatorSeq(Box<Expr>, Vec<(Operator, Box<Expr>)>),
        Subquery(Box<Query>),
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
        pub fn operator(self, op: Operator, right: Expr) -> Self {
            Self::Operator(Box::new(self), op, Box::new(right))
        }
    }
    impl Default for Expr {
        fn default() -> Self {
            Self::Raw(Default::default())
        }
    }

    #[serde_with::serde_as]
    #[derive(Deserialize, Serialize, Debug, Default, PartialEq, Eq)]
    #[serde(deny_unknown_fields)]
    pub struct ExprList(#[serde_as(as = "OneOrMany<_>")] pub Vec<Expr>);
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

pub mod from {
    use super::*;
    #[derive(Deserialize, Serialize, Default, PartialEq, Eq, Debug)]
    #[serde(deny_unknown_fields, untagged)]
    pub enum From {
        Table(String),
        AliasedTable {
            table: String,
            #[serde(rename = "as")]
            alias: String,
        },
        Subquery(Box<select::Query>),
        AliasedSubquery {
            #[serde(flatten)]
            query: Box<select::Query>,
            #[serde(rename = "as")]
            alias: String,
        },
        #[default]
        Unset,
    }
    impl From {
        pub fn with_alias(self, alias: Option<String>) -> Self {
            let Some(alias) = alias else {
                return self;
            };
            match self {
                Self::Table(table) => Self::AliasedTable { table, alias },
                Self::AliasedTable { table, .. } => Self::AliasedTable { table, alias },
                Self::AliasedSubquery { query, .. } => Self::AliasedSubquery { query, alias },
                Self::Subquery(query) => Self::AliasedSubquery { query, alias },
                Self::Unset => From::Unset,
            }
        }
    }
}

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
        Using(#[serde_as(as = "OneOrMany<_>")] Vec<String>),
    }
}

pub mod order_by {
    use super::*;

    #[derive(Deserialize, Serialize, Debug, PartialEq, Eq)]
    #[serde(deny_unknown_fields, untagged)]
    pub enum Expr {
        Asc(super::Expr),
        Ordering(super::Expr, Ordering),
    }
    impl Expr {
        pub fn new(e: super::Expr, ordering: Ordering) -> Self {
            if ordering == Ordering::Asc {
                Self::Asc(e)
            } else {
                Self::Ordering(e, ordering)
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

pub mod select {
    use super::*;
    #[serde_with::serde_as]
    #[derive(Deserialize, Serialize, Debug, Default, PartialEq, Eq)]
    #[serde(deny_unknown_fields)]
    pub struct Query {
        #[serde(skip_serializing_if = "ExprList::is_empty")]
        pub fields: ExprList,
        pub from: from::From,
        #[serde(rename = "where")]
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub where_: Option<Expr>,
        #[serde(rename = "groupBy")]
        #[serde(default, skip_serializing_if = "ExprList::is_empty")]
        pub group_by: ExprList,
        #[serde(default)]
        #[serde(skip_serializing_if = "Vec::is_empty")]
        #[serde_as(as = "OneOrMany<_>")]
        pub joins: Vec<join::Join>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub having: Option<Expr>,
        #[serde(default, rename = "orderBy", skip_serializing_if = "Vec::is_empty")]
        #[serde_as(as = "OneOrMany<_>")]
        pub order_by: Vec<order_by::Expr>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub limit: Option<usize>,
    }
}
