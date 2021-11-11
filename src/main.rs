extern crate savefile;
#[macro_use]
extern crate savefile_derive;

use std::env;
use std::path::Path;

use iced::{Align, Button, button, Column, Container, Element, Length, Sandbox, Scrollable, scrollable, Settings, Text, text_input, TextInput, window};
use iced::keyboard::Event;
use savefile::prelude::*;
use serenity::async_trait;
use serenity::Client;
use serenity::client::{Context, EventHandler, validate_token};
use serenity::model::channel::Message as MessageS;
use tokio::task::JoinHandle;


const GLOBAL_VERSION: u32 = 1;


#[derive(Savefile)]
struct State {
    token: String,
    overlay: String,
}

fn save_state(file: &'static str, state: State) {
    // Save current version of file.
    let roaming_path = format!("{}\\OverlayBot\\save\\{}", env::var_os("APPDATA").unwrap().to_str().unwrap(), file);
    std::fs::create_dir_all(Path::new(&roaming_path).parent().unwrap()).unwrap();
    save_file(&roaming_path, GLOBAL_VERSION, &state).unwrap();
}


fn load_state(file: &'static str) -> State {
    // The GLOBAL_VERSION means we have that version of our data structures,
    // but we can still load any older version.
    let roaming_path = format!("{}\\OverlayBot\\save\\{}", env::var_os("APPDATA").unwrap().to_str().unwrap(), file);
    load_file(&roaming_path, GLOBAL_VERSION).unwrap()
}


struct Application {
    scroll: scrollable::State,
    token: text_input::State,
    token_value: String,
    started: button::State,
    started_value: bool,
    overlay: text_input::State,
    overlay_value: String,
    handle: Option<JoinHandle<()>>,
}

#[derive(Debug, Clone)]
enum Message {
    TokenChanged(String),
    StartToggled(bool),
    OverlayChanged(String),
}


impl Sandbox for Application {
    type Message = Message;

    fn new() -> Application {
        let file = "save.bin";
        let roaming_path = format!("{}\\OverlayBot\\save\\{}", env::var_os("APPDATA").unwrap().to_str().unwrap(), file);
        let mut token_value = "".to_string();
        let mut overlay_value = "".to_string();

        if Path::new(&roaming_path).exists() {
            let state: State = load_state(file);
            token_value = state.token;
            overlay_value = state.overlay;
        }

        Application {
            scroll: scrollable::State::new(),
            token: text_input::State::new(),
            token_value,
            started: button::State::new(),
            started_value: false,
            overlay: text_input::State::new(),
            overlay_value,
            handle: Option::None,
        }
    }

    fn title(&self) -> String {
        String::from("Discord Overlay Bot")
    }

    fn update(&mut self, message: Message) {
        match message {
            Message::TokenChanged(_text_input) => {
                self.token_value = _text_input;
            }
            crate::Message::StartToggled(_bool) => {
                if _bool {
                    // bot is running, we need to stop it
                    self.handle.as_ref().unwrap().abort();
                    self.started_value = !_bool;
                    save_state("save.bin", State {
                        token: self.token_value.clone(),
                        overlay: self.overlay_value.clone(),
                    });
                } else {
                    if validate_token(&self.token_value).is_ok() {
                        self.started_value = !_bool;

                        let token = self.token_value.clone();
                        let lock = self.overlay_value.clone();
                        self.handle = Option::Some(
                            tokio::spawn(async {
                                spawn_serenity(token, lock).await
                            }));
                        save_state("save.bin", State {
                            token: self.token_value.clone(),
                            overlay: self.overlay_value.clone(),
                        });
                    }
                }
            }
            Message::OverlayChanged(_text_input) => {
                self.overlay_value = _text_input;
            }
        }
    }

    fn view(&mut self) -> Element<Message> {
        let Application {
            scroll,
            token,
            started,
            overlay,
            token_value,
            started_value,
            overlay_value,
            ..
        } = self;

        let content: Element<_> = Column::new()
            .push(
                Button::new(started, Text::new(
                    if *started_value {
                        "Stop Bot"
                    } else {
                        "Start Bot"
                    }
                ))
                    .on_press(Message::StartToggled(*started_value)))
            .push(
                Text::new(format!("Running? {}", &started_value.to_string()))
            )
            .push(
                TextInput::new(token, "Put your bot token here!", &token_value, Message::TokenChanged)
            )
            .push(
                TextInput::new(overlay, "Put your overlay settings here!", &overlay_value, Message::OverlayChanged)
            )
            .into();

        let scrollable = Scrollable::new(scroll)
            .push(Container::new(content).width(Length::Fill).align_x(Align::Start).align_y(Align::Start));

        Container::new(scrollable)
            .max_height(300)
            .max_width(300)
            .height(Length::Fill)
            .width(Length::Fill)
            .align_x(Align::Start)
            .align_y(Align::Start)
            .padding(20)
            .into()
    }
}

#[tokio::main]
async fn main() -> iced::Result {
    let mut settings = Settings::default();
    settings.window.resizable = false;

    let size: (u32, u32) = (300, 300);

    settings.window.size = size;
    settings.window.min_size = Option::from(size);
    settings.window.max_size = Option::from(size);


    Application::run(settings)
}


struct Handler {
    overlay: String,
}

#[async_trait]
impl EventHandler for Handler {
    async fn message(&self, ctx: Context, msg: MessageS) {
        if msg.content == "!overlay" {
            if msg.guild_id.is_none() {
                // respond with not in a guild
                if let Err(why) = msg.channel_id.say(&ctx.http, "Please use this within a server").await {
                    println!("Error sending message: {:?}", why);
                }
                return;
            }

            let guild = ctx.cache.guild(msg.guild_id.unwrap()).await;
            if guild.is_none() {
                // Could not find guild
                if let Err(why) = msg.channel_id.say(&ctx.http, "Error - Guild not available").await {
                    println!("Error sending message: {:?}", why);
                }
                return;
            }

            let unwrapped_guild = guild.unwrap();
            let user_id = msg.author.id;

            let voice_id = unwrapped_guild.voice_states.get(&user_id);
            if voice_id.is_none() {
                if let Err(why) = msg.channel_id.say(&ctx.http, "Please join a voice channel").await {
                    println!("Error sending message: {:?}", why);
                }
                return;
            }

            let voice_channel = voice_id.unwrap().channel_id;

            if voice_channel.is_none() {
                if let Err(why) = msg.channel_id.say(&ctx.http, "Error - VC not available").await {
                    println!("Error sending message: {:?}", why);
                }
                return;
            }


            let overlay = format!("{}{}/{}?{}", "https://streamkit.discord.com/overlay/voice/",
                                  unwrapped_guild.id.0, voice_channel.unwrap().0, &self.overlay);

            if let Err(why) = msg.channel_id.say(&ctx.http, overlay).await {
                println!("Error sending message: {:?}", why);
            }
        }
    }
}

async fn spawn_serenity(token: String, lock: String) {
    // Login with a bot token from the environment



    let mut client = Client::builder(token)
        .event_handler(Handler {
            overlay: lock
        })
        .await
        .expect("Error creating client");

    // start listening for events by starting a single shard
    if let Err(why) = client.start().await {
        println!("An error occurred while running the client: {:?}", why);
    }
}



