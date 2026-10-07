use super::*;
use std::{io, net::SocketAddr, time::Duration};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
};

async fn bounded<T>(future: impl std::future::Future<Output = T>) -> T {
    tokio::time::timeout(Duration::from_secs(3), future)
        .await
        .expect("test deadline")
}
async fn connected(app: &Application, id: u64) {
    bounded(async {
        loop {
            let poll = app.poll(id).await.unwrap();
            if poll.status.phase == Phase::Connected {
                return;
            }
            assert!(!poll.finished, "{:?}", poll.status);
        }
    })
    .await;
}
async fn drain(app: &Application, id: u64) -> Vec<PresentationEvent> {
    bounded(async {
        let mut result = Vec::new();
        loop {
            let poll = app.poll(id).await.unwrap();
            result.extend(poll.events);
            if poll.finished {
                return result;
            }
        }
    })
    .await
}

#[test]
fn endpoints_validate_ports_literals_and_dns_labels() {
    for host in [
        "localhost",
        "mud.example",
        "mud.example.",
        "127.0.0.1",
        "::1",
        "[::1]",
    ] {
        assert!(Endpoint::parse(host, 4000).is_ok());
    }
    for host in [
        "",
        "https://mud",
        "mud/path",
        "a b",
        "a\n",
        "-bad",
        "bad-",
        "a..b",
        "é.test",
        "foo:23",
        "[bad]",
    ] {
        assert_eq!(Endpoint::parse(host, 4000), Err(AppError::InvalidHost));
    }
    assert_eq!(
        Endpoint::parse(&"a".repeat(254), 4000),
        Err(AppError::InvalidHost)
    );
    for port in [0, 65536, u32::MAX] {
        assert_eq!(
            Endpoint::parse("localhost", port),
            Err(AppError::InvalidPort)
        );
    }
}

#[tokio::test]
async fn connection_commands_prompts_eof_and_replacement_are_isolated() {
    let app = Application::new();
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let id = app
        .start(
            "127.0.0.1",
            u32::from(listener.local_addr().unwrap().port()),
        )
        .unwrap();
    assert_eq!(app.start("localhost", 4000), Err(AppError::Busy));
    let (mut peer, _) = bounded(listener.accept()).await.unwrap();
    connected(&app, id).await;
    peer.write_all(b"Name: \xe2\x82").await.unwrap();
    let poll = bounded(app.poll(id)).await.unwrap();
    assert_eq!(poll.events, [PresentationEvent::Text("Name: ".into())]);
    assert_eq!(
        app.send_line(id, "bad\nline").await,
        Err(AppError::InvalidLine)
    );
    assert_eq!(
        app.send_line(id, &"x".repeat(16383)).await,
        Err(AppError::TooLarge)
    );
    app.send_line(id, "  hello  ").await.unwrap();
    let mut input = [0; 11];
    bounded(peer.read_exact(&mut input)).await.unwrap();
    assert_eq!(&input, b"  hello  \r\n");
    peer.shutdown().await.unwrap();
    assert_eq!(drain(&app, id).await, [PresentationEvent::Text("�".into())]);
    assert_eq!(bounded(peer.read(&mut [0])).await.unwrap(), 0);
    let next = app
        .start(
            "127.0.0.1",
            u32::from(listener.local_addr().unwrap().port()),
        )
        .unwrap();
    assert_ne!(id, next);
    assert_eq!(app.poll(id).await.unwrap_err(), AppError::StaleSession);
    app.disconnect(next).await.unwrap();
    app.shutdown().await;
    assert_eq!(app.start("localhost", 4000), Err(AppError::ShuttingDown));
}

#[tokio::test]
async fn presentation_failure_keeps_prior_text_and_closes_socket() {
    let app = Application::new();
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let id = app
        .start(
            "127.0.0.1",
            u32::from(listener.local_addr().unwrap().port()),
        )
        .unwrap();
    let (mut peer, _) = bounded(listener.accept()).await.unwrap();
    peer.write_all(b"before\x1b[\xff\xffafter").await.unwrap();
    assert_eq!(
        drain(&app, id).await,
        [PresentationEvent::Text("before".into())]
    );
    assert!(
        app.poll(id)
            .await
            .unwrap()
            .status
            .message
            .starts_with("Presentation error:")
    );
    assert_eq!(bounded(peer.read(&mut [0])).await.unwrap(), 0);
}

