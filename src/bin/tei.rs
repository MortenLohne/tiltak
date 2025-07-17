use std::{
    env,
    io::{self, BufRead},
    thread,
    time::Duration,
};

use tiltak::tei;

pub fn main() {
    let is_slatebot = env::args().any(|arg| arg == "--slatebot");
    let is_cobblebot = env::args().any(|arg| arg == "--cobblebot");

    let (sender, receiver) = async_channel::unbounded();

    let executor = smol::LocalExecutor::new();

    let _task = executor.spawn(tei::tei(
        is_slatebot,
        is_cobblebot,
        receiver,
        &output_callback,
        &smol::future::yield_now,
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
        thread::sleep(Duration::from_millis(1));
    }
}

fn output_callback(output: &str) {
    println!("{}", output);
}
