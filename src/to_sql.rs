use crate::queries::*;
use std::fmt::{self, Write};

pub(super) trait ToSql: Sized {
    fn to_sql(&self, f: &mut IndentedPrinter<'_>) -> fmt::Result;
    fn to_sql_str(&self, compact: bool) -> String {
        let mut out = String::new();
        let mut printer = IndentedPrinter::new(&mut out, compact);
        // TODO: Handle error
        ToSql::to_sql(self, &mut printer).unwrap();
        out
    }
}

// Inspired by the `indenter` crate
pub(super) struct IndentedPrinter<'a> {
    indent: usize,
    out: &'a mut String,
    needs_indent: bool,
    compact: bool,
}
impl<'a> IndentedPrinter<'a> {
    fn new(out: &'a mut String, compact: bool) -> Self {
        Self {
            out,
            indent: 0,
            needs_indent: true,
            compact,
        }
    }
    fn indented(&mut self) -> IndentedPrinter<'_> {
        let indent = self.indent + 2;
        IndentedPrinter {
            out: self.out,
            indent,
            needs_indent: true,
            compact: self.compact,
        }
    }
}
impl<'a> fmt::Write for IndentedPrinter<'a> {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        // abc\ndef
        for (i, line) in s.split('\n').enumerate() {
            if i > 0 {
                if self.compact {
                    self.out.push(' ');
                } else {
                    self.out.push('\n');
                }

                self.needs_indent = true;
            }
            if self.needs_indent {
                if line.is_empty() {
                    continue;
                }
                for _ in 0..self.indent {
                    if !self.compact {
                        self.out.push(' ');
                    }
                }
                self.needs_indent = false;
            }
            self.out.push_str(line);
        }
        Ok(())
    }
}

impl ToSql for Queries {
    fn to_sql(&self, f: &mut IndentedPrinter<'_>) -> fmt::Result {
        for q in self {
            ToSql::to_sql(q, f)?;
            write!(f, ";")?;
        }
        Ok(())
    }
}
impl<T: ToSql> ToSql for Vec<T> {
    fn to_sql(&self, f: &mut IndentedPrinter<'_>) -> fmt::Result {
        for (i, x) in self.iter().enumerate() {
            x.to_sql(f)?;
            if i != self.len() - 1 {
                writeln!(f, ",")?;
            }
        }
        Ok(())
    }
}

impl ToSql for String {
    fn to_sql(&self, f: &mut IndentedPrinter<'_>) -> fmt::Result {
        write!(f, "{}", self)
    }
}
impl ToSql for Expr {
    fn to_sql(&self, f: &mut IndentedPrinter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl ToSql for ExprList {
    fn to_sql(&self, f: &mut IndentedPrinter<'_>) -> fmt::Result {
        self.0.to_sql(f)
    }
}
impl ToSql for Query {
    fn to_sql(&self, f: &mut IndentedPrinter<'_>) -> fmt::Result {
        match self {
            Query::Select(s) => s.to_sql(f),
        }
    }
}

impl ToSql for from::From {
    fn to_sql(&self, f: &mut IndentedPrinter<'_>) -> fmt::Result {
        match self {
            Self::Unset => unreachable!(),
            Self::Table(s) => s.to_sql(f),
            Self::TableAlias { name, alias } => write!(f, "{} AS {}", name, alias),
            Self::Subquery(q) => {
                writeln!(f, "(")?;
                q.to_sql(&mut f.indented())?;
                write!(f, ")")?;
                if let Some(as_) = &q.as_ {
                    write!(f, " AS {}", as_)?;
                }
                Ok(())
            }
        }
    }
}

impl ToSql for join::On {
    fn to_sql(&self, f: &mut IndentedPrinter) -> fmt::Result {
        match self {
            Self::On(on) => {
                writeln!(f, "ON")?;
                on.to_sql(&mut f.indented())
            }
            Self::Using(col) => {
                writeln!(f, "USING")?;
                col.to_sql(&mut f.indented())
            }
        }
    }
}
impl ToSql for join::Join {
    fn to_sql(&self, f: &mut IndentedPrinter) -> fmt::Result {
        write!(f, "JOIN ")?;
        self.from.to_sql(f)?;
        writeln!(f)?;
        self.on.to_sql(&mut f.indented())
    }
}

impl ToSql for order_by::Ordering {
    fn to_sql(&self, f: &mut IndentedPrinter) -> fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Self::Asc => "ASC",
                Self::Desc => "DESC",
            }
        )
    }
}
impl ToSql for order_by::Expr {
    fn to_sql(&self, f: &mut IndentedPrinter) -> fmt::Result {
        self.0.to_sql(f)?;
        write!(f, " ")?;
        self.1.to_sql(f)
    }
}

impl ToSql for select::Query {
    fn to_sql(&self, f: &mut IndentedPrinter) -> fmt::Result {
        writeln!(f, "SELECT")?;
        self.fields.to_sql(&mut f.indented())?;
        writeln!(f)?;
        write!(f, "FROM ")?;
        self.from.to_sql(f)?;
        for join in &self.joins {
            writeln!(f)?;
            join.to_sql(f)?;
        }
        if let Some(where_) = &self.where_ {
            writeln!(f, "\nWHERE")?;
            where_.to_sql(&mut f.indented())?;
        }
        if !self.group_by.is_empty() {
            writeln!(f, "\nGROUP BY")?;
            self.group_by.to_sql(&mut f.indented())?;
        }
        if let Some(having) = &self.having {
            writeln!(f, "\nHAVING")?;
            having.to_sql(&mut f.indented())?;
        }
        if !self.order_by.is_empty() {
            writeln!(f, "\nORDER BY")?;
            self.order_by.to_sql(&mut f.indented())?;
        }
        if let Some(limit) = &self.limit {
            writeln!(f, "\nLIMIT {}", limit)?;
        }
        Ok(())
    }
}
