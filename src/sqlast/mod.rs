// Copyright 2018 Grove Enterprises LLC
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! SQL Abstract Syntax Tree (AST) types

mod ddl;
mod query;
mod sql_operator;
mod sqltype;
mod value;

use std::ops::Deref;

pub use self::ddl::{AlterTableOperation, TableConstraint};
pub use self::query::{
    Cte, Fetch, Join, JoinConstraint, JoinOperator, SQLOrderByExpr, SQLQuery, SQLSelect,
    SQLSelectItem, SQLSetExpr, SQLSetOperator, SQLValues, TableAlias, TableFactor,
};
pub use self::sqltype::SQLType;
pub use self::value::Value;

pub use self::sql_operator::SQLOperator;

/// Like `vec.join(", ")`, but for any types implementing ToString.
fn comma_separated_string<I>(iter: I) -> String
where
    I: IntoIterator,
    I::Item: Deref,
    <I::Item as Deref>::Target: ToString,
{
    iter.into_iter()
        .map(|t| t.deref().to_string())
        .collect::<Vec<String>>()
        .join(", ")
}

/// Identifier name, in the originally quoted form (e.g. `"id"`)
pub type SQLIdent = String;

/// An SQL expression of any type.
///
/// The parser does not distinguish between expressions of different types
/// (e.g. boolean vs string), so the caller must handle expressions of
/// inappropriate type, like `WHERE 1` or `SELECT 1=1`, as necessary.
#[derive(Debug, Clone, PartialEq, Hash)]
pub enum ASTNode {
    /// Identifier e.g. table name or column name
    SQLIdentifier(SQLIdent),
    /// Unqualified wildcard (`*`). SQL allows this in limited contexts (such as right
    /// after `SELECT` or as part of an aggregate function, e.g. `COUNT(*)`, but we
    /// currently accept it in contexts where it doesn't make sense, such as `* + *`
    SQLWildcard,
    /// Qualified wildcard, e.g. `alias.*` or `schema.table.*`.
    /// (Same caveats apply to SQLQualifiedWildcard as to SQLWildcard.)
    SQLQualifiedWildcard(Vec<SQLIdent>),
    /// Multi-part identifier, e.g. `table_alias.column` or `schema.table.col`
    SQLCompoundIdentifier(Vec<SQLIdent>),
    /// `IS NULL` expression
    SQLIsNull(Box<ASTNode>),
    /// `IS NOT NULL` expression
    SQLIsNotNull(Box<ASTNode>),
    /// `[ NOT ] IN (val1, val2, ...)`
    SQLInList {
        expr: Box<ASTNode>,
        list: Vec<ASTNode>,
        negated: bool,
    },
    /// `[ NOT ] IN (SELECT ...)`
    SQLInSubquery {
        expr: Box<ASTNode>,
        subquery: Box<SQLQuery>,
        negated: bool,
    },
    /// `<expr> [ NOT ] BETWEEN <low> AND <high>`
    SQLBetween {
        expr: Box<ASTNode>,
        negated: bool,
        low: Box<ASTNode>,
        high: Box<ASTNode>,
    },
    /// Binary expression e.g. `1 + 1` or `foo > bar`
    SQLBinaryExpr {
        left: Box<ASTNode>,
        op: SQLOperator,
        right: Box<ASTNode>,
    },
    /// CAST an expression to a different data type e.g. `CAST(foo AS VARCHAR(123))`
    SQLCast {
        expr: Box<ASTNode>,
        data_type: SQLType,
    },
    SQLExtract {
        field: SQLDateTimeField,
        expr: Box<ASTNode>,
    },
    /// `expr COLLATE collation`
    SQLCollate {
        expr: Box<ASTNode>,
        collation: SQLObjectName,
    },
    /// Nested expression e.g. `(foo > bar)` or `(1)`
    SQLNested(Box<ASTNode>),
    /// Unary expression
    SQLUnary {
        operator: SQLOperator,
        expr: Box<ASTNode>,
    },
    /// SQLValue
    SQLValue(Value),
    /// Scalar function call e.g. `LEFT(foo, 5)`
    SQLFunction(SQLFunction),
    /// CASE [<operand>] WHEN <condition> THEN <result> ... [ELSE <result>] END
    /// Note we only recognize a complete single expression as <condition>, not
    /// `< 0` nor `1, 2, 3` as allowed in a <simple when clause> per
    /// https://jakewheat.github.io/sql-overview/sql-2011-foundation-grammar.html#simple-when-clause
    SQLCase {
        operand: Option<Box<ASTNode>>,
        conditions: Vec<ASTNode>,
        results: Vec<ASTNode>,
        else_result: Option<Box<ASTNode>>,
    },
    /// An exists expression `EXISTS(SELECT ...)`, used in expressions like
    /// `WHERE EXISTS (SELECT ...)`.
    SQLExists(Box<SQLQuery>),
    /// A parenthesized subquery `(SELECT ...)`, used in expression like
    /// `SELECT (subquery) AS x` or `WHERE (subquery) = x`
    SQLSubquery(Box<SQLQuery>),
}

