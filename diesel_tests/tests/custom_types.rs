use crate::schema::*;
use diesel::connection::SimpleConnection;
use diesel::deserialize::{self, FromSql};
use diesel::pg::{Pg, PgValue};
use diesel::serialize::{self, IsNull, Output, ToSql};
use diesel::*;
use std::io::Write;

table! {
    use diesel::sql_types::*;
    use super::MyType;
    custom_types {
        id -> Integer,
        custom_enum -> MyType,
    }
}

#[derive(SqlType)]
#[postgres(type_name = "my_type")]
pub struct MyType;

#[derive(Debug, PartialEq, FromSqlRow, AsExpression)]
#[sql_type = "MyType"]
pub enum MyEnum {
    Foo,
    Bar,
}

impl ToSql<MyType, Pg> for MyEnum {
    fn to_sql<W: Write>(&self, out: &mut Output<W, Pg>) -> serialize::Result {
        match *self {
            MyEnum::Foo => out.write_all(b"foo")?,
            MyEnum::Bar => out.write_all(b"bar")?,
        }
        Ok(IsNull::No)
    }
}

impl FromSql<MyType, Pg> for MyEnum {
    fn from_sql(bytes: PgValue<'_>) -> deserialize::Result<Self> {
        match bytes.as_bytes() {
            b"foo" => Ok(MyEnum::Foo),
            b"bar" => Ok(MyEnum::Bar),
            _ => Err("Unrecognized enum variant".into()),
        }
    }
}

#[derive(Insertable, Queryable, Identifiable, Debug, PartialEq)]
#[table_name = "custom_types"]
struct HasCustomTypes {
    id: i32,
    custom_enum: MyEnum,
}

#[test]
fn custom_types_round_trip() {
    let data = vec![
        HasCustomTypes {
            id: 1,
            custom_enum: MyEnum::Foo,
        },
        HasCustomTypes {
            id: 2,
            custom_enum: MyEnum::Bar,
        },
    ];
    let connection = &mut connection();
    connection
        .batch_execute(
            r#"
        CREATE TYPE my_type AS ENUM ('foo', 'bar');
        CREATE TABLE custom_types (
            id SERIAL PRIMARY KEY,
            custom_enum my_type NOT NULL
        );
    "#,
        )
        .unwrap();

    let inserted = insert_into(custom_types::table)
        .values(&data)
        .get_results(connection)
        .unwrap();
    assert_eq!(data, inserted);
}

table! {
    use diesel::sql_types::*;
    use super::MyTypeInCustomSchema;
    custom_schema.custom_types_with_custom_schema {
        id -> Integer,
        custom_enum -> MyTypeInCustomSchema,
    }
}

#[derive(SqlType)]
#[postgres(type_name = "my_type", type_schema = "custom_schema")]
pub struct MyTypeInCustomSchema;

#[derive(Debug, PartialEq, FromSqlRow, AsExpression)]
#[sql_type = "MyTypeInCustomSchema"]
pub enum MyEnumInCustomSchema {
    Foo,
    Bar,
}

impl ToSql<MyTypeInCustomSchema, Pg> for MyEnumInCustomSchema {
    fn to_sql<W: Write>(&self, out: &mut Output<W, Pg>) -> serialize::Result {
        match *self {
            MyEnumInCustomSchema::Foo => out.write_all(b"foo")?,
            MyEnumInCustomSchema::Bar => out.write_all(b"bar")?,
        }
        Ok(IsNull::No)
    }
}

impl FromSql<MyTypeInCustomSchema, Pg> for MyEnumInCustomSchema {
    fn from_sql(bytes: PgValue<'_>) -> deserialize::Result<Self> {
        match bytes.as_bytes() {
            b"foo" => Ok(MyEnumInCustomSchema::Foo),
            b"bar" => Ok(MyEnumInCustomSchema::Bar),
            _ => Err("Unrecognized enum variant".into()),
        }
    }
}

#[derive(Insertable, Queryable, Identifiable, Debug, PartialEq)]
#[table_name = "custom_types_with_custom_schema"]
struct HasCustomTypesInCustomSchema {
    id: i32,
    custom_enum: MyEnumInCustomSchema,
}

#[test]
fn custom_types_in_custom_schema_round_trip() {
    let data = vec![
        HasCustomTypesInCustomSchema {
            id: 1,
            custom_enum: MyEnumInCustomSchema::Foo,
        },
        HasCustomTypesInCustomSchema {
            id: 2,
            custom_enum: MyEnumInCustomSchema::Bar,
        },
    ];
    let connection = &mut connection();
    connection
        .batch_execute(
            r#"
        CREATE SCHEMA IF NOT EXISTS custom_schema;
        CREATE TYPE custom_schema.my_type AS ENUM ('foo', 'bar');
        CREATE TABLE custom_schema.custom_types_with_custom_schema (
            id SERIAL PRIMARY KEY,
            custom_enum custom_schema.my_type NOT NULL
        );
    "#,
        )
        .unwrap();

    let inserted = insert_into(custom_types_with_custom_schema::table)
        .values(&data)
        .get_results(connection)
        .unwrap();
    assert_eq!(data, inserted);
}

#[derive(SqlType)]
#[postgres(type_name = "ty", type_schema = "other")]
struct OtherTy;

#[derive(SqlType)]
#[postgres(type_name = "ty", type_schema = "public")]
struct PublicTy;

#[derive(SqlType)]
#[postgres(type_name = "ty")]
struct InferedTy;

#[test]
fn custom_type_schema_inference() {
    use diesel::sql_types::HasSqlType;

    let conn = &mut connection();
    conn.batch_execute(
        "
        CREATE SCHEMA IF NOT EXISTS other;
        -- Clear leftovers from the previous execution
        DROP TABLE IF EXISTS other.foo;
        DROP TYPE IF EXISTS public.ty CASCADE;
        DROP TYPE IF EXISTS other.ty CASCADE;
        -- Create types on *both* schemas
        CREATE TYPE public.ty AS (field int);
        CREATE TYPE other.ty AS (field int);
        -- Create a table on the other schema referencing the created type
        CREATE TABLE other.foo (bar other.ty PRIMARY KEY);
        -- Include both in the search path
        SET search_path TO other, public;
        ",
    )
    .unwrap();

    let other_ty = <Pg as HasSqlType<OtherTy>>::metadata(conn);
    let public_ty = <Pg as HasSqlType<PublicTy>>::metadata(conn);
    let infered_ty = <Pg as HasSqlType<InferedTy>>::metadata(conn);
    let _ = dbg!(other_ty.oid());
    let _ = dbg!(public_ty.oid());
    let _ = dbg!(infered_ty.oid());

    assert_eq!(other_ty.oid().unwrap(), infered_ty.oid().unwrap());
    assert_ne!(public_ty.oid().unwrap(), other_ty.oid().unwrap());
}
