use crate::fx::Fx;
use mongodb::{
    bson::{doc, oid::ObjectId},
    error::Result as MongoDBResult,
    results::{DeleteResult, InsertOneResult},
};
use serde::{Deserialize, Serialize};
const INTERACTION_DATA_COLLECTION: &str = "interaction_data";

#[derive(Serialize, Deserialize, Debug)]
pub enum InteractionData {
    CreatingFx(Fx),
}

pub struct InteractionDataRegistry {
    database: mongodb::Database,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct WithID<T> {
    #[serde(rename = "_id")]
    id: ObjectId,
    #[serde(flatten)]
    data: T,
}

impl InteractionDataRegistry {
    pub fn new(database: mongodb::Database) -> Self {
        Self { database }
    }
    pub async fn create(&self, data: InteractionData) -> MongoDBResult<InsertOneResult> {
        self.database
            .collection(INTERACTION_DATA_COLLECTION)
            .insert_one(data, None)
            .await
    }

    pub async fn get(&self, id: ObjectId) -> MongoDBResult<Option<InteractionData>> {
        self.database
            .collection::<WithID<InteractionData>>(INTERACTION_DATA_COLLECTION)
            .find_one(doc! {"id": id}, None)
            .await
            .map(|option| option.map(|WithID { data, .. }| data))
    }

    pub async fn delete(&self, id: ObjectId) -> MongoDBResult<DeleteResult> {
        self.database
            .collection::<InteractionData>(INTERACTION_DATA_COLLECTION)
            .delete_one(doc! {"_id": id}, None)
            .await
    }
}