impl ToString for ASTNode {
    fn to_string(&self) -> String {
        match self {
            ASTNode::SQLIdentifier(s) => s.to_string(),
            ASTNode::SQLWildcard => "*".to_string(),
            ASTNode::SQLQualifiedWildcard(q) => q.join(".") + ".*",
            ASTNode::SQLCompoundIdentifier(s) => s.join("."),
            ASTNode::SQLIsNull(ast) => format!("{} IS NULL", ast.as_ref().to_string()),
            ASTNode::SQLIsNotNull(ast) => format!("{} IS NOT NULL", ast.as_ref().to_string()),
            ASTNode::SQLInList {
                expr,
                list,
                negated,
            } => format!(
                "{} {}IN ({})",
                expr.as_ref().to_string(),
                if *negated { "NOT " } else { "" },
                comma_separated_string(list)
            ),
            ASTNode::SQLInSubquery {
                expr,
                subquery,
                negated,
            } => format!(
                "{} {}IN ({})",
                expr.as_ref().to_string(),
                if *negated { "NOT " } else { "" },
                subquery.to_string()
            ),
            ASTNode::SQLBetween {
                expr,
                negated,
                low,
                high,
            } => format!(
                "{} {}BETWEEN {} AND {}",
                expr.to_string(),
                if *negated { "NOT " } else { "" },
                low.to_string(),
                high.to_string()
            ),
            ASTNode::SQLBinaryExpr { left, op, right } => format!(
                "{} {} {}",
                left.as_ref().to_string(),
                op.to_string(),
                right.as_ref().to_string()
            ),
            ASTNode::SQLCast { expr, data_type } => format!(
                "CAST({} AS {})",
                expr.as_ref().to_string(),
                data_type.to_string()
            ),
            ASTNode::SQLExtract { field, expr } => {
                format!("EXTRACT({} FROM {})", field.to_string(), expr.to_string())
            }
            ASTNode::SQLCollate { expr, collation } => format!(
                "{} COLLATE {}",
                expr.as_ref().to_string(),
                collation.to_string()
            ),
            ASTNode::SQLNested(ast) => format!("({})", ast.as_ref().to_string()),
            ASTNode::SQLUnary { operator, expr } => {
                format!("{} {}", operator.to_string(), expr.as_ref().to_string())
            }
            ASTNode::SQLValue(v) => v.to_string(),
            ASTNode::SQLFunction(f) => f.to_string(),
            ASTNode::SQLCase {
                operand,
                conditions,
                results,
                else_result,
            } => {
                let mut s = "CASE".to_string();
                if let Some(operand) = operand {
                    s += &format!(" {}", operand.to_string());
                }
                s += &conditions
                    .iter()
                    .zip(results)
                    .map(|(c, r)| format!(" WHEN {} THEN {}", c.to_string(), r.to_string()))
                    .collect::<Vec<String>>()
                    .join("");
                if let Some(else_result) = else_result {
                    s += &format!(" ELSE {}", else_result.to_string())
                }
                s + " END"
            }
            ASTNode::SQLExists(s) => format!("EXISTS ({})", s.to_string()),
            ASTNode::SQLSubquery(s) => format!("({})", s.to_string()),
        }
    }
}

/// A window specification (i.e. `OVER (PARTITION BY .. ORDER BY .. etc.)`)
#[derive(Debug, Clone, PartialEq, Hash)]
pub struct SQLWindowSpec {
    pub partition_by: Vec<ASTNode>,
    pub order_by: Vec<SQLOrderByExpr>,
    pub window_frame: Option<SQLWindowFrame>,
}

impl ToString for SQLWindowSpec {
    fn to_string(&self) -> String {
        let mut clauses = vec![];
        if !self.partition_by.is_empty() {
            clauses.push(format!(
                "PARTITION BY {}",
                comma_separated_string(&self.partition_by)
            ))
        };
        if !self.order_by.is_empty() {
            clauses.push(format!(
                "ORDER BY {}",
                comma_separated_string(&self.order_by)
            ))
        };
        if let Some(window_frame) = &self.window_frame {
            if let Some(end_bound) = &window_frame.end_bound {
                clauses.push(format!(
                    "{} BETWEEN {} AND {}",
                    window_frame.units.to_string(),
                    window_frame.start_bound.to_string(),
                    end_bound.to_string()
                ));
            } else {
                clauses.push(format!(
                    "{} {}",
                    window_frame.units.to_string(),
                    window_frame.start_bound.to_string()
                ));
            }
        }
        clauses.join(" ")
    }
}

