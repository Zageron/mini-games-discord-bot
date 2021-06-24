pub mod commands;
pub mod zmg;

#[macro_use]
extern crate prettytable;

extern crate redis;
use redis::AsyncCommands;

use futures::stream::StreamExt;

use dotenv::dotenv;
use serde::{Deserialize, Serialize};
use std::{env, error::Error};
use twilight_cache_inmemory::{InMemoryCache, ResourceType};
use twilight_gateway::{
    cluster::{Cluster, ShardScheme},
    Event,
};
use twilight_http::Client as HttpClient;
use twilight_model::gateway::Intents;

pub trait StatefulGame {
    fn new() -> Self;
    fn start(&mut self);
    fn pick(&mut self, index: usize, player: u64) -> bool;
    fn view_playfield(&self) -> [u64; 9];
}

#[derive(Serialize, Deserialize, Debug)]
pub struct TicTacToe {
    play_field: [u64; 9],
}

impl StatefulGame for TicTacToe {
    fn new() -> Self {
        Self {
            play_field: [1, 2, 3, 4, 5, 6, 7, 8, 9],
        }
    }

    fn start(&mut self) {
        self.play_field = [1, 2, 3, 4, 5, 6, 7, 8, 9];
    }

    fn pick(&mut self, index: usize, player: u64) -> bool {
        if index < 8 && self.play_field[index] <= 9 {
            self.play_field[index] = player;
            return true;
        }

        false
    }

    fn view_playfield(&self) -> [u64; 9] {
        self.play_field
    }
}

fn pretty_table(table: &[u64; 9]) -> prettytable::Table {
    ptable!(
        [table[0], table[1], table[2]],
        [table[3], table[4], table[5]],
        [table[6], table[7], table[8]]
    )
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error + Send + Sync>> {
    dotenv().ok();
    let token = env::var("DISCORD_TOKEN").expect("Expected a token, check your .env file.");

    let scheme = ShardScheme::Auto;

    let (cluster, mut events) = Cluster::builder(&token, Intents::GUILD_MESSAGES)
        .shard_scheme(scheme)
        .build()
        .await?;

    let cluster_spawn = cluster.clone();

    tokio::spawn(async move {
        cluster_spawn.up().await;
    });

    let http = HttpClient::new(&token);

    let cache = InMemoryCache::builder()
        .resource_types(ResourceType::MESSAGE)
        .build();

    while let Some((shard_id, event)) = events.next().await {
        cache.update(&event);
        tokio::spawn(handle_event(shard_id, event, http.clone()));
    }

    Ok(())
}

async fn handle_event(
    shard_id: u64,
    event: Event,
    http: HttpClient,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    match event {
        Event::MessageCreate(msg) => match msg.content.as_str() {
            "!start" => {
                let game = TicTacToe::new();
                let serialized_game = serde_json::to_string(&game)?;

                let client = redis::Client::open("redis://127.0.0.1/")?;
                let mut con = client.get_async_connection().await?;
                con.set("game", serialized_game).await?;

                http.create_message(msg.channel_id)
                    .content(
                        "```\n".to_string()
                            + &pretty_table(&game.view_playfield()).to_string()
                            + "\n```",
                    )?
                    .await?;
            }
            "!pick" => {
                http.create_message(msg.channel_id)
                    .content("got pick")?
                    .await?;
            }
            "!ping" => {
                http.create_message(msg.channel_id)
                    .content("pong!")?
                    .await?;
            }
            _ => {}
        },
        Event::ShardConnected(_) => {
            println!("Connected on shard {}", shard_id);
        }
        // Other events here...
        _ => {}
    }

    Ok(())
}
