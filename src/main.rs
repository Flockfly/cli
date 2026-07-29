use std::collections::HashMap;
use std::io::{self, Write};
use std::process::Command;
use std::time::Duration;

use flockfly::api::HttpApiFactory;
use flockfly::commands::{run_cli_with, Runtime};

struct StdRuntime {
    env: HashMap<String, String>,
}

impl Runtime for StdRuntime {
    fn env(&self) -> &HashMap<String, String> {
        &self.env
    }

    fn out(&mut self, text: &str) {
        println!("{text}");
    }

    fn err(&mut self, text: &str) {
        eprintln!("{text}");
    }

    fn confirm(&mut self, question: &str) -> bool {
        eprint!("{question}");
        let _ = io::stderr().flush();
        let mut answer = String::new();
        io::stdin()
            .read_line(&mut answer)
            .is_ok_and(|_| matches!(answer.trim().to_ascii_lowercase().as_str(), "y" | "yes"))
    }

    fn open_browser(&mut self, url: &str) {
        let command = if cfg!(target_os = "macos") {
            "open"
        } else if cfg!(target_os = "windows") {
            "start"
        } else {
            "xdg-open"
        };
        let _ = Command::new(command).arg(url).spawn();
    }

    fn sleep(&mut self, duration: Duration) {
        std::thread::sleep(duration);
    }
}

fn main() {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    let mut runtime = StdRuntime {
        env: std::env::vars().collect(),
    };
    let code = run_cli_with(&args, &mut runtime, &HttpApiFactory);
    std::process::exit(code);
}