#[tokio::test]
async fn output_pressure_is_bounded_and_cancellation_bypasses_it() {
    let app = Application::new();
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let id = app
        .start(
            "127.0.0.1",
            u32::from(listener.local_addr().unwrap().port()),
        )
        .unwrap();
    let (mut peer, _) = bounded(listener.accept()).await.unwrap();
    connected(&app, id).await;
    peer.write_all(&vec![7; 4096]).await.unwrap();
    let connection = app.connection(id).unwrap();
    bounded(async {
        loop {
            if lock(&connection.buffer).events.len() == OUTPUT_EVENTS {
                break;
            }
            tokio::task::yield_now().await;
        }
    })
    .await;
    assert!(lock(&connection.buffer).bytes <= OUTPUT_BYTES);
    let poll = app.poll(id).await.unwrap();
    assert_eq!(poll.events.len(), POLL_EVENTS);
    app.send_line(id, "ping").await.unwrap();
    let mut line = [0; 6];
    bounded(peer.read_exact(&mut line)).await.unwrap();
    assert_eq!(&line, b"ping\r\n");
    bounded(app.disconnect(id)).await.unwrap();
    assert_eq!(bounded(peer.read(&mut [0])).await.unwrap(), 0);
}

#[tokio::test(start_paused = true)]
async fn dns_deadline_retains_slot_and_numeric_addresses_bypass_it() {
    let release = Arc::new(Notify::new());
    let release_lookup = release.clone();
    let resolver = Resolver::injected(Arc::new(move |_, _| {
        let release = release_lookup.clone();
        Box::pin(async move {
            release.notified().await;
            Ok(vec!["127.0.0.1:1".parse().unwrap()])
        })
    }));
    let app = Application {
        registry: Mutex::new(Registry {
            next: 1,
            current: None,
            shutting_down: false,
        }),
        resolver: resolver.clone(),
    };
    let id = app.start("localhost", 4000).unwrap();
    tokio::task::yield_now().await;
    tokio::task::yield_now().await;
    tokio::time::advance(Duration::from_secs(11)).await;
    app.connection(id).unwrap().wait().await;
    assert!(
        app.poll(id)
            .await
            .unwrap()
            .status
            .message
            .contains("timed out")
    );
    assert!(
        resolver
            .resolve(Endpoint::parse("localhost", 4000).unwrap())
            .await
            .unwrap_err()
            .contains("still finishing")
    );
    assert!(
        resolver
            .resolve(Endpoint::parse("127.0.0.1", 4000).unwrap())
            .await
            .is_ok()
    );
    release.notify_one();
    tokio::task::yield_now().await;
}

#[tokio::test]
async fn injected_dns_empty_failure_and_address_fallback() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let good = listener.local_addr().unwrap();
    let refused = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let bad = refused.local_addr().unwrap();
    drop(refused);
    let resolver = Resolver::injected(Arc::new(move |_, _| {
        Box::pin(async move { Ok(vec![bad, good, good]) })
    }));
    let app = Application {
        registry: Mutex::new(Registry {
            next: 1,
            current: None,
            shutting_down: false,
        }),
        resolver,
    };
    let id = app.start("test.invalid", 4000).unwrap();
    let (mut peer, _) = bounded(listener.accept()).await.unwrap();
    connected(&app, id).await;
    bounded(app.shutdown()).await;
    assert_eq!(bounded(peer.read(&mut [0])).await.unwrap(), 0);
    for result in [
        Ok(Vec::<SocketAddr>::new()),
        Err(io::Error::other("private details")),
    ] {
        let cell = Arc::new(Mutex::new(Some(result)));
        let resolver = Resolver::injected(Arc::new(move |_, _| {
            let result = lock(&cell).take().unwrap();
            Box::pin(async move { result })
        }));
        let error = resolver
            .resolve(Endpoint::parse("test.invalid", 4000).unwrap())
            .await
            .unwrap_err();
        assert!(!error.contains("private"));
    }
}

