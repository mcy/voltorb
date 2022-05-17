//! A [Voltorb Flip] clone that runs in your ANSI terminal.
//!
//! [Voltorb Flip]: https://bulbapedia.bulbagarden.net/wiki/Voltorb_Flip

use std::process::exit;
use std::time::Duration;
use std::time::Instant;

use argh::FromArgs;

pub mod game;
pub mod term;

/// Voltorb Flip, Goldenrod City's hottest new game of 2010.
/// <https://youtu.be/gRXcyH1JdCI>
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
  /// frames-per-second to run the game at
  #[argh(option, short = 'f', default = "30")]
  fps: u32,
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
  if !(15..=120).contains(&opts.fps) {
    eprintln!("error: --fps must be between 15 and 120");
    exit(1)
  }

  let mut tty = term::AnsiTty::default();
  tty.install_panic_hook();

  let result = term::with_tty(&mut tty, |tty| {
    let mut game = game::Game::new(game::Options {
      board_dims: (opts.columns, opts.rows),
      max_card_value: opts.max_card,
      enable_debugging: cfg!(debug_assertions)
        && std::env::var("VOLTORB_DEBUG").is_ok(),
    });

    let mut canvas = term::Canvas::new(tty.viewport()?);

    let mut event = None;
    loop {
      let frame_timer = Instant::now();
      if let Some(term::Event::Winch(vp)) = event {
        canvas.winch(vp);
        event = None;
      }
      if !game.interact(event) {
        break;
      }
      canvas.render(game.render(canvas.viewport()), tty)?;

      let timeout = Duration::from_secs_f64(1.0 / opts.fps as f64)
        .saturating_sub(frame_timer.elapsed());
      event = tty.poll(Some(timeout))?;
    }

    tty.fini()
  });

  if let Err(e) = result {
    eprintln!("error: {e}");
    exit(1);
  }
}
