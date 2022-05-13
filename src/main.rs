//!

use std::process::exit;
use std::time::Duration;

use argh::FromArgs;

pub mod game;
pub mod term;

/// Voltorb Flip, Goldenrod City's hottest new game of 2010.
#[derive(FromArgs)]
struct Opts {
  /// number of columns for the game board (5 to 8)
  #[argh(option, short = 'c', default = "5")]
  columns: u32,
  /// number of rows for the game board (5 to 8)
  #[argh(option, short = 'r', default = "5")]
  rows: u32,
  /// maximum card number (3 to 9)
  #[argh(option, short = 'm', default = "3")]
  max_card: u8,
}

fn main() {
  let opts: Opts = argh::from_env();

  if !(5..=8).contains(&opts.columns) {
    eprintln!("error: --columns must be between 5 and 8");
    exit(1)
  }
  if !(5..=8).contains(&opts.rows) {
    eprintln!("error: --rows must be between 5 and 8");
    exit(1)
  }
  if !(3..=9).contains(&opts.max_card) {
    eprintln!("error: --max-card must be between 3 and 9");
    exit(1)
  }

  let mut game = game::Game::new(game::Options {
    board_dims: (opts.columns, opts.rows),
    max_card_value: opts.max_card,
  });

  let mut tty = term::tty::AnsiTty::default();
  tty.install_panic_hook();
  
  let mut canvas = term::render::Canvas::new(&mut tty).unwrap();
  canvas.render(game.render(canvas.viewport())).unwrap();

  let mut wait = Some(Duration::default());
  let mut interact = true;
  loop {
    let event = if interact {
      let event = canvas.tty().poll(wait.take()).unwrap();
      if let Some(term::tty::Event::Winch(val)) = event {
        canvas.winch(val);
        canvas.render(game.render(canvas.viewport())).unwrap();
        continue;
      }
      event
    } else if let Some(wait) = wait.take() {
      std::thread::sleep(wait);
      None
    } else {
      None
    };
    interact = true;

    match game.interact(event) {
      game::Response::Quit => break,
      game::Response::WaitForInput => {}
      game::Response::Wait {
        duration,
        ignore_inputs,
      } => {
        wait = Some(duration);
        interact = !ignore_inputs;
      }
    }

    canvas.render(game.render(canvas.viewport())).unwrap();
  }

  canvas.tty().fini().unwrap();
}
