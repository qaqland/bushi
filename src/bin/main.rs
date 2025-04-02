use bushi::{config::Config, state::AppState};

fn main() {
    println!("Hello, world!");
    let config = Config::new().unwrap();
    let mut state = AppState::build(config);
    state.sync_all();
}
