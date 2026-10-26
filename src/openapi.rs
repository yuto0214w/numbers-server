use serde_json::Value;
use utoipa::{
    OpenApi, PartialSchema, ToSchema,
    openapi::{
        ArrayBuilder, ObjectBuilder, OneOfBuilder, Ref, RefOr, Schema,
        schema::{AllOfBuilder, ArrayItems, KnownFormat, Object, SchemaFormat, Type},
    },
};

use crate::types::{
    BOARD_SIZE, Piece,
    ws::{PlayerAction, RegisterRoomEvent, Responses},
};

#[derive(OpenApi)]
#[openapi(
    info(
        title = "Numbers server API",
        description = include_str!("../res/api_description.md"),
    ),
    servers((url = "/")),
    tags(
        (name = "server", description = "サーバーそのものの情報"),
        (name = "rooms", description = "部屋の作成と取得"),
        (name = "websocket", description = "対局中のリアルタイム通信"),
    ),
    components(schemas(PlayerAction, Responses, RegisterRoomEvent)),
)]
pub struct ApiDoc;

// utils

pub struct Null;

impl PartialSchema for Null {
    fn schema() -> RefOr<Schema> {
        ObjectBuilder::new().schema_type(Type::Null).into()
    }
}

impl ToSchema for Null {}

#[inline(always)]
pub fn constant<V>(schema_type: Type, value: V) -> Object
where
    V: Into<Value>,
{
    ObjectBuilder::new()
        .schema_type(schema_type)
        .enum_values(Some([value]))
        .build()
}

#[inline(always)]
pub fn uuid() -> ObjectBuilder {
    ObjectBuilder::new()
        .schema_type(Type::String)
        .format(Some(SchemaFormat::KnownFormat(KnownFormat::Uuid)))
}

#[inline(always)]
pub fn tuple<I>(items: I) -> ArrayBuilder
where
    I: IntoIterator<Item = RefOr<Schema>>,
{
    ArrayBuilder::new()
        .items(ArrayItems::False)
        .prefix_items(items.into_iter().map(|item| match item {
            RefOr::T(schema) => schema,
            reference => AllOfBuilder::new().item(reference).into(),
        }))
}

#[inline(always)]
pub fn position() -> ArrayBuilder {
    let coordinate = || {
        ObjectBuilder::new()
            .schema_type(Type::Integer)
            .minimum(Some(0usize))
            .maximum(Some(BOARD_SIZE - 1))
            .into()
    };
    tuple([coordinate(), coordinate()]).description(Some(
        "`[x, y]` の形の座標 (`RoomData.pieces[y][x]` のマスを指す)",
    ))
}

#[inline(always)]
pub fn board() -> ArrayBuilder {
    let row = ArrayBuilder::new()
        .items(
            OneOfBuilder::new()
                .item(Ref::from_schema_name(Piece::name()))
                .item(Null::schema()),
        )
        .min_items(Some(BOARD_SIZE))
        .max_items(Some(BOARD_SIZE));
    ArrayBuilder::new()
        .items(row)
        .min_items(Some(BOARD_SIZE))
        .max_items(Some(BOARD_SIZE))
        .description(Some(format!(
            "`pieces[y][x]` で参照する {}x{} の盤面で，駒がないマスは `null`",
            BOARD_SIZE, BOARD_SIZE
        )))
}

#[inline(always)]
pub fn movement() -> ArrayBuilder {
    tuple([position().into(), position().into()])
}

#[inline(always)]
pub fn tagged(
    tag: u8,
    name: &str,
    description: &str,
    content: Option<RefOr<Schema>>,
) -> ObjectBuilder {
    let builder = ObjectBuilder::new()
        .schema_type(Type::Object)
        .title(Some(name))
        .description(Some(description))
        .property("t", constant(Type::Integer, tag))
        .required("t");
    match content {
        Some(content) => builder.property("c", content).required("c"),
        None => builder,
    }
}

#[inline(always)]
pub fn result(success: bool) -> ObjectBuilder {
    ObjectBuilder::new()
        .schema_type(Type::Object)
        .property("success", constant(Type::Boolean, success))
        .required("success")
}

#[inline(always)]
pub fn failure<T>() -> ObjectBuilder
where
    T: PartialSchema,
{
    result(false)
        .property("message", T::schema())
        .required("message")
}
