use std::{
    env,
    io::{self, BufRead},
    mem,
    sync::mpsc,
    thread, time,
};

#[cfg(all(feature = "mimalloc", not(feature = "dhat-heap")))]
use mimalloc::MiMalloc;

#[cfg(all(feature = "mimalloc", not(feature = "dhat-heap")))]
#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

#[cfg(feature = "dhat-heap")]
#[global_allocator]
static ALLOC: dhat::Alloc = dhat::Alloc;

use tiltak::tei;

struct SmolPlatform;

impl tei::Platform for SmolPlatform {
    type Instant = time::Instant;

    fn yield_fn() -> impl std::future::Future {
        smol::future::yield_now()
    }

    fn current_time() -> Self::Instant {
        time::Instant::now()
    }

    fn elapsed_time(start: &Self::Instant) -> time::Duration {
        start.elapsed()
    }
}

pub fn main() {
    let is_slatebot = env::args().any(|arg| arg == "--slatebot");
    let is_cobblebot = env::args().any(|arg| arg == "--cobblebot");
    let is_bench = env::args().any(|arg| arg == "--bench");

    if is_bench {
        bench();
        return;
    }

    let (sender, receiver) = async_channel::unbounded();

    let executor = smol::LocalExecutor::new();

    let task = executor.spawn(tei::tei::<_, SmolPlatform>(
        is_slatebot,
        is_cobblebot,
        receiver,
        &output_callback,
    ));

    thread::spawn(move || {
        for line in io::BufReader::new(io::stdin()).lines().map(Result::unwrap) {
            smol::block_on(async {
                sender.send(line).await.unwrap();
            });
        }
    });

    while !task.is_finished() {
        while executor.try_tick() {}
        thread::sleep(time::Duration::from_millis(1));
    }
}

// Play out an entire game over tei
pub fn bench() {
    #[cfg(feature = "dhat-heap")]
    let _profiler = dhat::Profiler::new_heap();

    let (sender, receiver) = async_channel::bounded(1);

    let (output_sender, output_receiver) = mpsc::channel();

    let callback = move |output: &str| {
        output_sender.send(output.to_string()).unwrap();
    };

    let executor = smol::LocalExecutor::new();

    let task = executor.spawn(tei::tei::<_, SmolPlatform>(
        false, false, receiver, &callback,
    ));

    let start_time = time::Instant::now();

    thread::spawn(move || {
        smol::block_on(async {
            sender.send("tei".to_string()).await.unwrap();
            sender
                .send("setoption name HalfKomi value 4".to_string())
                .await
                .unwrap();
            sender.send("isready".to_string()).await.unwrap();
            while output_receiver.recv().unwrap() != "readyok" {}
            sender.send("teinewgame 6".to_string()).await.unwrap();

            // Tiltak vs Topaz, game #722537 on Playtak
            let move_strings = [
                "a6", "f1", "d3", "c5", "c3", "e5", "d5", "Cd4", "e3", "c6", "Cc4", "d6", "b6",
                "b5", "b4", "d4-", "d4", "e6", "b6>", "e4", "f4", "b6", "a4", "b5-", "d5<", "d2",
                "c2", "b6>", "Sb6", "2c6-", "d1", "e1", "e2", "2d3>", "f1<", "d3", "c4+", "Sc4",
                "3c5>12", "c4+", "b2", "f2", "f1", "3e3-12", "e2>", "Sf3", "f6", "f3-", "f5", "c4",
                "a2", "c1", "a4>", "c4-", "c4", "2c5-", "a5", "3c4<", "b5", "4e1<13", "b6>", "e3",
                "c4", "4b4>", "Se1", "b3", "e1<", "3c1+", "3d1+12", "b6", "2c6<", "a4", "c6",
                "a4+", "b5<", "a6-", "f3", "3f2+12", "3d3<", "e1", "b5", "a6", "b5<", "a6-", "Sa4",
                "4a5>112", "3e5<", "e5", "f5<", "4c2>", "6d5<114", "b3-", "a4>", "f5", "d5", "f5<",
                "d5>", "e4+", "2c5>11", "d5>", "6a5>1113", "2b2<", "6e5-24", "c5+", "3b6>", "3f4<",
                "e3+*", "5d2>", "5c6>14", "d5+", "2b5>11", "Sf5", "5c3>14", "f5<", "4e4<31*",
                "e2+*", "6c4-", "d2>", "3b4>12", "5e5<122", "4d4+13", "6e3+411*", "3c3>12",
                "5e4<2111", "3c3+111", "e6<*", "5e3-", "c2", "6e2<", "6d6<", "6d2<", "4c6-22",
                "2d5<", "f4",
            ];

            // Iterator over all white's move indexes
            for i in (0..(move_strings.len() / 2)).map(|i| i * 2) {
                sender
                    .send(format!(
                        "position startpos moves {}",
                        move_strings[0..i].join(" ")
                    ))
                    .await
                    .unwrap();
                sender.send("go nodes 100000".to_string()).await.unwrap();
                while let Ok(message) = output_receiver.recv() {
                    if message.starts_with("info") {
                        println!("{}: {}", i / 2 + 1, message);
                    }
                    if message.starts_with("bestmove") {
                        break;
                    }
                }
            }
            mem::drop(sender)
        });
    });

    while !task.is_finished() {
        while executor.try_tick() {}
        thread::sleep(time::Duration::from_millis(1));
    }
    println!("Took {:.2}s", start_time.elapsed().as_secs_f32());
}

fn output_callback(output: &str) {
    println!("{}", output);
}
