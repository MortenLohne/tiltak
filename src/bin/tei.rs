use std::{
    env,
    io::{self, BufRead},
    thread, time,
};

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

    let (sender, receiver) = async_channel::unbounded();

    let executor = smol::LocalExecutor::new();

    let _task = executor.spawn(tei::tei::<_, SmolPlatform>(
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

    loop {
        while executor.try_tick() {}
        thread::sleep(time::Duration::from_millis(1));
    }
}

fn output_callback(output: &str) {
    println!("{}", output);
}
