pub use std::{
    env,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
        Mutex,
    },
    time::Duration,
    fs::File,
    io::prelude::*
};

pub use serenity::{
    async_trait,
    client::{Client, Context, EventHandler},
    framework::{
        standard::{
            macros::{command, group},
            Args,
            Delimiter,
            CommandResult,
        },
        StandardFramework,
    },
    http::Http,
    model::{channel::Message, gateway::Ready, prelude::ChannelId},
    prelude::{GatewayIntents, Mentionable},
    Result as SerenityResult,
};

pub use songbird::{
    input::{
        self,
        restartable::Restartable,
        cached::Memory,
        Input
    },
    Event,
    EventContext,
    EventHandler as VoiceEventHandler,
    SerenityInit,
    TrackEvent,
};

pub struct Bot;

#[async_trait]
impl EventHandler for Bot {
    async fn ready(&self, _: Context, ready: Ready) {
        println!("{} is connected!", ready.user.name);
    }

    async fn message(&self, ctx: Context, msg: Message) {
        let user: serenity::model::user::User = msg.author.clone();
        let channel_id = msg.channel_id;
        if user.bot { return; }
        if channel_id.to_string() != env::var("CHAT_ROOM_ID").unwrap_or("".to_string()) { return; }

        let target_message = &msg.content;
        put_log(target_message, &user);

        if target_message.starts_with(super::PREFIX) { return ; }
        let super_user = env::var("SUPER_USER").unwrap_or("none".to_string());
        if user.id.to_string() == super_user {
            v(&ctx, &msg, Args::new(target_message, &[Delimiter::Single(' ')])).await.unwrap();
        }
    }
}

#[group]
#[commands(join, leave, v, vs, list, set)]
struct General;

#[command]
#[only_in(guilds)]
async fn join(ctx: &Context, msg: &Message) -> CommandResult {
    let guild = msg.guild(&ctx.cache).unwrap();
    let guild_id = guild.id;
    let channel_id = msg.channel_id;
    env::set_var("CHAT_ROOM_ID", channel_id.to_string());

    let channel_id = guild
        .voice_states
        .get(&msg.author.id)
        .and_then(|voice_state| voice_state.channel_id);

    let connect_to = match channel_id {
        Some(channel) => channel,
        None => {
            check_msg(msg.reply(ctx, "Not in a voice channel").await);

            return Ok(());
        },
    };

    let manager = songbird::get(ctx)
        .await
        .expect("Songbird Voice client placed in at initialisation.")
        .clone();

    let (_, success) = manager.join(guild_id, connect_to).await;

    if let Ok(_channel) = success {
        check_msg(
            msg.channel_id
                .say(&ctx.http, &format!("Joined {}", connect_to.mention()))
                .await,
        );
    } else {
        check_msg(
            msg.channel_id
                .say(&ctx.http, "Error joining the channel")
                .await,
        );
    }

    let handler_lock = match manager.get(guild_id) {
        Some(handler) => handler,
        None => {
            check_msg(msg.reply(ctx, "Not in a voice channel").await);

            return Ok(());
        },
    };

    let mut handler = handler_lock.lock().await;
    if !handler.is_deaf() {
        if let Err(e) = handler.deafen(true).await {
            println!("failed to deafen: {}", e);
        }
    }

    Ok(())
}

#[command]
#[only_in(guilds)]
async fn leave(ctx: &Context, msg: &Message) -> CommandResult {
    let guild = msg.guild(&ctx.cache).unwrap();
    let guild_id = guild.id;

    env::set_var("CHAT_ROOM_ID", "");

    let manager = songbird::get(ctx).await
        .expect("Songbird Voice client placed in at initialisation.").clone();
    let has_handler = manager.get(guild_id).is_some();

    if has_handler {
        if let Err(e) = manager.remove(guild_id).await {
            check_msg(msg.channel_id.say(&ctx.http, format!("Failed: {:?}", e)).await);
        }

        check_msg(msg.channel_id.say(&ctx.http, "Left voice channel").await);
    } else {
        check_msg(msg.reply(ctx, "Not in a voice channel").await);
    }

    Ok(())
}

