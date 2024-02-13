use super::{IdInterface, ShapeInterface};
use sqlx::{query, Postgres, Transaction};

pub struct Pronouns<'a> {
    pub subj: &'a str,
    pub obj: &'a str,
    pub poss_pres: &'a str,
    pub poss_past: &'a str,
}

impl<'a> From<&'a str> for Pronouns<'a> {
    fn from(pronouns_config: &'a str) -> Self {
        let pronouns: Vec<&str> = pronouns_config.split("/").collect();

        let [subj, obj, poss_pres, poss_past] = pronouns[..4] else {
            panic!("player pronouns incorrectly configured - see config.example.json for example of how to format player pronouns");
        };

        Self {
            subj,
            obj,
            poss_pres,
            poss_past,
        }
    }
}

impl<'a, 'tr> ShapeInterface<'a, 'tr> for Pronouns<'a> {
    type Shape = [String; 4];

    async fn from_values(pronouns_array: &'a Self::Shape) -> Self {
        let [subj, obj, poss_pres, poss_past] = pronouns_array;

        Self {
            subj,
            obj,
            poss_pres,
            poss_past,
        }
    }

    async fn try_fetch_values(
        transaction: &'a mut Transaction<'tr, Postgres>,
        id: i32,
    ) -> sqlx::Result<Self::Shape> {
        let pronouns = query!(
            r#"SELECT subj, obj, poss_pres, poss_past FROM pronouns WHERE id = $1"#,
            id
        )
        .fetch_one(&mut **transaction)
        .await?;

        Ok([
            pronouns.subj,
            pronouns.obj,
            pronouns.poss_pres,
            pronouns.poss_past,
        ])
    }
}

impl<'a, 'tr> IdInterface<'a, 'tr> for Pronouns<'a> {
    type IdType = i32;

    async fn try_fetch_id(
        &self,
        transaction: &'a mut Transaction<'tr, Postgres>,
    ) -> sqlx::Result<Self::IdType> {
        let id = query!(
            r#"SELECT id FROM pronouns WHERE subj = $1 AND obj = $2 AND poss_pres = $3 AND poss_past = $4"#,
            self.subj,
            self.obj,
            self.poss_pres,
            self.poss_past
        ).fetch_one(&mut **transaction).await?.id;

        Ok(id)
    }

    async fn try_insert(
        &self,
        transaction: &'a mut Transaction<'tr, Postgres>,
    ) -> sqlx::Result<Self::IdType> {
        let id = query!(
            r#"INSERT INTO pronouns (subj, obj, poss_pres, poss_past)
            VALUES ( $1, $2, $3, $4 )
            ON CONFLICT DO NOTHING
            RETURNING id
            "#,
            self.subj,
            self.obj,
            self.poss_pres,
            self.poss_past
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
            .expect("failed to insert new pronouns record")
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
