use songbird::SerenityInit;
use dotenv::dotenv;

pub const PREFIX: &str = "!";

mod vox;
mod bot;

#[tokio::main]
async fn main() {
    // init settings
    dotenv().ok();
    let token = bot::env::var("DISCORD_TOKEN").expect("Expected a token in the environment");
    // let mut f = File::create("setting").unwrap();
    // f.write_all(b"13").unwrap();

    let framework = bot::StandardFramework::new()
        .configure(|c| c.prefix(PREFIX))
        .group(&bot::GENERAL_GROUP);

    let intents = bot::GatewayIntents::non_privileged()
        | bot::GatewayIntents::MESSAGE_CONTENT;

    let mut client = bot::Client::builder(token, intents)
        .event_handler(bot::Bot)
        .framework(framework)
        .register_songbird()
        .await
        .expect("Err creating client");

    let _ = client
        .start()
        .await
        .map_err(|why| println!("Client ended: {:?}", why));
}