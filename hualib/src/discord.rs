use serenity::model::channel::Message;

pub fn check_msg(result: serenity::Result<Message>) {
    if let Err(err) = result {
        log::error!("error sending message: {:?}", err);
    }
}
