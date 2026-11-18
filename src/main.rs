mod store;

use std::{
    io::{BufRead, BufReader, Write},
    net::{TcpListener, TcpStream},
    sync::{Arc, RwLock},
    thread,
};

use store::Store;

const ADDR: &str = "127.0.0.1:6379";
type SharedStore = Arc<RwLock<Store>>;

fn main() -> std::io::Result<()> {
    // Arc = many threads can own the same store.
    // RwLock = many readers at once, but only one writer at a time.
    let store: SharedStore = Arc::new(RwLock::new(Store::new()));

    let listener = TcpListener::bind(ADDR)?;
    println!("mini-redis listening on {ADDR}");

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                let store = Arc::clone(&store);
                thread::spawn(move || {
                    if let Err(e) = handle_client(stream, store) {
                        eprintln!("client error: {e}");
                    }
                });
            }
            Err(e) => eprintln!("failed to accept connection: {e}"),
        }
    }

    Ok(())
}

fn handle_client(stream: TcpStream, store: SharedStore) -> std::io::Result<()> {
    let peer = stream.peer_addr()?;
    println!("{peer} connected");

    // A clone lets us read from one half and write to the other independently.
    let mut reader = BufReader::new(stream.try_clone()?);
    let mut writer = stream;
    let mut line = String::new();

    loop {
        line.clear();
        if reader.read_line(&mut line)? == 0 {
            break; // client closed the connection
        }

        let args: Vec<&str> = line.trim().split_whitespace().collect();
        let Some(command) = args.first() else {
            writer.write_all(b"ERR empty command\n")?;
            continue;
        };

        let response = match command.to_ascii_uppercase().as_str() {
            "PING" => "PONG\n".to_string(),
            "SET" => match args.as_slice() {
                [_, key, value] => {
                    // write() blocks until no readers are active.
                    let mut s = store.write().unwrap();
                    s.set((*key).to_string(), (*value).to_string());
                    "OK\n".to_string()
                }
                _ => "ERR usage: SET <key> <value>\n".to_string(),
            },
            "GET" => match args.as_slice() {
                [_, key] => {
                    // read() allows other readers in parallel.
                    let s = store.read().unwrap();
                    match s.get(key) {
                        Some(value) => format!("{value}\n"),
                        None => "(nil)\n".to_string(),
                    }
                }
                _ => "ERR usage: GET <key>\n".to_string(),
            },
            "DEL" => match args.as_slice() {
                [_, key] => {
                    let mut s = store.write().unwrap();
                    if s.remove(key).is_some() {
                        "1\n".to_string()
                    } else {
                        "0\n".to_string()
                    }
                }
                _ => "ERR usage: DEL <key>\n".to_string(),
            },
            "QUIT" => {
                writer.write_all(b"bye\n")?;
                break;
            }
            other => format!("ERR unknown command: {other}\n"),
        };

        writer.write_all(response.as_bytes())?;
    }

    println!("{peer} disconnected");
    Ok(())
}
