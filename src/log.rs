pub fn common_log_setting() {
    env_logger::Builder::from_default_env()
        .filter_module("huahua_discord", log::LevelFilter::Debug)
        .filter_module("serenity", log::LevelFilter::Error)
        .filter_module("songbird", log::LevelFilter::Error)
        .init();
}
