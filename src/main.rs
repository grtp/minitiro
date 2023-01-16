use songbird::SerenityInit;
use serenity::prelude::*;
use dotenv::dotenv;
use std::sync::{ Arc, RwLock };

mod vox;
mod bot;

struct VoiceActorListLength;
impl TypeMapKey for VoiceActorListLength { type Value = Arc<RwLock<usize>>; }
struct VoiceActorList;
impl TypeMapKey for VoiceActorList { type Value = Arc<RwLock<Vec<(String, String)>>>; }
struct CurrentVoiceActor;
impl TypeMapKey for CurrentVoiceActor { type Value = Arc<RwLock<String>>; }
struct AllowChannels;
impl TypeMapKey for AllowChannels { type Value = Arc<RwLock<Vec<String>>>; }
struct AllowMembers;
impl TypeMapKey for AllowMembers { type Value = Arc<RwLock<Vec<String>>>; }

pub const CMD_PREFIX: &str = "/";

#[tokio::main]
async fn main() {
    dotenv().ok();
    let token = bot::env::var("DISCORD_TOKEN").expect("Expected a token in the environment");

    let framework = bot::StandardFramework::new()
        .configure(|c| c.prefix(CMD_PREFIX))
        .group(&bot::GENERAL_GROUP);

    let intents = bot::GatewayIntents::non_privileged()
        | bot::GatewayIntents::MESSAGE_CONTENT;

    let mut client = bot::Client::builder(token, intents)
        .event_handler(bot::Bot)
        .framework(framework)
        .register_songbird()
        .await
        .expect("Err creating client");
    {
        let mut data = client.data.write().await;
        let speakers = vox::get_speakers().await;
        let Some(speakers) = speakers else { panic!("[Err] Ops can't found any actor"); };
        data.insert::<VoiceActorListLength>(Arc::new(RwLock::new(speakers.len())));
        data.insert::<VoiceActorList>(Arc::new(RwLock::new(speakers)));
        data.insert::<CurrentVoiceActor>(Arc::new(RwLock::new("29".to_string())));
        data.insert::<AllowChannels>(Arc::new(RwLock::new(Vec::<String>::new())));
        data.insert::<AllowMembers>(Arc::new(RwLock::new(Vec::<String>::new())));
    }

    let _ = client
        .start()
        .await
        .map_err(|why| println!("Client ended: {:?}", why));
}