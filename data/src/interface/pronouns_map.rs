use super::{IdInterface, ShapeInterface};
use sqlx::{query, Postgres, Transaction};

pub struct PronounsMap<'a> {
    pub pronouns_values: &'a [String; 4],
    pub player_name: &'a str,
}

impl<'a, 'tr> ShapeInterface<'a, 'tr> for PronounsMap<'a> {
    type Shape = ([String; 4], String);

    async fn from_values(values_tuple: &'a Self::Shape) -> Self {
        Self {
            pronouns_values: &values_tuple.0,
            player_name: &values_tuple.1,
        }
    }

    async fn try_fetch_values(
        transaction: &'a mut Transaction<'tr, Postgres>,
        id: i32,
    ) -> sqlx::Result<Self::Shape> {
        let joined = query!(
            r#"SELECT player_name, subj, obj, poss_pres, poss_past
            FROM pronouns_map
                JOIN pronouns
                ON pronouns_id = pronouns.id
                JOIN player
                ON player_id = player.id
            WHERE pronouns_map.id = $1"#,
            id
        )
        .fetch_one(&mut **transaction)
        .await?;

        let player_name = joined.player_name;
        let pronouns_values = [joined.subj, joined.obj, joined.poss_pres, joined.poss_past];

        Ok((pronouns_values, player_name))
    }
}

impl<'a, 'tr> IdInterface<'a, 'tr> for PronounsMap<'a> {
    type IdType = i32;

    async fn try_fetch_id(
        &self,
        transaction: &'a mut Transaction<'tr, Postgres>,
    ) -> sqlx::Result<Self::IdType> {
        let id = query!(
            r#"SELECT pronouns_map.id FROM pronouns_map
                JOIN pronouns ON pronouns_id = pronouns.id
                JOIN player ON player_id = player.id
            WHERE
                subj = $1
                AND obj = $2
                AND poss_pres = $3
                AND poss_past = $4
                AND player_name = $5"#,
            self.pronouns_values[0],
            self.pronouns_values[1],
            self.pronouns_values[2],
            self.pronouns_values[3],
            self.player_name,
        )
        .fetch_one(&mut **transaction)
        .await?
        .id;

        Ok(id)
    }

    async fn try_insert(
        &self,
        transaction: &'a mut Transaction<'tr, Postgres>,
    ) -> sqlx::Result<Self::IdType> {
        let id = query!(
            r#"INSERT INTO pronouns_map (pronouns_id, player_id)
            SELECT (
                SELECT id FROM pronouns
                WHERE subj = $1
                    AND obj = $2
                    AND poss_pres = $3
                    AND poss_past = $4
            ) as pronouns_id, (
                SELECT id
                FROM player
                WHERE player_name = $5
            ) as player_id
            ON CONFLICT DO NOTHING
            RETURNING id
            "#,
            self.pronouns_values[0],
            self.pronouns_values[1],
            self.pronouns_values[2],
            self.pronouns_values[3],
            self.player_name,
        )
        .fetch_one(&mut **transaction)
        .await?
        .id;

        Ok(id)
    }

    async fn fetch_or_insert_id(
        &self,
        transaction: &'a mut Transaction<'tr, Postgres>,
    ) -> Self::IdType {
        if let Ok(id) = self.try_fetch_id(&mut *transaction).await {
            return id;
        }

        self.try_insert(&mut *transaction)
            .await
            .expect("failed to insert new pronouns_map record")
    }
}
