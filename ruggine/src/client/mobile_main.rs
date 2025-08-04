use dioxus::prelude::*;
use ruggine::common::{protocol::Message, models::User};
use std::collections::HashMap;
use tokio::net::TcpStream;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

fn main() {
    dioxus_mobile::launch(app);
}

fn app(cx: Scope) -> Element {
    let connected = use_state(cx, || false);
    let username = use_state(cx, || String::new());
    let current_group = use_state(cx, || String::new());
    let messages = use_state(cx, || Vec::<String>::new());
    let input_message = use_state(cx, || String::new());
    let server_address = use_state(cx, || "127.0.0.1:5000".to_string());

    cx.render(rsx! {
        div {
            style: "font-family: Arial, sans-serif; max-width: 100%; margin: 0 auto; padding: 20px; background: #f5f5f5; min-height: 100vh;",
            
            // Header
            div {
                style: "background: #2c3e50; color: white; padding: 15px; border-radius: 8px; margin-bottom: 20px; text-align: center;",
                h1 { "🦀 Ruggine Chat Mobile" }
                if !connected.get() {
                    p { "Tap to connect and start chatting!" }
                } else {
                    p { "Connected as: {username.get()}" }
                    if !current_group.is_empty() {
                        p { "Group: {current_group.get()}" }
                    }
                }
            }

            if !connected.get() {
                // Connection Screen
                div {
                    style: "background: white; padding: 20px; border-radius: 8px; box-shadow: 0 2px 4px rgba(0,0,0,0.1);",
                    h2 { "Connect to Server" }
                    
                    input {
                        style: "width: 100%; padding: 12px; margin: 10px 0; border: 1px solid #ddd; border-radius: 4px; font-size: 16px;",
                        placeholder: "Server address (e.g., 127.0.0.1:5000)",
                        value: "{server_address.get()}",
                        oninput: move |evt| server_address.set(evt.value.clone())
                    }
                    
                    input {
                        style: "width: 100%; padding: 12px; margin: 10px 0; border: 1px solid #ddd; border-radius: 4px; font-size: 16px;",
                        placeholder: "Choose your username",
                        value: "{username.get()}",
                        oninput: move |evt| username.set(evt.value.clone())
                    }
                    
                    button {
                        style: "width: 100%; padding: 15px; background: #3498db; color: white; border: none; border-radius: 4px; font-size: 16px; font-weight: bold; cursor: pointer;",
                        onclick: move |_| {
                            if !username.is_empty() && !server_address.is_empty() {
                                // TODO: Implement connection logic
                                connected.set(true);
                            }
                        },
                        "Connect"
                    }
                }
            } else {
                // Chat Interface
                div {
                    style: "display: flex; flex-direction: column; height: 70vh;",
                    
                    // Messages Area
                    div {
                        style: "flex: 1; background: white; padding: 15px; border-radius: 8px; margin-bottom: 10px; overflow-y: auto; border: 1px solid #ddd;",
                        if messages.is_empty() {
                            div {
                                style: "text-align: center; color: #888; padding: 20px;",
                                p { "No messages yet. Start the conversation!" }
                            }
                        } else {
                            for message in messages.iter() {
                                div {
                                    style: "margin: 8px 0; padding: 8px; background: #f8f9fa; border-radius: 4px; border-left: 3px solid #3498db;",
                                    "{message}"
                                }
                            }
                        }
                    }
                    
                    // Input Area
                    div {
                        style: "display: flex; gap: 10px;",
                        input {
                            style: "flex: 1; padding: 12px; border: 1px solid #ddd; border-radius: 4px; font-size: 16px;",
                            placeholder: "Type your message...",
                            value: "{input_message.get()}",
                            oninput: move |evt| input_message.set(evt.value.clone()),
                            onkeypress: move |evt| {
                                if evt.key() == Key::Enter && !input_message.is_empty() {
                                    let mut msgs = messages.get().clone();
                                    msgs.push(format!("{}: {}", username.get(), input_message.get()));
                                    messages.set(msgs);
                                    input_message.set(String::new());
                                }
                            }
                        }
                        button {
                            style: "padding: 12px 20px; background: #2ecc71; color: white; border: none; border-radius: 4px; font-weight: bold; cursor: pointer;",
                            onclick: move |_| {
                                if !input_message.is_empty() {
                                    let mut msgs = messages.get().clone();
                                    msgs.push(format!("{}: {}", username.get(), input_message.get()));
                                    messages.set(msgs);
                                    input_message.set(String::new());
                                }
                            },
                            "Send"
                        }
                    }
                }
                
                // Quick Actions
                div {
                    style: "display: flex; gap: 10px; margin-top: 10px; flex-wrap: wrap;",
                    button {
                        style: "padding: 10px 15px; background: #e74c3c; color: white; border: none; border-radius: 4px; cursor: pointer;",
                        onclick: move |_| connected.set(false),
                        "Disconnect"
                    }
                    button {
                        style: "padding: 10px 15px; background: #f39c12; color: white; border: none; border-radius: 4px; cursor: pointer;",
                        onclick: move |_| {
                            // TODO: Show groups panel
                        },
                        "Groups"
                    }
                    button {
                        style: "padding: 10px 15px; background: #9b59b6; color: white; border: none; border-radius: 4px; cursor: pointer;",
                        onclick: move |_| {
                            // TODO: Show users panel
                        },
                        "Users"
                    }
                }
            }
        }
    })
}
