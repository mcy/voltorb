//! Game logic.

use std::collections::VecDeque;

use rand::seq::SliceRandom;
use rand::Rng;

use crate::term::Cell;
use crate::term::Event;
use crate::term::Key;
use crate::term::Layer;
use crate::term::Mod;

mod gfx;

// Options for configuring a [`Game`].
pub struct Options {
  // The dimensions of the board. Values must be in `5..=8`.
  pub board_dims: (u32, u32),
  // Maximum value for a multiplier card. Values must be in `3..=9`.
  pub max_card_value: u8,
  /// Enables debug output.
  pub enable_debugging: bool,
}

#[derive(Copy, Clone, Debug, Default)]
struct Card {
  /// The value of the card from 0 to 9; zero is a Voltorb.
  value: u8,
  /// Whether this card has been flipped.
  flipped: bool,
  /// Which memo values have been set by the player for this card.
  memo: u16,
}

#[derive(Copy, Clone, Debug, Default)]
struct Hint {
  /// The sum of cards along a row/column.
  sum: u32,
  /// The number of Voltorbs in a row/column.
  voltorbs: u32,
}

#[derive(Copy, Clone, Debug, Default)]
struct Wait {
  /// The number of frames to wait.
  wait_for: u64,
  /// If true, any input will end the wait instead of being ignored.
  input_ends_wait: bool,
}

/// The big ol' game state struct.
///
/// This contains all state for the current game.
pub struct Game {
  /// Options, which determine the size of the board vectors below.
  options: Options,

  level: u32,
  score: u64,
  round_score: u64,

  cards: Vec<Card>,
  col_hints: Vec<Hint>,
  row_hints: Vec<Hint>,

  /// The index of the card currently selected by the player in `cards`.
  selected_card: usize,
  frame_num: u64,
  state: State,
  /// Whether to wait, preempting game logic for some number of frames.
  waits: Vec<Wait>,

  /// Bitset of which cards are currently flipping. Cards will animate towards
  /// the value of `flipped`, i.e., a card with `flipped` set will appear to
  /// flip face-up.
  cards_flipping: u64,
  /// The frame cards have been flipping since, if any. Cards can only flip
  /// in batches.
  flipping_since: u64,
  /// The number of frames between each frame of flipping.
  frames_per_flip_step: u64,

  debug: VecDeque<String>,
}

#[derive(Clone, Copy, Debug)]
enum State {
  /// A new game needs to be generated.
  NewGame,
  /// The game is currently actively being played.
  Standby,
  /// Check the result of a card getting flipped over.
  FlipCheck,
  /// Indicates that a game ended; this does scoring and proceeds to
  /// NewGame.
  GameOver { new_level: u32, win: bool },
}

const MAX_LEVEL: usize = 8;

impl Game {
  /// Create a new game state.
  pub fn new(options: Options) -> Self {
    let (x, y) = options.board_dims;
    Self {
      level: 1,
      score: 0,
      round_score: 0,

      cards: vec![Card::default(); (x as usize) * (y as usize)],
      col_hints: vec![Hint::default(); x as usize],
      row_hints: vec![Hint::default(); y as usize],

      selected_card: 0,
      state: State::NewGame,
      debug: VecDeque::new(),
      waits: Vec::new(),

      frame_num: 0,
      cards_flipping: 0,
      flipping_since: 0,
      frames_per_flip_step: 1,

      options,
    }
  }

  /// Renders the current game state as a pile of layers that can be handed off
  /// to the compositor.
  pub fn render(&self, viewport: Cell) -> Vec<Layer> {
    gfx::render(self, viewport, &gfx::Stylesheet::default())
  }

  fn debug(&mut self, val: impl FnOnce() -> String) {
    if self.options.enable_debugging {
      if self.debug.len() == 16 {
        let _ = self.debug.pop_front();
      }
      self.debug.push_back(val());
    }
  }

