use crate::{GeneratedIdTransaction, ShapeInterface};
use sqlx::{query, Executor, Pool, Postgres};

pub struct Pronouns<'a> {
    pub subj: &'a str,
    pub obj: &'a str,
    pub poss_pres: &'a str,
    pub poss_past: &'a str,
}

impl<'a> From<&'a str> for Pronouns<'a> {
    fn from(value: &'a str) -> Self {
        let pronouns: Vec<&str> = value.split("/").collect();

        let [subj, obj, poss_pres, poss_past] = pronouns[..4] else {
            panic!("player pronouns incorrectly configured - see config.example.json for example of how to format player pronouns");
        };

        Pronouns {
            subj,
            obj,
            poss_pres,
            poss_past,
        }
    }
}

impl<'a> ShapeInterface<'a> for Pronouns<'a> {
    type Shape = [String; 4];

    async fn from_values(values: &'a Self::Shape) -> Self {
        let [subj, obj, poss_pres, poss_past] = values;

        Self {
            subj,
            obj,
            poss_pres,
            poss_past,
        }
    }

    async fn fetch_values<E: Executor<'a, Database = Postgres>>(
        pool: E,
        id: i32,
    ) -> sqlx::Result<Self::Shape> {
        let record = query!(
            r#"SELECT subj, obj, poss_pres, poss_past FROM pronouns WHERE id = $1"#,
            id
        )
        .fetch_one(pool)
        .await?;

        Ok([record.subj, record.obj, record.poss_pres, record.poss_past])
    }

    async fn fetch_id_by_values<E: Executor<'a, Database = Postgres>>(
        pool: E,
        values: &Self,
    ) -> sqlx::Result<i32> {
        let id = query!(
            r#"SELECT id FROM pronouns WHERE subj = $1 AND obj = $2 AND poss_pres = $3 AND poss_past = $4"#,
            values.subj,
            values.obj,
            values.poss_pres,
            values.poss_past
        ).fetch_one(pool).await?.id;

        Ok(id)
    }

    async fn try_insert(
        &self,
        pool: &'a Pool<Postgres>,
    ) -> sqlx::Result<GeneratedIdTransaction<'a>> {
        let mut transaction = pool.begin().await?;

        let id = query!(
            r#"INSERT INTO pronouns (subj, obj, poss_pres, poss_past)
            VALUES ( $1, $2, $3, $4 )
            RETURNING id
            "#,
            self.subj,
            self.obj,
            self.poss_pres,
            self.poss_past
        )
        .fetch_one(&mut *transaction)
        .await?
        .id;

        Ok(GeneratedIdTransaction(transaction, id))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[async_std::test]
    async fn pronouns_table_from_str() {
        let table: Pronouns = "they/them/their/theirs".into();
        assert_eq!(table.subj, "they");
        assert_eq!(table.obj, "them");
        assert_eq!(table.poss_pres, "their");
        assert_eq!(table.poss_past, "theirs");
    }
}
