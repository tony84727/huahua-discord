use mongodb::bson::{self, oid::ObjectId};
use serenity::{
    client::Context,
    model::application::interaction::{
        message_component::MessageComponentInteraction, InteractionResponseType,
    },
};

use crate::fx::{Controller, Creator, Fx, Repository};

use self::data::InteractionData;

pub mod data;
pub mod fx;

#[derive(Debug, PartialEq)]
struct MessageComponentIntent {
    id: ObjectId,
    action: Option<String>,
}

impl TryFrom<&str> for MessageComponentIntent {
    type Error = CustomIDParseError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        let segments: (&str, Option<&str>) = match value.find(":") {
            Some(index) => (&value[..index], Some(&value[index + 1..])),
            None => (&value, None),
        };
        let id = ObjectId::parse_str(segments.0).map_err(CustomIDParseError::MalformedID)?;
        let action = segments.1.map(|x| x.to_string());
        Ok(MessageComponentIntent { id, action: action })
    }
}

#[derive(Debug)]
enum CustomIDParseError {
    MalformedID(bson::oid::Error),
}

pub struct ButtonHandler<'a, C, R>
where
    C: Creator,
    R: Repository,
{
    controller: &'a Controller<C, R>,
    data: data::InteractionDataRegistry,
}

impl<'a, C, R> ButtonHandler<'a, C, R>
where
    C: Creator,
    R: Repository,
{
    pub fn new(controller: &'a Controller<C, R>, database: mongodb::Database) -> Self {
        Self {
            controller,
            data: data::InteractionDataRegistry::new(database),
        }
    }
    pub async fn handle(&self, ctx: &Context, interaction: &MessageComponentInteraction) {
        let MessageComponentIntent { id, .. } =
            match MessageComponentIntent::try_from(interaction.data.custom_id.as_str()) {
                Ok(intent) => intent,
                Err(why) => {
                    log::error!(
                        "receiving a malformed custom_id {}, error: {:?}",
                        &interaction.data.custom_id,
                        why,
                    );
                    return;
                }
            };
        match self.data.get(id).await {
            Ok(Some(InteractionData::CreatingFx(fx))) => {
                self.handle_create(ctx, interaction, fx).await;
            }
            Ok(None) => {
                self.report_staled(ctx, interaction).await;
            }
            Err(why) => {
                log::error!("error while retriving interaction data {:?}", why);
            }
        };
    }

    async fn handle_create(
        &self,
        ctx: &Context,
        interaction: &MessageComponentInteraction,
        fx: Fx,
    ) {
        if let Err(why) = self.controller.confirm_create(fx).await {
            log::error!("{:?}", why);
        }
        if let Err(why) = interaction
            .create_interaction_response(ctx, |message| {
                message
                    .kind(InteractionResponseType::ChannelMessageWithSource)
                    .interaction_response_data(|data| data.ephemeral(true).content("新增成功!"))
            })
            .await
        {
            log::error!("{:?}", why);
        }
    }

    async fn report_staled(&self, ctx: &Context, interaction: &MessageComponentInteraction) {
        if let Err(why) = interaction
            .create_interaction_response(ctx, |message| {
                message
                    .kind(InteractionResponseType::ChannelMessageWithSource)
                    .interaction_response_data(|data| {
                        data.ephemeral(true).content("本毛忘了，請重新呼叫指令")
                    })
            })
            .await
        {
            log::error!("{:?}", why);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use test_case::test_case;

    #[test_case("AAAAAAAAAAAAAAAAAAAAAAAA:create" => MessageComponentIntent {
		id: ObjectId::parse_str("AAAAAAAAAAAAAAAAAAAAAAAA").unwrap(),
		action: Some("create".to_string())
	}; "id + action")]
    #[test_case("AAAAAAAAAAAAAAAAAAAAAAAA" => MessageComponentIntent{
		id: ObjectId::parse_str("AAAAAAAAAAAAAAAAAAAAAAAA").unwrap(),
		action: None
	}; "id") ]
    fn test_message_compoennt_intent_from_custom_id(custom_id: &str) -> MessageComponentIntent {
        MessageComponentIntent::try_from(custom_id).unwrap()
    }
}
