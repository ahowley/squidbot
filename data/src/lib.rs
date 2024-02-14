pub use interface::*;
use parse::parse_config::{Config, PlayerConfig};
use sqlx::{postgres::PgPoolOptions, query, Pool, Postgres, Transaction};
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

async fn update_player_from_config<'a, 'tr>(
    transaction: &'a mut Transaction<'tr, Postgres>,
    player_name: &String,
    player_config: &PlayerConfig,
) -> i32 {
    let player = Player::from_values(player_name).await;
    let player_id = player.fetch_or_insert_id(&mut *transaction).await;

    let mut valid_pronouns_maps: Vec<i32> = vec![];
    for pronouns_string in &player_config.pronouns {
        let pronouns: Pronouns = pronouns_string[..].into();
        let pronouns_id = pronouns.fetch_or_insert_id(&mut *transaction).await;

        let pronouns_map_values = [pronouns_id, player_id];
        let pronouns_map = PronounsMap::from_values(&pronouns_map_values).await;
        let pronouns_map_id = pronouns_map.fetch_or_insert_id(&mut *transaction).await;
        valid_pronouns_maps.push(pronouns_map_id);
    }

    let mut valid_censors: Vec<i32> = vec![];
    for deadname in &player_config.deadnames {
        if deadname.len() == 0 {
            continue;
        }
        let censor_values = (deadname.clone(), player_id);
        let censor = Censor::from_values(&censor_values).await;
        let censor_id = censor.fetch_or_insert_id(&mut *transaction).await;
        valid_censors.push(censor_id);
    }

    let player_pronouns_maps = query!(
        r#"SELECT pronouns_map.id
        FROM pronouns_map
        WHERE player_id = $1"#,
        player_id
    )
    .fetch_all(&mut **transaction)
    .await
    .expect("failed to fetch player pronouns from database");

    let player_censors = query!(
        r#"SELECT id
        FROM censor
        WHERE player_id = $1"#,
        player_id
    )
    .fetch_all(&mut **transaction)
    .await
    .expect("failed to fetch censors for player from database");

    for pronouns_map in player_pronouns_maps {
        let map_id = pronouns_map.id;
        if !valid_pronouns_maps.contains(&map_id) {
            query!(r#"DELETE FROM pronouns_map WHERE id = $1"#, map_id)
                .execute(&mut **transaction)
                .await
                .expect("failed to prune pronouns_map");
        }
    }

    for censor in player_censors {
        let censor_id = censor.id;
        if !valid_censors.contains(&censor_id) {
            query!(r#"DELETE FROM censor WHERE id = $1"#, censor_id)
                .execute(&mut **transaction)
                .await
                .expect("failed to prune censors");
        }
    }

    player_id
}

pub async fn update_players<'a, 'tr>(
    transaction: &'a mut Transaction<'tr, Postgres>,
    config: &Config,
) {
    let players = &config.players;
    for (player_name, player_config) in players {
        update_player_from_config(&mut *transaction, player_name, player_config).await;
    }
}
