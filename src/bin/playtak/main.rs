use bufstream::BufStream;
use std::io::{BufRead, Result, Write};
use std::net;
use std::net::TcpStream;

pub fn main() -> Result<()> {
    Ok(())
}

pub fn connect(client_name: &str, user: &str, pwd: &str) -> Result<BufStream<TcpStream>> {
    let mut connection = dial()?;
    login(&mut connection, client_name, user, pwd)?;
    Ok(connection)
}

fn dial() -> Result<BufStream<TcpStream>> {
    net::TcpStream::connect("playtak.com:10000").map(BufStream::new)
}

fn login(conn: &mut BufStream<TcpStream>, client_name: &str, user: &str, pwd: &str) -> Result<()> {
    let mut line = String::new();
    loop {
        conn.read_line(&mut line)?;
        println!("{}", line);
        if line.starts_with("Login") {
            break;
        }
        line.clear();
    }
    println!("Logging in: ");
    writeln!(conn, "client {}", client_name)?;
    writeln!(conn, "Login {} {}", user, pwd)?;
    conn.flush()?;
    Ok(())
}