#[tokio::test]
async fn text_pressure_poll_limits_and_exact_command_bound() {
    let app = Application::new();
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let id = app
        .start(
            "127.0.0.1",
            u32::from(listener.local_addr().unwrap().port()),
        )
        .unwrap();
    let (mut peer, _) = bounded(listener.accept()).await.unwrap();
    connected(&app, id).await;
    bounded(peer.write_all(&vec![b'x'; OUTPUT_BYTES + 8192]))
        .await
        .unwrap();
    let connection = app.connection(id).unwrap();
    bounded(async {
        loop {
            if lock(&connection.buffer).bytes > OUTPUT_BYTES - 4096 {
                break;
            }
            tokio::task::yield_now().await;
        }
    })
    .await;
    assert!(lock(&connection.buffer).bytes <= OUTPUT_BYTES);
    let mut received = 0;
    while received < OUTPUT_BYTES + 8192 {
        let poll = bounded(app.poll(id)).await.unwrap();
        let bytes: usize = poll.events.iter().map(event_bytes).sum();
        assert!(bytes <= POLL_BYTES);
        assert!(poll.events.len() <= POLL_EVENTS);
        received += bytes;
    }
    app.send_line(id, "").await.unwrap();
    let mut empty = [0; 2];
    bounded(peer.read_exact(&mut empty)).await.unwrap();
    assert_eq!(&empty, b"\r\n");
    app.send_line(id, &"é".repeat(8191)).await.unwrap();
    let mut command = vec![0; MAX_SEND_BYTES];
    bounded(peer.read_exact(&mut command)).await.unwrap();
    assert_eq!(&command[MAX_SEND_BYTES - 2..], b"\r\n");
    assert_eq!(
        app.send_line(id, "hidden\u{85}").await,
        Err(AppError::InvalidLine)
    );
    bounded(app.shutdown()).await;
    assert_eq!(bounded(peer.read(&mut [0])).await.unwrap(), 0);
}

#[tokio::test]
async fn only_one_empty_poll_waits_and_shutdown_wakes_it() {
    let app = Arc::new(Application::new());
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let id = app
        .start(
            "127.0.0.1",
            u32::from(listener.local_addr().unwrap().port()),
        )
        .unwrap();
    let (mut peer, _) = bounded(listener.accept()).await.unwrap();
    connected(&app, id).await;
    // Consume any coalesced status notification before waiting for new output.
    app.poll(id).await.unwrap();
    let first = app.poll(id);
    tokio::pin!(first);
    tokio::select! {
        result = &mut first => panic!("empty poll returned early: {result:?}"),
        _ = tokio::task::yield_now() => {}
    }
    assert_eq!(app.poll(id).await.unwrap_err(), AppError::PollBusy);
    bounded(app.shutdown()).await;
    assert!(bounded(first).await.unwrap().finished);
    assert_eq!(bounded(peer.read(&mut [0])).await.unwrap(), 0);
}

#[tokio::test]
async fn cancelling_dns_is_immediate_but_slot_lives_until_lookup_finishes() {
    let release = Arc::new(Notify::new());
    let entered = Arc::new(Notify::new());
    let lookup_release = release.clone();
    let lookup_entered = entered.clone();
    let resolver = Resolver::injected(Arc::new(move |_, _| {
        let release = lookup_release.clone();
        let entered = lookup_entered.clone();
        Box::pin(async move {
            entered.notify_one();
            release.notified().await;
            Ok(vec!["127.0.0.1:1".parse().unwrap()])
        })
    }));
    let mut app = Application::new();
    app.resolver = resolver.clone();
    let id = app.start("test.invalid", 4000).unwrap();
    bounded(entered.notified()).await;
    bounded(app.disconnect(id)).await.unwrap();
    assert!(app.poll(id).await.unwrap().finished);
    let next = app.start("test.invalid", 4000).unwrap();
    bounded(app.connection(next).unwrap().wait()).await;
    assert!(
        app.poll(next)
            .await
            .unwrap()
            .status
            .message
            .contains("still finishing")
    );
    release.notify_one();
    tokio::task::yield_now().await;
    app.shutdown().await;
}

#[tokio::test]
async fn dns_candidates_are_distinct_ordered_and_capped() {
    let resolver = Resolver::injected(Arc::new(|_, _| {
        Box::pin(async {
            Ok((1..=20)
                .flat_map(|port| [SocketAddr::from(([127, 0, 0, 1], port)); 2])
                .collect())
        })
    }));
    let addresses = resolver
        .resolve(Endpoint::parse("test.invalid", 4000).unwrap())
        .await
        .unwrap();
    assert_eq!(
        addresses.iter().map(SocketAddr::port).collect::<Vec<_>>(),
        (1..=8).collect::<Vec<_>>()
    );
}

