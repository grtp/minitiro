pub use std::{
    env,
    sync::{
        Arc,
        Mutex,
    },
    fs::File,
    io::prelude::*,
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
    model::{channel::Message, gateway::Ready, prelude::ChannelId},
    prelude::{GatewayIntents, Mentionable},
    Result as SerenityResult,
};

pub use songbird::{
    input,
    EventHandler as VoiceEventHandler,
};

use crate::{VoiceActorListLength, VoiceActorList, CurrentVoiceActor, AllowChannels, AllowMembers};

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

        let target_message = &msg.content;
        if target_message.starts_with(super::CMD_PREFIX) { return ; }
        put_log(target_message, &user);

        let allow_channels = {
            let data = ctx.data.read().await;
            data.get::<AllowChannels>().unwrap().clone()
        };
        if !allow_channels.read().unwrap().contains(&channel_id.to_string()) { return; }

        let allow_members = {
            let data = ctx.data.read().await;
            data.get::<AllowMembers>().unwrap().clone()
        };
        if !allow_members.read().unwrap().contains(&user.id.to_string()) { return; }

        r(&ctx, &msg, Args::new(target_message, &[Delimiter::Single(' ')])).await.unwrap();
    }
}

#[group]
#[commands( minitiro, fire, r, i, set, readme, ignore, list, list_pretty )]
struct General;

