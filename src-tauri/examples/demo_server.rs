//! Development-only loopback peer. No credentials, input logging, or public bind.
use solidify_client::protocols::telnet::{TelnetDecoder, TelnetEvent};
use std::{io, time::Duration};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
};

#[tokio::main(flavor = "current_thread")]
async fn main() -> io::Result<()> {
    let mut args = std::env::args().skip(1);
    let port = match args.next().as_deref() {
        None => 4000,
        Some("--port") => args
            .next()
            .and_then(|v| v.parse::<u16>().ok())
            .ok_or_else(|| io::Error::other("Usage: demo_server [--port 0..65535]"))?,
        _ => return Err(io::Error::other("Usage: demo_server [--port 0..65535]")),
    };
    if args.next().is_some() {
        return Err(io::Error::other("Unexpected argument"));
    }
    let listener = TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, port)).await?;
    println!("solidify demo listening on {}", listener.local_addr()?);
    println!(
        "One client at a time. Commands: help, styles, unicode, controls, markup, burst, malformed, quit."
    );
    loop {
        let (mut socket, _) = listener.accept().await?;
        // One active connection and a five-minute idle deadline bound demo resources.
        let _ = serve(&mut socket).await;
        let _ = socket.shutdown().await;
    }
}
async fn write(socket: &mut TcpStream, bytes: &[u8]) -> io::Result<()> {
    tokio::time::timeout(Duration::from_secs(5), socket.write_all(bytes))
        .await
        .map_err(|_| io::Error::other("Demo write deadline"))?
}
async fn serve(socket: &mut TcpStream) -> io::Result<()> {
    for part in [
        b"\x1b[".as_slice(),
        b"1;36msolidify local demo\x1b[0m\r\nStreaming Unicode: \xf0\x9f",
        b"\xff\xfb\x01",
        b"\x8c\x8d caf\xc3",
        b"\xa9\r\nType help for commands.\r\n\x1b[32mdemo> \x1b[0m",
    ] {
        write(socket, part).await?;
        tokio::time::sleep(Duration::from_millis(60)).await;
    }
    let mut decoder = TelnetDecoder::new();
    let mut input = Vec::new();
    let mut scratch = [0; 1024];
    loop {
        let count = tokio::time::timeout(Duration::from_secs(300), socket.read(&mut scratch))
            .await
            .map_err(|_| io::Error::other("Demo idle deadline"))??;
        if count == 0 {
            return Ok(());
        }
        let mut data = Vec::new();
        decoder
            .feed(&scratch[..count], |event| {
                if let TelnetEvent::Data(bytes) = event {
                    data.extend(bytes)
                }
            })
            .map_err(|_| io::Error::other("Invalid Telnet input"))?;
        for byte in data {
            if input.len() >= 16384 {
                return Err(io::Error::other("Demo line limit"));
            }
            if byte != b'\n' {
                input.push(byte);
                continue;
            }
            if input.last() == Some(&b'\r') {
                input.pop();
            }
            let command = String::from_utf8_lossy(&input);
            let response: &[u8] = match command.as_ref() {
                "help" => b"\r\nCommands: styles, unicode, controls, markup, burst, malformed, quit.\r\n",
                "styles" => b"\r\n\x1b[31mRed \x1b[32mGreen \x1b[34mBlue\x1b[0m\r\n\x1b[1mBold\x1b[22m \x1b[3mItalic\x1b[23m \x1b[4mUnderline\x1b[24m \x1b[7mInverse\x1b[27m\r\n",
                "unicode" => "\r\ncafé · 中文 · 🌍 · e\u{301}\r\n".as_bytes(),
                "controls" => b"\r\nprogress 10%\rprogress 100%\r\nBackspacex\x08!\r\nA\tB\x07\r\n",
                "markup" => b"\r\n<script>alert('literal text')</script>\r\n<a href='https://example.invalid'>not a link</a>\r\n",
                "malformed" => b"\r\nText before malformed ANSI.\x1b[\x18discarded",
                "quit" => {
                    write(socket, b"\r\nGoodbye.\r\n").await?;
                    return Ok(());
                }
                "burst" => {
                    for _ in 0..2500 {
                        write(socket, b"A bounded line of demo output.\r\n").await?;
                    }
                    b"Burst complete.\r\n"
                }
                _ => b"\r\nCommand received. Type help for available demonstrations.\r\n",
            };
            write(socket, response).await?;
            write(socket, b"\x1b[32mdemo> \x1b[0m").await?;
            input.clear();
        }
    }
}
