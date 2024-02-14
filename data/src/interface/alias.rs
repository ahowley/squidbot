use super::{IdInterface, ShapeInterface};
use sqlx::{query, Postgres, Transaction};

pub struct Alias {
    pub sender_id: i32,
    pub player_id: i32,
}