/// Specifies the data processed by a window function, e.g.
/// `RANGE UNBOUNDED PRECEDING` or `ROWS BETWEEN 5 PRECEDING AND CURRENT ROW`.
#[derive(Debug, Clone, PartialEq, Hash)]
pub struct SQLWindowFrame {
    pub units: SQLWindowFrameUnits,
    pub start_bound: SQLWindowFrameBound,
    /// The right bound of the `BETWEEN .. AND` clause.
    pub end_bound: Option<SQLWindowFrameBound>,
    // TBD: EXCLUDE
}

#[derive(Debug, Clone, PartialEq, Hash)]
pub enum SQLWindowFrameUnits {
    Rows,
    Range,
    Groups,
}

impl ToString for SQLWindowFrameUnits {
    fn to_string(&self) -> String {
        match self {
            SQLWindowFrameUnits::Rows => "ROWS".to_string(),
            SQLWindowFrameUnits::Range => "RANGE".to_string(),
            SQLWindowFrameUnits::Groups => "GROUPS".to_string(),
        }
    }
}

impl FromStr for SQLWindowFrameUnits {
    type Err = ParserError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "ROWS" => Ok(SQLWindowFrameUnits::Rows),
            "RANGE" => Ok(SQLWindowFrameUnits::Range),
            "GROUPS" => Ok(SQLWindowFrameUnits::Groups),
            _ => Err(ParserError::ParserError(format!(
                "Expected ROWS, RANGE, or GROUPS, found: {}",
                s
            ))),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Hash)]
pub enum SQLWindowFrameBound {
    /// "CURRENT ROW"
    CurrentRow,
    /// "<N> PRECEDING" or "UNBOUNDED PRECEDING"
    Preceding(Option<u64>),
    /// "<N> FOLLOWING" or "UNBOUNDED FOLLOWING". This can only appear in
    /// SQLWindowFrame::end_bound.
    Following(Option<u64>),
}

impl ToString for SQLWindowFrameBound {
    fn to_string(&self) -> String {
        match self {
            SQLWindowFrameBound::CurrentRow => "CURRENT ROW".to_string(),
            SQLWindowFrameBound::Preceding(None) => "UNBOUNDED PRECEDING".to_string(),
            SQLWindowFrameBound::Following(None) => "UNBOUNDED FOLLOWING".to_string(),
            SQLWindowFrameBound::Preceding(Some(n)) => format!("{} PRECEDING", n),
            SQLWindowFrameBound::Following(Some(n)) => format!("{} FOLLOWING", n),
        }
    }
}

/// A top-level statement (SELECT, INSERT, CREATE, etc.)
#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone, PartialEq, Hash)]
pub enum SQLStatement {
    /// SELECT
    SQLQuery(Box<SQLQuery>),
    /// INSERT
    SQLInsert {
        /// TABLE
        table_name: SQLObjectName,
        /// COLUMNS
        columns: Vec<SQLIdent>,
        /// A SQL query that specifies what to insert
        source: Box<SQLQuery>,
    },
    SQLCopy {
        /// TABLE
        table_name: SQLObjectName,
        /// COLUMNS
        columns: Vec<SQLIdent>,
        /// VALUES a vector of values to be copied
        values: Vec<Option<String>>,
    },
    /// UPDATE
    SQLUpdate {
        /// TABLE
        table_name: SQLObjectName,
        /// Column assignments
        assignments: Vec<SQLAssignment>,
        /// WHERE
        selection: Option<ASTNode>,
    },
    /// DELETE
    SQLDelete {
        /// FROM
        table_name: SQLObjectName,
        /// WHERE
        selection: Option<ASTNode>,
    },
    /// CREATE VIEW
    SQLCreateView {
        /// View name
        name: SQLObjectName,
        columns: Vec<SQLIdent>,
        query: Box<SQLQuery>,
        materialized: bool,
        with_options: Vec<SQLOption>,
    },
    /// CREATE TABLE
    SQLCreateTable {
        /// Table name
        name: SQLObjectName,
        /// Optional schema
        columns: Vec<SQLColumnDef>,
        constraints: Vec<TableConstraint>,
        with_options: Vec<SQLOption>,
        external: bool,
        file_format: Option<FileFormat>,
        location: Option<String>,
    },
    /// ALTER TABLE
    SQLAlterTable {
        /// Table name
        name: SQLObjectName,
        operation: AlterTableOperation,
    },
    /// DROP TABLE
    SQLDrop {
        object_type: SQLObjectType,
        if_exists: bool,
        names: Vec<SQLObjectName>,
        cascade: bool,
    },
    SQLTransaction(Vec<Box<SQLStatement>>),
}

