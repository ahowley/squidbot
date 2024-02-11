use sqlx::{postgres::PgPoolOptions, Pool, Postgres};
use std::env;

async fn get_postgres_url() -> String {
    env::var("DATABASE_URL").expect("failed to load DATABASE_URL environment variable")
}

pub async fn get_connection_pool(path_to_dotenv: &str) -> Pool<Postgres> {
    dotenv::from_path(path_to_dotenv)
        .expect("failed to find or load .env from provided path_to_dotenv");
    let connection_url = get_postgres_url().await;

    PgPoolOptions::new()
        .max_connections(5)
        .connect(&connection_url)
        .await
        .expect("failed to connect to database - check DATABASE_URL .env variable and ensure your database server is running")
}
