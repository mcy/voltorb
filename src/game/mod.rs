//! Game logic.

use std::time::Duration;

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
}

#[derive(Copy, Clone, Debug, Default)]
struct Card {
  /// The value of the card from 0 to 9; zero is a Voltorb.
  value: u8,
  /// Whether this card has been flipped.
  flipped: bool,
  /// By how many texels on each side to compress the card (for flipping).
  compress: u8,
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

  state: State,
}

#[derive(Clone, Copy, Debug)]
enum State {
  /// A new game needs to be generated. This flips all cards face down and then
  /// generates a new game at the given level.
  NewGame(u32),
  /// The game is currently actively being played.
  Standby,
  /// The game is flipping over a single card to reveal it to the player.
  Flipping,
  /// A Voltorb was just flipped over. All cards are revealed, and then a
  /// new game starts.
  GameOver(u32),
  /// The last multiplier was just flipped over. Proceeds immediately to a
  /// new game.
  LevelUp,
}

/// An interaction response, instructing the main loop to do something.
#[derive(Clone, Copy, Debug)]
pub enum Response {
  /// Wait some amount of time and then interact with a `None` input again.
  Wait {
    /// The time to wait.
    duration: Duration,
    /// If false, user input can interrupt the wait and trigger the next
    /// interaction early.
    ignore_inputs: bool,
  },
  /// Wait for user interaction forever.
  WaitForInput,
  /// Clean up and quit.
  Quit,
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
      state: State::NewGame(1),

      options,
    }
  }

  /// Renders the current game state as a pile of layers that can be handed off
  /// to the compositor.
  pub fn render(&self, viewport: Cell) -> Vec<Layer> {
    gfx::render(self, viewport, &gfx::Stylesheet::default())
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

    // The number of Voltorbs is approximately a linear function of the area,
    // so regardless of size the Voltorbs make up a consistent fraction of the
    // board at a particular level.
    let voltorbs = (self.cards.len() / 5 * 2)
      .min((self.level * (avg_width - 3)) as usize + self.cards.len() / 5);

    // The sum of all multiplier cards is a generalization of the formula
    // `sum := 2 * level + 9` that the HGSS data appears to follow.
    //
    // The level used for the computation is either `level` or `level - 1/2`,
    // chosen at random.
    let mut sum = self.level * (max_card - 1) + 3 * max_card;
    if rng.gen::<bool>() {
      sum -= (max_card - 1) / 2;
    }

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
      * (1 << (max_card - 1))
      * (self.cards.len() as u64).pow(max_card / 2);

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
      sum -= max_candidate.unwrap();
      cards.push(value);
    }

    self.cards.fill(Card {
      value: 1,
      ..Card::default()
    });

    let mut indices = (0usize..self.cards.len()).collect::<Vec<_>>();
    indices.shuffle(&mut rng);
    for index in &indices[0..voltorbs] {
      self.cards[*index].value = 0;
    }
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

  /// Presents a player interaction to the game.
  ///
  /// This drives the internal state machine forward for the game to
  /// respond in kind.
  pub fn interact(&mut self, event: Option<Event>) -> Response {
    let stride = self.options.board_dims.0 as usize;
    match (self.state, event) {
      (
        _,
        Some(Event::Key {
          key: Key::Glyph('q'),
          ..
        }),
      ) => return Response::Quit,
      (
        _,
        Some(Event::Key {
          key: Key::Glyph('c'),
          mods,
        }),
      ) if mods.contains(Mod::Ctrl) => return Response::Quit,

      (State::NewGame(level), _) => {
        let mut done = true;
        for card in &mut self.cards {
          if !card.flipped && card.compress > 0 {
            card.compress -= 1;
            if card.compress != 0 {
              done = false;
            }
          } else if card.flipped {
            card.compress += 1;
            if card.compress == 4 {
              card.flipped = false;
            }
            done = false;
          }
        }
        if !done {
          return Response::Wait {
            duration: Duration::from_millis(10),
            ignore_inputs: true,
          };
        }

        self.level = level.clamp(1, MAX_LEVEL as u32);
        self.generate_board();
        self.state = State::Standby;
      }

      (State::Standby, Some(Event::Key { key, .. })) => match key {
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
        Key::Enter => {
          if !self.cards[self.selected_card].flipped {
            self.state = State::Flipping;
            self.cards[self.selected_card].compress += 1;
            return Response::Wait {
              duration: Duration::from_millis(10),
              ignore_inputs: true,
            };
          }
        }
        Key::Glyph(k @ '0'..='9') => {
          let index = k as u8 - b'0';
          self.cards[self.selected_card].memo ^= 1 << index;
        }
        _ => {}
      },

      (State::Flipping, _) => {
        let card = &mut self.cards[self.selected_card];
        if card.flipped {
          card.compress -= 1;
          if card.compress == 0 {
            // Resolve actual game logic now that the flip has happened.
            if card.value == 0 {
              let flipped = self.cards.iter().filter(|x| x.flipped).count();
              self.state =
                State::GameOver((flipped as u32 - 1).min(self.level));
              return Response::Wait {
                duration: Duration::from_millis(500),
                ignore_inputs: false,
              };
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
              self.state = State::LevelUp;
              return Response::Wait {
                duration: Duration::from_millis(500),
                ignore_inputs: false,
              };
            }

            self.state = State::Standby;
            return Response::WaitForInput;
          }
        } else {
          card.compress += 1;
          if card.compress == 4 {
            card.flipped = true;
          }
        }
        return Response::Wait {
          duration: Duration::from_millis(10),
          ignore_inputs: true,
        };
      }

      (State::GameOver(level), _) => {
        let mut done = true;
        for card in &mut self.cards {
          if card.flipped && card.compress > 0 {
            card.compress -= 1;
            if card.compress != 0 {
              done = false;
            }
          } else if !card.flipped {
            card.compress += 1;
            if card.compress == 4 {
              card.flipped = true;
            }
            done = false;
          }
        }
        if !done {
          return Response::Wait {
            duration: Duration::from_millis(10),
            ignore_inputs: true,
          };
        }

        self.state = State::NewGame(level);
        return Response::Wait {
          duration: Duration::from_secs(5),
          ignore_inputs: false,
        };
      }

      (State::LevelUp, _) => {
        self.score = self.round_score;
        self.state = State::NewGame(self.level + 1);
        return Response::Wait {
          duration: Duration::from_secs(5),
          ignore_inputs: false,
        };
      }
      _ => {}
    }

    Response::WaitForInput
  }
}
