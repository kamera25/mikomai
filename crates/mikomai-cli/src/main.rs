use mikomai_adapters::memory::InMemoryTaskRepository;
use mikomai_core::ApplicationService;

fn main() {
    let mut args = std::env::args().skip(1);
    match args.next().as_deref() {
        Some("chat") => {
            let goal = args.collect::<Vec<_>>().join(" ");
            let app = ApplicationService { repository: InMemoryTaskRepository::default() };
            match app.start(goal) { Ok(task) => println!("task {} accepted", task.task.id), Err(e) => { eprintln!("mikomai-cli: {e}"); std::process::exit(1); } }
        }
        _ => { eprintln!("usage: mikomai-cli chat <message>"); std::process::exit(2); }
    }
}