impl ToString for SQLStatement {
    fn to_string(&self) -> String {
        match self {
            SQLStatement::SQLQuery(s) => s.to_string(),
            SQLStatement::SQLInsert {
                table_name,
                columns,
                source,
            } => {
                let mut s = format!("INSERT INTO {} ", table_name.to_string());
                if !columns.is_empty() {
                    s += &format!("({}) ", columns.join(", "));
                }
                s += &source.to_string();
                s
            }
            SQLStatement::SQLCopy {
                table_name,
                columns,
                values,
            } => {
                let mut s = format!("COPY {}", table_name.to_string());
                if !columns.is_empty() {
                    s += &format!(" ({})", comma_separated_string(columns));
                }
                s += " FROM stdin; ";
                if !values.is_empty() {
                    s += &format!(
                        "\n{}",
                        values
                            .iter()
                            .map(|v| v.clone().unwrap_or_else(|| "\\N".to_string()))
                            .collect::<Vec<String>>()
                            .join("\t")
                    );
                }
                s += "\n\\.";
                s
            }
            SQLStatement::SQLUpdate {
                table_name,
                assignments,
                selection,
            } => {
                let mut s = format!("UPDATE {}", table_name.to_string());
                if !assignments.is_empty() {
                    s += " SET ";
                    s += &comma_separated_string(assignments);
                }
                if let Some(selection) = selection {
                    s += &format!(" WHERE {}", selection.to_string());
                }
                s
            }
            SQLStatement::SQLDelete {
                table_name,
                selection,
            } => {
                let mut s = format!("DELETE FROM {}", table_name.to_string());
                if let Some(selection) = selection {
                    s += &format!(" WHERE {}", selection.to_string());
                }
                s
            }
            SQLStatement::SQLCreateView {
                name,
                columns,
                query,
                materialized,
                with_options,
            } => {
                let modifier = if *materialized { " MATERIALIZED" } else { "" };
                let with_options = if !with_options.is_empty() {
                    format!(" WITH ({})", comma_separated_string(with_options))
                } else {
                    "".into()
                };
                let columns = if !columns.is_empty() {
                    format!(" ({})", comma_separated_string(columns))
                } else {
                    "".into()
                };
                format!(
                    "CREATE{} VIEW {}{}{} AS {}",
                    modifier,
                    name.to_string(),
                    with_options,
                    columns,
                    query.to_string(),
                )
            }
            SQLStatement::SQLCreateTable {
                name,
                columns,
                constraints,
                with_options,
                external,
                file_format,
                location,
            } => {
                let mut s = format!(
                    "CREATE {}TABLE {} ({}",
                    if *external { "EXTERNAL " } else { "" },
                    name.to_string(),
                    comma_separated_string(columns)
                );
                if !constraints.is_empty() {
                    s += &format!(", {}", comma_separated_string(constraints));
                }
                s += ")";
                if *external {
                    s += &format!(
                        " STORED AS {} LOCATION '{}'",
                        file_format.as_ref().unwrap().to_string(),
                        location.as_ref().unwrap()
                    );
                }
                if !with_options.is_empty() {
                    s += &format!(" WITH ({})", comma_separated_string(with_options));
                }
                s
            }
            SQLStatement::SQLAlterTable { name, operation } => {
                format!("ALTER TABLE {} {}", name.to_string(), operation.to_string())
            }
            SQLStatement::SQLDrop {
                object_type,
                if_exists,
                names,
                cascade,
            } => format!(
                "DROP {}{} {}{}",
                object_type.to_string(),
                if *if_exists { " IF EXISTS" } else { "" },
                comma_separated_string(names),
                if *cascade { " CASCADE" } else { "" },
            ),
            SQLStatement::SQLTransaction(stmts) => format!(
                "BEGIN;\n{}\nCOMMIT;",
                stmts.iter().map(|s| s.to_string()).collect::<Vec<String>>().join("\n"),
            ),
        }
    }
}

/// A name of a table, view, custom type, etc., possibly multi-part, i.e. db.schema.obj
#[derive(Debug, Clone, PartialEq, Hash)]
pub struct SQLObjectName(pub Vec<SQLIdent>);