#[tokio::test]
async fn full_command_queue_rejects_without_replay() {
    use std::{future::Future, task::Poll};
    let app = Application::new();
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let id = app
        .start(
            "127.0.0.1",
            u32::from(listener.local_addr().unwrap().port()),
        )
        .unwrap();
    let (mut peer, _) = bounded(listener.accept()).await.unwrap();
    connected(&app, id).await;
    let mut pending = Vec::new();
    // Poll each request once without yielding to the connection actor.
    for _ in 0..8 {
        let mut send = Box::pin(app.send_line(id, "x"));
        std::future::poll_fn(|cx| {
            assert!(send.as_mut().poll(cx).is_pending());
            Poll::Ready(())
        })
        .await;
        pending.push(send);
    }
    assert_eq!(
        app.send_line(id, "never replay").await,
        Err(AppError::QueueFull)
    );
    for send in pending {
        bounded(send).await.unwrap();
    }
    let mut bytes = [0; 24];
    bounded(peer.read_exact(&mut bytes)).await.unwrap();
    assert_eq!(&bytes, b"x\r\nx\r\nx\r\nx\r\nx\r\nx\r\nx\r\nx\r\n");
    bounded(app.shutdown()).await;
    assert_eq!(bounded(peer.read(&mut [0])).await.unwrap(), 0);
}

#[tokio::test]
async fn option_snapshots_bypass_full_output_and_viewports_validate_identity() {
    bounded(async {
        let app = Application::new();
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let id = app
            .start(
                "127.0.0.1",
                u32::from(listener.local_addr().unwrap().port()),
            )
            .unwrap();
        assert_eq!(
            app.update_viewport(id, 0, 24),
            Err(AppError::InvalidViewport)
        );
        assert_eq!(
            app.update_viewport(id, 80, 65536),
            Err(AppError::InvalidViewport)
        );
        app.update_viewport(id, 120, 40).unwrap();
        let (mut peer, _) = listener.accept().await.unwrap();
        connected(&app, id).await;
        peer.write_all(b"\xff\xfd\x1f").await.unwrap();
        let mut naws = [0; 12];
        peer.read_exact(&mut naws).await.unwrap();
        assert_eq!(&naws, b"\xff\xfb\x1f\xff\xfa\x1f\0\x78\0\x28\xff\xf0");
        peer.write_all(&[7; 4096]).await.unwrap();
        let connection = app.connection(id).unwrap();
        while lock(&connection.buffer).events.len() != OUTPUT_EVENTS {
            tokio::task::yield_now().await;
        }
        peer.write_all(b"\xff\xfb\x01\xff\xfc\x01").await.unwrap();
        while {
            let options = lock(&connection.buffer).options;
            options.masking_generation != 1 || options.remote_echo
        } {
            tokio::task::yield_now().await;
        }
        let snapshot = app.poll(id).await.unwrap();
        assert_eq!(snapshot.events.len(), POLL_EVENTS);
        assert_eq!(snapshot.options.masking_generation, 1);
        assert!(!snapshot.options.remote_echo);
        let mut replies = [0; 6];
        peer.read_exact(&mut replies).await.unwrap();
        assert_eq!(&replies, b"\xff\xfd\x01\xff\xfe\x01");
        app.update_viewport(id, 255, 255).unwrap();
        let mut resized = [0; 11];
        peer.read_exact(&mut resized).await.unwrap();
        assert_eq!(&resized, b"\xff\xfa\x1f\0\xff\xff\0\xff\xff\xff\xf0");
        app.disconnect(id).await.unwrap();
        assert_eq!(peer.read(&mut [0]).await.unwrap(), 0);
        assert_eq!(app.update_viewport(id, 1, 1), Err(AppError::Closed));
        let next = app
            .start(
                "127.0.0.1",
                u32::from(listener.local_addr().unwrap().port()),
            )
            .unwrap();
        assert_eq!(app.update_viewport(id, 80, 24), Err(AppError::StaleSession));
        let (_peer, _) = listener.accept().await.unwrap();
        connected(&app, next).await;
        assert_eq!(
            app.poll(next).await.unwrap().options,
            OptionSnapshot::default()
        );
        app.shutdown().await;
    })
    .await;
}
