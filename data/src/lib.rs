pub use interface::*;
use sqlx::{postgres::PgPoolOptions, Pool, Postgres, Transaction};
use std::env;

mod interface;

async fn get_postgres_url() -> String {
    env::var("DATABASE_URL").expect("failed to load DATABASE_URL environment variable")
}

pub async fn create_connection_pool(path_to_dotenv: &str) -> Pool<Postgres> {
    dotenv::from_path(path_to_dotenv)
        .expect("failed to find or load .env from provided path_to_dotenv");
    let connection_url = get_postgres_url().await;

    PgPoolOptions::new()
        .max_connections(5)
        .connect(&connection_url)
        .await
        .expect("failed to connect to database - check DATABASE_URL .env variable and ensure your database server is running")
}

pub async fn begin_transaction<'a>(pool: &'a Pool<Postgres>) -> Transaction<'a, Postgres> {
    pool.begin()
        .await
        .expect("failed to initiate database transaction")
}