impl ToString for SQLObjectName {
    fn to_string(&self) -> String {
        self.0.join(".")
    }
}

/// SQL assignment `foo = expr` as used in SQLUpdate
#[derive(Debug, Clone, PartialEq, Hash)]
pub struct SQLAssignment {
    pub id: SQLIdent,
    pub value: ASTNode,
}

impl ToString for SQLAssignment {
    fn to_string(&self) -> String {
        format!("{} = {}", self.id, self.value.to_string())
    }
}

/// SQL column definition
#[derive(Debug, Clone, PartialEq, Hash)]
pub struct SQLColumnDef {
    pub name: SQLIdent,
    pub data_type: SQLType,
    pub is_primary: bool,
    pub is_unique: bool,
    pub default: Option<ASTNode>,
    pub allow_null: bool,
}

impl ToString for SQLColumnDef {
    fn to_string(&self) -> String {
        let mut s = format!("{} {}", self.name, self.data_type.to_string());
        if self.is_primary {
            s += " PRIMARY KEY";
        }
        if self.is_unique {
            s += " UNIQUE";
        }
        if let Some(ref default) = self.default {
            s += &format!(" DEFAULT {}", default.to_string());
        }
        if !self.allow_null {
            s += " NOT NULL";
        }
        s
    }
}

/// SQL function
#[derive(Debug, Clone, PartialEq, Hash)]
pub struct SQLFunction {
    pub name: SQLObjectName,
    pub args: Vec<ASTNode>,
    pub over: Option<SQLWindowSpec>,
    // aggregate functions may specify eg `COUNT(DISTINCT x)`
    pub distinct: bool,
}

impl ToString for SQLFunction {
    fn to_string(&self) -> String {
        let mut s = format!(
            "{}({}{})",
            self.name.to_string(),
            if self.distinct { "DISTINCT " } else { "" },
            comma_separated_string(&self.args),
        );
        if let Some(o) = &self.over {
            s += &format!(" OVER ({})", o.to_string())
        }
        s
    }
}

#[derive(Debug, Clone, PartialEq, Hash)]
pub enum SQLDateTimeField {
    Year,
    Month,
    Day,
    Hour,
    Minute,
    Second,
}

impl ToString for SQLDateTimeField {
    fn to_string(&self) -> String {
        match self {
            SQLDateTimeField::Year => "YEAR".to_string(),
            SQLDateTimeField::Month => "MONTH".to_string(),
            SQLDateTimeField::Day => "DAY".to_string(),
            SQLDateTimeField::Hour => "HOUR".to_string(),
            SQLDateTimeField::Minute => "MINUTE".to_string(),
            SQLDateTimeField::Second => "SECOND".to_string(),
        }
    }
}

/// External table's available file format
#[derive(Debug, Clone, PartialEq, Hash)]
pub enum FileFormat {
    TEXTFILE,
    SEQUENCEFILE,
    ORC,
    PARQUET,
    AVRO,
    RCFILE,
    JSONFILE,
}

impl ToString for FileFormat {
    fn to_string(&self) -> String {
        use self::FileFormat::*;
        match self {
            TEXTFILE => "TEXTFILE".to_string(),
            SEQUENCEFILE => "SEQUENCEFILE".to_string(),
            ORC => "ORC".to_string(),
            PARQUET => "PARQUET".to_string(),
            AVRO => "AVRO".to_string(),
            RCFILE => "RCFILE".to_string(),
            JSONFILE => "TEXTFILE".to_string(),
        }
    }
}

use crate::sqlparser::ParserError;
use std::str::FromStr;
impl FromStr for FileFormat {
    type Err = ParserError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        use self::FileFormat::*;
        match s {
            "TEXTFILE" => Ok(TEXTFILE),
            "SEQUENCEFILE" => Ok(SEQUENCEFILE),
            "ORC" => Ok(ORC),
            "PARQUET" => Ok(PARQUET),
            "AVRO" => Ok(AVRO),
            "RCFILE" => Ok(RCFILE),
            "JSONFILE" => Ok(JSONFILE),
            _ => Err(ParserError::ParserError(format!(
                "Unexpected file format: {}",
                s
            ))),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Hash)]
pub enum SQLObjectType {
    Table,
    View,
}

impl SQLObjectType {
    fn to_string(&self) -> String {
        match self {
            SQLObjectType::Table => "TABLE".into(),
            SQLObjectType::View => "VIEW".into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Hash)]
pub struct SQLOption {
    pub name: SQLIdent,
    pub value: Value,
}

impl ToString for SQLOption {
    fn to_string(&self) -> String {
        format!("{} = {}", self.name.to_string(), self.value.to_string())
    }
}
