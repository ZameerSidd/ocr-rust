use serde::{Deserialize};
use tiberius::Query;

#[derive(Clone)]
pub enum SqlParam {
    String(String),
    // I32(i32),
    I64(i64),
    // F32(f32),
    // F64(f64),
    // DateTime(NaiveDateTime), 
    // Bool(bool),
    // Null,
    // Add other types as needed
}

impl SqlParam {
    pub fn bind_to_query<'a>(&'a self, query: &mut Query<'a>) {
        match self {
            SqlParam::String(s) => query.bind(s.as_str()),
            //SqlParam::I32(i) => query.bind(*i),
            SqlParam::I64(i) => query.bind(*i),
            // SqlParam::F64(i) => query.bind(*i),
            // SqlParam::F32(i) => query.bind(*i),
            // SqlParam::Bool(b) => query.bind(*b),
            // SqlParam::DateTime(dt) => query.bind(*dt),
            // SqlParam::Null => query.bind(Option::<&str>::None),
        };
    }
}

#[derive(Deserialize, Debug)]
pub struct TokenResponse {
    pub access_token: String
}

