mod store;

use std::io::{self, Write};
use store::Store;

const HELP: &str = "Commands: SET <key> <value> | GET <key> | DEL <key> | HELP | EXIT";

fn main() {
    let mut store = Store::new();

    println!("mini-redis v0.1");
    println!("{HELP}");

    loop {
        print!("> ");
        // print! doesn't flush, so we do it ourselves or the prompt won't appear.
        if io::stdout().flush().is_err() {
            break;
        }

        let mut line = String::new();
        match io::stdin().read_line(&mut line) {
            Ok(0) => break, // 0 bytes means EOF (Ctrl-D)
            Ok(_) => {}
            Err(e) => {
                eprintln!("failed to read input: {e}");
                break;
            }
        }

        // Split on any whitespace. For now values can't contain spaces —
        // fixing that is a good little exercise later.
        let args: Vec<&str> = line.trim().split_whitespace().collect();
        let Some(command) = args.first() else {
            continue;
        };

        match command.to_ascii_uppercase().as_str() {
            "SET" => match args.as_slice() {
                [_, key, value] => {
                    store.set((*key).to_string(), (*value).to_string());
                    println!("OK");
                }
                _ => println!("usage: SET <key> <value>"),
            },
            "GET" => match args.as_slice() {
                [_, key] => match store.get(key) {
                    Some(value) => println!("\"{value}\""),
                    None => println!("(nil)"),
                },
                _ => println!("usage: GET <key>"),
            },
            "DEL" => match args.as_slice() {
                [_, key] => {
                    if store.remove(key).is_some() {
                        println!("1");
                    } else {
                        println!("0");
                    }
                }
                _ => println!("usage: DEL <key>"),
            },
            "HELP" => println!("{HELP}"),
            "EXIT" | "QUIT" => break,
            other => println!("unknown command: {other}"),
        }
    }

    println!("bye!");
}