#[command]
#[aliases(召喚, invite, comeon)]
#[only_in(guilds)]
async fn minitiro(ctx: &Context, msg: &Message) -> CommandResult {
    let guild = msg.guild(&ctx.cache).unwrap();
    let guild_id = guild.id;
    let channel_id = msg.channel_id;
    let user_id = msg.author.id.to_string();
    
    let allow_channels = {
        let data = ctx.data.write().await;
        data.get::<AllowChannels>().unwrap().clone()
    };
    allow_channels.write().unwrap().push(channel_id.to_string());

    let is_hit = {
        let data = ctx.data.read().await;
        data.get::<AllowMembers>().unwrap().clone()
    };
    let is_hit = is_hit.read().unwrap().clone().contains(&user_id);

    if !is_hit {
        let allow_members = {
            let data = ctx.data.write().await;
            data.get::<AllowMembers>().unwrap().clone()
        };
        allow_members.write().unwrap().push(user_id);
    }

    let channel_id = guild
        .voice_states
        .get(&msg.author.id)
        .and_then(|voice_state| voice_state.channel_id);

    let connect_to = match channel_id {
        Some(channel) => channel,
        None => {
            check_msg(msg.reply(ctx, "どのボイスチャンネルに入ればいいかわからへん").await);

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
                .say(&ctx.http, &format!("{} に接続！", connect_to.mention()))
                .await,
        );
    } else {
        check_msg(
            msg.channel_id
                .say(&ctx.http, "そのチャンネルには入れん")
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
#[aliases(首, kill, dead, leave)]
#[only_in(guilds)]
async fn fire(ctx: &Context, msg: &Message) -> CommandResult {
    let guild = msg.guild(&ctx.cache).unwrap();
    let guild_id = guild.id;

    let allow_channels = {
        let data = ctx.data.write().await;
        data.get::<AllowChannels>().unwrap().clone()
    };
    *allow_channels.write().unwrap() = Vec::<String>::new();

    let allow_members = {
        let data = ctx.data.write().await;
        data.get::<AllowMembers>().unwrap().clone()
    };
    *allow_members.write().unwrap() = Vec::<String>::new();

    let manager = songbird::get(ctx).await
        .expect("Songbird Voice client placed in at initialisation.").clone();
    let has_handler = manager.get(guild_id).is_some();

    if has_handler {
        if let Err(e) = manager.remove(guild_id).await {
            check_msg(msg.channel_id.say(&ctx.http, format!("Failed: {:?}", e)).await);
        }

        check_msg(msg.channel_id.say(&ctx.http, "ほなまた :wave:").await);
    } else {
        check_msg(msg.reply(ctx, "Not in a voice channel").await);
    }

    Ok(())
}

#[command]
#[aliases(read)]
#[only_in(guilds)]
async fn r(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {
    let allow_channels = {
        let data = ctx.data.read().await;
        data.get::<AllowChannels>().unwrap().clone()
    };
    if !allow_channels.read().unwrap().contains(&msg.channel_id.to_string()) { return Ok(()); }
    let text = match args.single::<String>() {
        Ok(text) => text,
        Err(_) => {
            check_msg(
                msg.channel_id
                    .say(&ctx.http, "読めませんでした")
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
        let current_voice_actor = {
            let data = ctx.data.read().await;
            data.get::<CurrentVoiceActor>().unwrap().clone()
        };
        let current_voice_actor = current_voice_actor.read().unwrap().clone();
        let current_voice_actor: usize = current_voice_actor.parse().unwrap();
        let wav = super::vox::create_wav(&text, current_voice_actor).await.unwrap();
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
#[aliases(read_with_id)]
#[only_in(guilds)]
async fn i(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {
    let allow_channels = {
        let data = ctx.data.read().await;
        data.get::<AllowChannels>().unwrap().clone()
    };
    if !allow_channels.read().unwrap().contains(&msg.channel_id.to_string()) { return Ok(()); }
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
    let num: usize = num.parse().unwrap_or(9999);
    let list_len = {
        let data = ctx.data.read().await;
        data.get::<VoiceActorListLength>().unwrap().clone()
    };
    if num >= *list_len.read().unwrap() {
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
    let voice_list = {
        let data = ctx.data.read().await;
        data.get::<VoiceActorList>().unwrap().clone()
    };
    let voice_list = voice_list.read().unwrap().clone();
    let mut response = String::new();
    for item in voice_list.iter() {
        response.push_str(&format!("・{:02} {}\n", item.0.parse::<u32>().unwrap(), item.1.replace("\"", "")));
    }
    check_msg(
        msg.channel_id
            .say(&ctx.http, response)
            .await,
    );
    Ok(())
}

#[command]
#[only_in(guilds)]
async fn list_pretty(ctx: &Context, msg: &Message) -> CommandResult {
    let voice_list = {
        let data = ctx.data.read().await;
        data.get::<VoiceActorList>().unwrap().clone()
    };
    let mut voice_list = voice_list.read().unwrap().clone();
    let mut response = String::new();
    voice_list.sort_by_key(|x| x.0.parse::<u32>().unwrap());
    for item in voice_list.iter() {
        response.push_str(&format!("・{:02} {}\n", item.0.parse::<u32>().unwrap(), item.1.replace("\"", "")));
    }
    check_msg(
        msg.channel_id
            .say(&ctx.http, response)
            .await,
    );
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
    let num: usize = num.parse().unwrap_or(9999);
    let list_len = {
        let data = ctx.data.read().await;
        data.get::<VoiceActorListLength>().unwrap().clone()
    };
    let list_len = list_len.read().unwrap().clone();
    if num >= list_len {
        check_msg(
            msg.channel_id
                .say(&ctx.http, format!("accepted value: 0 ~ {}", list_len - 1))
                .await,
        );
        return Ok(());
    }
    let current_voice_actor = {
        let data = ctx.data.write().await;
        data.get::<CurrentVoiceActor>().unwrap().clone()
    };
    let mut current_voice_actor = current_voice_actor.write().unwrap();
    *current_voice_actor = num.to_string();
    Ok(())
}

#[command]
#[only_in(guilds)]
async fn readme(ctx: &Context, msg: &Message) -> CommandResult {
    let user_id = msg.author.id.to_string();
    let is_hit = {
        let data = ctx.data.read().await;
        data.get::<AllowMembers>().unwrap().clone()
    };
    let is_hit = is_hit.read().unwrap().clone().contains(&user_id);
    let allow_members = {
        let data = ctx.data.write().await;
        data.get::<AllowMembers>().unwrap().clone()
    };
    if !is_hit {
        allow_members.write().unwrap().push(user_id);
    }
    Ok(())
}

#[command]
#[only_in(guilds)]
async fn ignore(ctx: &Context, msg: &Message) -> CommandResult {
    let user_id = msg.author.id.to_string();
    let index = {
        let data = ctx.data.read().await;
        data.get::<AllowMembers>().unwrap().clone()
    };
    let index = index.read().unwrap().clone();
    let Some(index) = index.iter().position(|x| *x == user_id ) else {
        return Ok(());
    };
    let allow_members = {
        let data = ctx.data.write().await;
        data.get::<AllowMembers>().unwrap().clone()
    };
    allow_members.write().unwrap().remove(index);
    Ok(())
}

fn check_msg(result: SerenityResult<Message>) {
    if let Err(why) = result {
        println!("Error sending message: {:?}", why);
    }
}

fn put_log(text: &str, user: &serenity::model::user::User) {
    let mut log_text: String = String::new();
    for (i, c) in text.chars().enumerate() {
        if !(i > 15) { log_text.push_str(&c.to_string()); }
        else if !(i > 18) { log_text.push_str("."); }
        else { break; }
    }
    println!("[log] msg {}#{:04} ({}) {{ {} }}", user.name, user.discriminator, user.id, log_text);
}