  /// Generates a new game board in-place.
  ///
  /// This function uses a formula, rather than a table like HGSS, which allows
  /// it to be generalized to larger widths and card values. It approximates the
  /// HGSS data for dims = 5x5 and max_card = 3, although not exactly.
  fn generate_board(&mut self) {
    let max_card = self.options.max_card_value as u32;
    let avg_width = (self.options.board_dims.0 + self.options.board_dims.1) / 2;
    let mut rng = rand::thread_rng();

    self.debug(|| "generating new game...".to_string());
    self.debug(|| format!("avg_width: {avg_width}"));

    // The number of Voltorbs is approximately a linear function of the area,
    // so regardless of size the Voltorbs make up a consistent fraction of the
    // board at a particular level.
    let voltorbs = (self.cards.len() / 5 * 2)
      .min((self.level * (avg_width - 3)) as usize + self.cards.len() / 5);
    self.debug(|| format!("voltorbs: {voltorbs}"));

    // The sum of all multiplier cards is a generalization of the formula
    // `sum := 2 * level + 9` that the HGSS data appears to follow.
    //
    // The level used for the computation is either `level` or `level - 1/2`,
    // chosen at random.
    let mut sum = self.level * (max_card - 1) + 3 * max_card;
    if rng.gen::<bool>() {
      sum -= (max_card - 1) / 2;
    }
    self.debug(|| format!("sum: {sum}"));

    // Separately, we compute the maximum payout for this round; this keeps the
    // total payout in close ranges per level.
    //
    // The maxes for vanilla are 50, 100, 200, 400, 600, 1000, 2000, 4000. The
    // current formula scales a value taken from a table by a function that
    // is approximately area**(max_card/2).
    //
    // For vanilla options, this degenerates to the vanilla maxes.
    let maxes: [u64; MAX_LEVEL] = [1, 2, 4, 8, 12, 20, 40, 80];
    let max = maxes[self.level as usize - 1]
      * (1 << (max_card - 2))
      * (self.cards.len() as u64).pow(max_card / 2);
    self.debug(|| format!("max: {max}"));

    // We generate a collection of cards by selecting all card choices that
    // would not cause the prefix product of payouts to overflow max, and pick
    // one randomly. We subtract it from the sum, add it to the payout, and
    // repeat.
    //
    // At this point, we have a game formed, so the rest of this function is
    // just building out the relevant data structures.
    let mut cards = Vec::new();
    let mut coins = 1;
    while sum > 1 && cards.len() < self.cards.len() - voltorbs {
      let max_candidate = (2..=max_card)
        .filter(|&x| (x as u64) * coins <= max && x < sum)
        .max();
      if max_candidate.is_none() {
        break;
      }

      let value = rng.gen_range(2..=max_candidate.unwrap());
      coins *= value as u64;
      sum -= value;
      cards.push(value);
    }
    self.debug(|| format!("cards: {cards:?}"));
    self.debug(|| format!("coins: {coins}"));

    self.cards.fill(Card {
      value: 1,
      ..Card::default()
    });

    let mut indices = (0usize..self.cards.len()).collect::<Vec<_>>();
    indices.shuffle(&mut rng);

    self.debug(|| format!("voltorbs: {:?}", &indices[..voltorbs]));
    for index in &indices[..voltorbs] {
      self.cards[*index].value = 0;
    }

    self.debug(|| {
      let zip = Iterator::zip(indices[voltorbs..].iter(), cards.iter());
      let pairs = zip.map(|(i, c)| format!("{i}: {c}")).collect::<Vec<_>>();
      format!("cards: {{{}}}", pairs.join(", "))
    });
    for (index, card) in Iterator::zip(indices[voltorbs..].iter(), cards.iter())
    {
      self.cards[*index].value = *card as u8;
    }

    self.round_score = 0;
    self.row_hints.fill(Hint::default());
    self.col_hints.fill(Hint::default());

    let stride = self.options.board_dims.0 as usize;
    for (i, card) in self.cards.iter().enumerate() {
      self.row_hints[i / stride].sum += card.value as u32;
      self.col_hints[i % stride].sum += card.value as u32;
      if card.value == 0 {
        self.row_hints[i / stride].voltorbs += 1;
        self.col_hints[i % stride].voltorbs += 1;
      }
    }
  }

  fn flip_all(&mut self, flipped: bool) {
    let mut cards_flipping = 0;
    for (i, card) in self.cards.iter_mut().enumerate() {
      if card.flipped != flipped {
        cards_flipping |= 1 << i;
      }
      card.flipped = flipped;
    }

    self.frames_per_flip_step = 1;
    self.cards_flipping = cards_flipping;
    self.flipping_since = self.frame_num;
    self.waits.push(Wait {
      wait_for: gfx::CARD_WIDTH as u64,
      input_ends_wait: false,
    });
  }

