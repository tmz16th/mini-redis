use std::{
    io::{self, BufRead, BufReader, Write},
    net::TcpStream,
};

const ADDR: &str = "127.0.0.1:6379";

fn main() -> io::Result<()> {
    let stream = match TcpStream::connect(ADDR) {
        Ok(stream) => stream,
        Err(e) => {
            eprintln!("couldn't connect to {ADDR}: {e}");
            eprintln!("start the server first, in another terminal: cargo run");
            return Ok(());
        }
    };

    let mut writer = stream.try_clone()?;
    let mut server = BufReader::new(stream);
    let stdin = io::stdin();
    let mut input = String::new();

    println!("connected to {ADDR}");
    println!("commands: PING | SET <key> <value> | GET <key> | DEL <key> | QUIT");

    loop {
        print!("> ");
        io::stdout().flush()?;

        input.clear();
        if stdin.read_line(&mut input)? == 0 {
            break;
        }

        let command = input.trim();
        if command.is_empty() {
            continue;
        }

        writer.write_all(command.as_bytes())?;
        writer.write_all(b"\n")?;
        writer.flush()?;

        let mut response = String::new();
        if server.read_line(&mut response)? == 0 {
            eprintln!("server closed the connection");
            break;
        }
        print!("{response}");

        if command.eq_ignore_ascii_case("quit") {
            break;
        }
    }

    println!("disconnected");
    Ok(())
}
