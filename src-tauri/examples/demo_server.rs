//! Development-only loopback peer. No credentials, input logging, or public bind.
use solidify_client::protocols::{
    options::{ECHO, NAWS, SGA, TTYPE, TerminalSize},
    telnet::{OptionDirection, OptionPolicy, TelnetDecoder, TelnetEvent, TelnetNegotiator, encode},
};
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
        "One client at a time. Commands: help, styles, unicode, controls, markup, burst, protocol, mask, options-off, options-on, malformed, quit."
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
async fn frame(socket: &mut TcpStream, event: TelnetEvent) -> io::Result<()> {
    let mut bytes = Vec::new();
    encode(&event, |part| bytes.extend_from_slice(part))
        .map_err(|_| io::Error::other("Invalid demo frame"))?;
    write(socket, &bytes).await
}
struct PeerOptions {
    q: TelnetNegotiator,
    identified: bool,
    size: Option<TerminalSize>,
}
impl PeerOptions {
    fn new() -> Self {
        Self {
            q: TelnetNegotiator::new(OptionPolicy::new(&[ECHO, SGA], &[TTYPE, NAWS, SGA])),
            identified: false,
            size: None,
        }
    }
    async fn request(
        &mut self,
        socket: &mut TcpStream,
        direction: OptionDirection,
        option: u8,
        enabled: bool,
    ) -> io::Result<()> {
        if let Some(command) = self
            .q
            .request(direction, option, enabled)
            .expect("implemented demo options")
        {
            frame(socket, command.into()).await?;
        }
        Ok(())
    }
    async fn receive(&mut self, socket: &mut TcpStream, event: &TelnetEvent) -> io::Result<()> {
        let before = self.q.is_enabled(OptionDirection::Remote, TTYPE);
        match event {
            TelnetEvent::Negotiation { verb, option } => {
                if let Some(reply) = self.q.receive(*verb, *option) {
                    frame(socket, reply.into()).await?;
                }
                let after = self.q.is_enabled(OptionDirection::Remote, TTYPE);
                if !after {
                    self.identified = false;
                }
                if !before && after {
                    frame(
                        socket,
                        TelnetEvent::Subnegotiation {
                            option: TTYPE,
                            payload: vec![1],
                        },
                    )
                    .await?;
                }
                if !self.q.is_enabled(OptionDirection::Remote, NAWS) {
                    self.size = None;
                }
            }
            TelnetEvent::Subnegotiation {
                option: TTYPE,
                payload,
            } if before => {
                self.identified = payload == b"\0SOLIDIFY";
            }
            TelnetEvent::Subnegotiation {
                option: NAWS,
                payload,
            } if payload.len() == 4 && self.q.is_enabled(OptionDirection::Remote, NAWS) => {
                self.size = Some(TerminalSize {
                    columns: u16::from_be_bytes([payload[0], payload[1]]),
                    rows: u16::from_be_bytes([payload[2], payload[3]]),
                });
            }
            _ => {}
        }
        Ok(())
    }
    async fn toggle(&mut self, socket: &mut TcpStream, enabled: bool) -> io::Result<()> {
        for (direction, option) in [
            (OptionDirection::Remote, TTYPE),
            (OptionDirection::Remote, NAWS),
            (OptionDirection::Remote, SGA),
            (OptionDirection::Local, SGA),
        ] {
            self.request(socket, direction, option, enabled).await?;
        }
        if !enabled {
            self.request(socket, OptionDirection::Local, ECHO, false)
                .await?;
        }
        Ok(())
    }
}
async fn serve(socket: &mut TcpStream) -> io::Result<()> {
    let mut options = PeerOptions::new();
    options.toggle(socket, true).await?;
    for part in [
        b"\x1b[".as_slice(),
        b"1;36msolidify local demo\x1b[0m\r\nStreaming Unicode: \xf0\x9f",
        b"\xff\xfb\x03",
        b"\x8c\x8d caf\xc3",
        b"\xa9\r\nType help for commands.\r\n\x1b[32mdemo> \x1b[0m",
    ] {
        write(socket, part).await?;
        tokio::time::sleep(Duration::from_millis(60)).await;
    }
    let mut decoder = TelnetDecoder::new();
    let mut input = Vec::new();
    let mut masked_response = false;
    let mut scratch = [0; 1024];
    loop {
        let count = tokio::time::timeout(Duration::from_secs(300), socket.read(&mut scratch))
            .await
            .map_err(|_| io::Error::other("Demo idle deadline"))??;
        if count == 0 {
            return Ok(());
        }
        let mut events = Vec::new();
        decoder
            .feed(&scratch[..count], |event| events.push(event))
            .map_err(|_| io::Error::other("Invalid Telnet input"))?;
        for event in events {
            options.receive(socket, &event).await?;
            let TelnetEvent::Data(data) = event else {
                continue;
            };
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
                if masked_response {
                    input.fill(0);
                    input.clear();
                    masked_response = false;
                    options
                        .request(socket, OptionDirection::Local, ECHO, false)
                        .await?;
                    write(
                        socket,
                        b"\r\nInput accepted and discarded.\r\n\x1b[32mdemo> \x1b[0m",
                    )
                    .await?;
                    continue;
                }
                let command = String::from_utf8_lossy(&input);
                let response: &[u8] = match command.as_ref() {
                    "help" => b"\r\nCommands: styles, unicode, controls, markup, burst, protocol, mask, options-off, options-on, malformed, quit.\r\n",
                    "protocol" => {
                        let identity = if options.identified { "SOLIDIFY" } else { "unavailable" };
                        let dimensions = options.size.map(|s| format!("{} columns x {} rows", s.columns, s.rows)).unwrap_or_else(|| "unavailable".into());
                        write(socket, format!("\r\nTerminal: {identity}; viewport: {dimensions}.\r\n").as_bytes()).await?;
                        b""
                    }
                    "mask" => {
                        options.request(socket, OptionDirection::Local, ECHO, true).await?;
                        masked_response = true;
                        input.clear();
                        write(socket, b"\r\nEnter synthetic input (discarded): ").await?;
                        continue;
                    }
                    "options-off" => { options.toggle(socket, false).await?; b"\r\nOptions disabled.\r\n" }
                    "options-on" => { options.toggle(socket, true).await?; b"\r\nOptions requested. Use protocol to inspect.\r\n" }
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use solidify_client::{application::Application, protocols::presentation::PresentationEvent};

    async fn prompt(app: &Application, id: u64) -> String {
        let mut text = String::new();
        loop {
            let poll = app.poll(id).await.unwrap();
            for event in poll.events {
                if let PresentationEvent::Text(part) = event {
                    text.push_str(&part);
                }
            }
            assert!(text.len() < 65536);
            if text.contains("demo> ") || text.contains("Enter synthetic input (discarded): ") {
                return text;
            }
            assert!(!poll.finished, "demo closed before prompt");
        }
    }
    async fn report(app: &Application, id: u64, expected: &str) {
        for _ in 0..10 {
            app.send_line(id, "protocol").await.unwrap();
            if prompt(app, id).await.contains(expected) {
                return;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        panic!("demo metadata did not settle");
    }
    #[tokio::test]
    async fn demo_options_masking_resize_and_eof_use_real_application_path() {
        tokio::time::timeout(Duration::from_secs(8), async {
            let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
            let app = Application::new();
            let id = app
                .start(
                    "127.0.0.1",
                    u32::from(listener.local_addr().unwrap().port()),
                )
                .unwrap();
            let server = async {
                let (mut socket, _) = listener.accept().await.unwrap();
                serve(&mut socket).await.unwrap();
                socket.shutdown().await.unwrap();
            };
            let client = async {
                assert!(prompt(&app, id).await.contains("🌍 café"));
                report(&app, id, "Terminal: SOLIDIFY").await;
                app.update_viewport(id, 255, 50).unwrap();
                report(&app, id, "255 columns x 50 rows").await;
                app.send_line(id, "mask").await.unwrap();
                prompt(&app, id).await;
                let state = app.poll(id).await.unwrap().options;
                assert!(state.remote_echo);
                assert_eq!(state.masking_generation, 1);
                app.send_line(id, "synthetic-demo-value").await.unwrap();
                let output = prompt(&app, id).await;
                assert!(output.contains("Input accepted and discarded."));
                assert!(!output.contains("synthetic-demo-value"));
                assert!(!app.poll(id).await.unwrap().options.remote_echo);
                app.send_line(id, "options-off").await.unwrap();
                prompt(&app, id).await;
                report(&app, id, "Terminal: unavailable; viewport: unavailable").await;
                app.update_viewport(id, 123, 45).unwrap();
                app.send_line(id, "options-on").await.unwrap();
                prompt(&app, id).await;
                report(
                    &app,
                    id,
                    "Terminal: SOLIDIFY; viewport: 123 columns x 45 rows",
                )
                .await;
                app.send_line(id, "quit").await.unwrap();
                while !app.poll(id).await.unwrap().finished {}
                app.shutdown().await;
            };
            tokio::join!(server, client);
        })
        .await
        .expect("demo test deadline");
    }
}
