use parse::Post;
use sqlx::{query, Postgres};

pub struct PostInterface<'a> {
    pub transaction: sqlx::Transaction<'a, Postgres>,
    pub post: Post,
    pub campaign_id: i32,
}

impl<'a> PostInterface<'a> {
    pub async fn try_insert(&mut self) -> sqlx::Result<()> {
        let sender_id = query!(
            r#"SELECT id FROM sender WHERE sender_name = $1"#,
            self.post.sender_name,
        )
        .fetch_one(&mut *self.transaction)
        .await?
        .id;

        let id = query!(
            r#"INSERT INTO post (id, campaign_id, sender_id, timestamp_sent)
            VALUES ( $1, $2, $3, $4 )
            RETURNING id"#,
            self.post.id,
            self.campaign_id,
            sender_id,
            self.post.datetime,
        )
        .fetch_one(&mut *self.transaction)
        .await?
        .id;

        if self.post.is_message {
            query!(
                r#"INSERT INTO chat_message (post_id, content)
                VALUES ( $1, $2 )"#,
                id,
                self.post.content_raw,
            )
            .execute(&mut *self.transaction)
            .await?;
        }

        for roll in &self.post.rolls {
            let roll_id = query!(
                r#"INSERT INTO roll (post_id, formula, outcome)
                VALUES ( $1, $2, $3 )
                RETURNING id"#,
                id,
                roll.formula,
                roll.outcome,
            )
            .fetch_one(&mut *self.transaction)
            .await?
            .id;

            for single_roll in &roll.single_rolls {
                query!(
                    r#"INSERT INTO roll_single (roll_id, faces, outcome)
                    VALUES ( $1, $2, $3)"#,
                    roll_id,
                    single_roll.faces,
                    single_roll.outcome,
                )
                .execute(&mut *self.transaction)
                .await?;
            }
        }

        Ok(())
    }
}