  fn flip_selected(&mut self, flipped: bool, slow: bool) {
    self.cards[self.selected_card].flipped = flipped;

    self.frames_per_flip_step = if slow { 10 } else { 1 };
    self.cards_flipping = 1 << self.selected_card;
    self.flipping_since = self.frame_num;
    self.waits.push(Wait {
      wait_for: self.frames_per_flip_step * (gfx::CARD_WIDTH / 2 + 1) as u64
        + gfx::CARD_WIDTH as u64,
      input_ends_wait: false,
    });
  }

  /// Presents a player interaction to the game.
  ///
  /// Returns whether the game loop should continue.
  pub fn interact(&mut self, event: Option<Event>) -> bool {
    self.frame_num += 1;
    let stride = self.options.board_dims.0 as usize;
    if event.is_some() {
      let state = self.state;
      let frame_num = self.frame_num;
      self.debug(|| format!("interact: {frame_num}, {state:?}, {event:?}"));
    }

    if matches!(event, Some(Event::Key { key: Key::Glyph('c' | 'C'), mods}) if mods.contains(Mod::Ctrl))
    {
      return false;
    }

    if let Some(wait) = self.waits.first_mut() {
      if wait.wait_for == 0 || (wait.input_ends_wait && event.is_some()) {
        self.waits.remove(0);
        if !self.waits.is_empty() {
          return true;
        }
      } else {
        wait.wait_for -= 1;
        return true;
      }
    }

    match (self.state, event) {
      (State::NewGame, _) => {
        self.generate_board();
        self.state = State::Standby;
      }

      (State::Standby, Some(Event::Key { key, .. })) => match key {
        Key::Glyph('q' | 'Q') => return false,
        Key::Left => {
          if self.selected_card % stride == 0 {
            self.selected_card += stride - 1;
          } else {
            self.selected_card -= 1;
          }
        }
        Key::Right => {
          self.selected_card += 1;
          if self.selected_card % stride == 0 {
            self.selected_card -= stride;
          }
        }
        Key::Up => {
          if let Some(select) = self.selected_card.checked_sub(stride) {
            self.selected_card = select;
          } else {
            self.selected_card =
              self.cards.len() - (stride - self.selected_card % stride);
          }
        }
        Key::Down => {
          self.selected_card += stride;
          if self.selected_card > self.cards.len() {
            self.selected_card %= stride;
          }
        }
        Key::Enter | Key::Glyph('\\')
          if key == Key::Enter || self.options.enable_debugging =>
        {
          if !self.cards[self.selected_card].flipped {
            // If only one card remains to be flipped, make this a slow flip
            // 10% of the time.
            //
            // In debug mode, \ will do this too..
            let remaining = self.cards.iter().filter(|c| c.value > 1).count();
            let slow = (remaining == 1 && rand::thread_rng().gen_bool(0.1))
              || key != Key::Enter;

            self.state = State::FlipCheck;
            self.flip_selected(true, slow);
          }
        }
        Key::Glyph(k @ '0'..='9') => {
          let index = k as u8 - b'0';
          self.cards[self.selected_card].memo ^= 1 << index;
        }
        Key::PageUp if self.options.enable_debugging => {
          self.state = State::GameOver {
            new_level: self.level + 1,
            win: true,
          };
        }
        Key::PageDown if self.options.enable_debugging => {
          self.state = State::GameOver {
            new_level: self.level - 1,
            win: false,
          };
        }
        _ => {}
      },

      (State::FlipCheck, _) => {
        let card = &mut self.cards[self.selected_card];
        if card.value == 0 {
          let flipped = self.cards.iter().filter(|x| x.flipped).count();
          self.state = State::GameOver {
            new_level: (flipped as u32 - 1).min(self.level),
            win: false,
          };
          self.flip_all(true);
          self.waits.push(Wait {
            wait_for: 30 * 5,
            input_ends_wait: true,
          });
          return true;
        }

        if self.round_score == 0 {
          self.round_score = 1;
        }
        self.round_score *= card.value as u64;

        // Check if we've won.
        if !self
          .cards
          .iter()
          .any(|card| card.value > 1 && !card.flipped)
        {
          self.state = State::GameOver {
            new_level: self.level + 1,
            win: true,
          };
          self.flip_all(true);
          self.waits.push(Wait {
            wait_for: 30 * 5,
            input_ends_wait: true,
          });
          return true;
        }

        self.state = State::Standby;
      }

      (State::GameOver { new_level, win }, _) => {
        if win {
          self.score += self.round_score;
        }
        self.level = new_level.clamp(1, MAX_LEVEL as u32);
        self.flip_all(false);
        self.state = State::NewGame;
      }
      _ => {}
    }

    true
  }
}
