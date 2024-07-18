use std::fmt::{self, Write};

use crate::queries::*;

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
        IndentedPrinter {
            out: self.out,
            indent: self.indent + 2,
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
            writeln!(f, ";")?;
        }
        Ok(())
    }
}

fn to_sql_list<T: ToSql>(x: &[T], f: &mut IndentedPrinter<'_>, separator: &str) -> fmt::Result {
    for (i, xx) in x.iter().enumerate() {
        xx.to_sql(f)?;
        if i != x.len() - 1 {
            write!(f, "{}", separator)?;
        }
    }
    Ok(())
}

impl<T: ToSql> ToSql for Vec<T> {
    fn to_sql(&self, f: &mut IndentedPrinter<'_>) -> fmt::Result {
        to_sql_list(self, f, ",\n")
    }
}

impl ToSql for String {
    fn to_sql(&self, f: &mut IndentedPrinter<'_>) -> fmt::Result {
        write!(f, "{}", self)
    }
}

impl ToSql for Operator {
    fn to_sql(&self, f: &mut IndentedPrinter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

fn parenthesized_expr(f: &mut IndentedPrinter<'_>, expr: &Expr) -> fmt::Result {
    if expr.is_raw() {
        return expr.to_sql(f);
    }
    write!(f, "(")?;
    expr.to_sql(f)?;
    write!(f, ")")
}

impl ToSql for Expr {
    fn to_sql(&self, f: &mut IndentedPrinter<'_>) -> fmt::Result {
        match self {
            Expr::Raw(s) => write!(f, "{}", s),
            Expr::RawInteger(s) => write!(f, "{}", s),
            Expr::RawBool(s) => write!(f, "{:?}", s),
            Expr::RawFloat(s) => write!(f, "{}", s),
            Expr::Aliased { expr, alias } => {
                expr.to_sql(f)?;
                write!(f, " AS {}", alias)
            }
            Expr::OperatorSeq(q1, v) => {
                // TODO Use op.linebreak?
                parenthesized_expr(f, q1)?;
                for (op, q) in v {
                    write!(f, " ")?;
                    op.to_sql(f)?;
                    write!(f, " ")?;
                    parenthesized_expr(f, q)?;
                }
                Ok(())
            }
            Expr::Operator(q1, op, q2) => {
                parenthesized_expr(f, q1)?;
                if op.linebreak() {
                    writeln!(f)?;
                } else {
                    write!(f, " ")?;
                }
                op.to_sql(f)?;
                write!(f, " ")?;
                parenthesized_expr(f, q2)
            }
            Expr::Subquery(s) => {
                writeln!(f)?;
                ToSql::to_sql(s.as_ref(), &mut f.indented())
            }
            Expr::FunctionCall {
                r#fn: function,
                params,
            } => {
                write!(f, "{}(", function)?;
                to_sql_list(&params.0, f, ", ")?;
                write!(f, ")")
            }
        }
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
            Self::Table(s) => s.to_sql(f),
            Self::AliasedTable { table, alias } => {
                write!(f, "{} AS {}", table, alias)
            }
            Self::Subquery { query, alias } => {
                writeln!(f, "(")?;
                ToSql::to_sql(query.as_ref(), &mut f.indented())?;
                write!(f, ")")?;
                if let Some(alias) = alias {
                    write!(f, " AS {}", alias)?;
                }
                Ok(())
            }
        }
    }
}

impl ToSql for join::On {
    fn to_sql(&self, f: &mut IndentedPrinter) -> fmt::Result {
        if self.is_empty() {
            return Ok(());
        }
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
        if self.on.is_empty() {
            write!(f, "CROSS JOIN ")?;
        } else {
            write!(f, "JOIN ")?;
        }
        self.from.to_sql(f)?;
        writeln!(f)?;
        self.on.to_sql(&mut f.indented())
    }
}

impl ToSql for order_by::Ordering {
    fn to_sql(&self, f: &mut IndentedPrinter) -> fmt::Result {
        if *self != Self::Asc {
            write!(f, " DESC",)?;
        }
        Ok(())
    }
}
impl ToSql for order_by::Expr {
    fn to_sql(&self, f: &mut IndentedPrinter) -> fmt::Result {
        match self {
            order_by::Expr::Asc(e) => e.to_sql(f),
            order_by::Expr::Ordering { order, expr } => {
                expr.to_sql(f)?;
                order.to_sql(f)?;
                Ok(())
            }
        }
    }
}

impl ToSql for select::Query {
    fn to_sql(&self, f: &mut IndentedPrinter) -> fmt::Result {
        writeln!(f, "SELECT")?;
        if let Some(fields) = &self.fields {
            fields.to_sql(&mut f.indented())?;
        } else {
            ExprList(vec!["*".into()]).to_sql(&mut f.indented())?;
        }

        if let Some(from) = &self.from {
            writeln!(f)?;
            write!(f, "FROM ")?;
            from.to_sql(f)?;
        }
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
            write!(f, "\nLIMIT {}", limit)?;
        }
        if !self.settings.is_empty() {
            write!(f, "\nSETTINGS ")?;
            self.settings.to_sql(f)?;
        }
        if !f.compact {
            writeln!(f)?;
        }
        Ok(())
    }
}