#[command]
#[only_in(guilds)]
async fn v(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {
    if msg.channel_id.to_string() != env::var("CHAT_ROOM_ID").unwrap_or("".to_string()) { return Ok(()); }
    let text = match args.single::<String>() {
        Ok(text) => text,
        Err(_) => {
            check_msg(
                msg.channel_id
                    .say(&ctx.http, "something wrong")
                    .await,
            );
            return Ok(());
        }
    };

    let guild = msg.guild(&ctx.cache).unwrap();
    let guild_id = guild.id;

    let manager = songbird::get(ctx)
        .await
        .expect("Songbird Voice client placed in at initialisation.")
        .clone();

    if let Some(handler_lock) = manager.get(guild_id) {
        let mut handler = handler_lock.lock().await;
        let voice_actor = env::var("VOICE_ACTOR").unwrap_or("0".to_string());
        let voice_actor: u32 = voice_actor.parse().unwrap();
        let wav = super::vox::create_wav(&text, voice_actor).await.unwrap();
        {
            use std::fs::File;
            use std::io::prelude::*;
            let mut f = File::create("buffer.wav").unwrap();
            f.write_all(&wav).unwrap();
        }
        let source = input::ffmpeg("buffer.wav").await.unwrap();
        handler.enqueue_source(source.into());
    } else {
        check_msg(
            msg.channel_id
                .say(&ctx.http, "Not in a voice channel to play in")
                .await,
        );
    }

    Ok(())
}

#[command]
#[only_in(guilds)]
async fn vs(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {
    if msg.channel_id.to_string() != env::var("CHAT_ROOM_ID").unwrap_or("".to_string()) { return Ok(()); }
    let num = match args.single::<String>() {
        Ok(text) => text,
        Err(_) => {
            check_msg(
                msg.channel_id
                    .say(&ctx.http, "something wrong")
                    .await,
            );
            return Ok(());
        }
    };
    let num: u32 = num.parse().unwrap_or(51);
    if num > 50 {
        check_msg(
            msg.channel_id
                .say(&ctx.http, "accepted value: 0 ~ 50")
                .await,
        );
        return Ok(());
    }
    let text = match args.single::<String>() {
        Ok(text) => text,
        Err(_) => {
            check_msg(
                msg.channel_id
                    .say(&ctx.http, "something wrong")
                    .await,
            );
            return Ok(());
        }
    };

    let guild = msg.guild(&ctx.cache).unwrap();
    let guild_id = guild.id;

    let manager = songbird::get(ctx)
        .await
        .expect("Songbird Voice client placed in at initialisation.")
        .clone();

    if let Some(handler_lock) = manager.get(guild_id) {
        let mut handler = handler_lock.lock().await;
        let wav = super::vox::create_wav(&text, num).await.unwrap();
        let mut f = File::create("buffer.wav").unwrap();
        f.write_all(&wav).unwrap();
        let source = input::ffmpeg("buffer.wav").await.unwrap();
        handler.enqueue_source(source.into());
    } else {
        check_msg(
            msg.channel_id
                .say(&ctx.http, "Not in a voice channel to play in")
                .await,
        );
    }

    Ok(())
}

#[command]
#[only_in(guilds)]
async fn list(ctx: &Context, msg: &Message) -> CommandResult {
    let speakers = super::vox::get_speakers().await;
    match speakers {
        Some(value) => {
            let mut result = String::new();
            for row in value {
                for el in row {
                    result.push_str(&el);
                    result.push_str(" ");
                }
                result.push_str("\n");
            }
            check_msg(
                msg.channel_id
                    .say(&ctx.http, result)
                    .await,
            );
        },
        _ => (),
    }
    Ok(())
}

#[command]
#[only_in(guilds)]
async fn set(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {
    let num = match args.single::<String>() {
        Ok(text) => text,
        Err(_) => {
            check_msg(
                msg.channel_id
                    .say(&ctx.http, "something wrong")
                    .await,
            );
            return Ok(());
        }
    };
    let num: u32 = num.parse().unwrap_or(51);
    if num > 50 {
        check_msg(
            msg.channel_id
                .say(&ctx.http, "accepted value: 0 ~ 50")
                .await,
        );
        return Ok(());
    }
    env::set_var("VOICE_ACTOR", num.to_string());
    Ok(())
}

fn check_msg(result: SerenityResult<Message>) {
    if let Err(why) = result {
        println!("Error sending message: {:?}", why);
    }
}

// fn get_setting() -> String {
//     let mut buf = String::new();
//     let mut f = File::open("setting").unwrap();
//     f.read_to_string(&mut buf).unwrap();
//     buf
// }

fn put_log(text: &str, user: &serenity::model::user::User) {
    let mut log_text: String = String::new();
    for (i, c) in text.chars().enumerate() {
        if !(i > 15) { log_text.push_str(&c.to_string()); }
        else if !(i > 18) { log_text.push_str("."); }
        else { break; }
    }
    println!("[log] message {}#{:04} ({}) {{ {} }}", user.name, user.discriminator, user.id, log_text);
}